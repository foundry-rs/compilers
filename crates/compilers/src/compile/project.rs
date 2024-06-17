//! Manages compiling of a `Project`
//!
//! The compilation of a project is performed in several steps.
//!
//! First the project's dependency graph [`crate::Graph`] is constructed and all imported
//! dependencies are resolved. The graph holds all the relationships between the files and their
//! versions. From there the appropriate version set is derived
//! [`crate::Graph`] which need to be compiled with different
//! [`crate::compilers::solc::Solc`] versions.
//!
//! At this point we check if we need to compile a source file or whether we can reuse an _existing_
//! `Artifact`. We don't to compile if:
//!     - caching is enabled
//!     - the file is **not** dirty
//!     - the artifact for that file exists
//!
//! This concludes the preprocessing, and we now have either
//!    - only `Source` files that need to be compiled
//!    - only cached `Artifacts`, compilation can be skipped. This is considered an unchanged,
//!      cached project
//!    - Mix of both `Source` and `Artifacts`, only the `Source` files need to be compiled, the
//!      `Artifacts` can be reused.
//!
//! The final step is invoking `Solc` via the standard JSON format.
//!
//! ### Notes on [Import Path Resolution](https://docs.soliditylang.org/en/develop/path-resolution.html#path-resolution)
//!
//! In order to be able to support reproducible builds on all platforms, the Solidity compiler has
//! to abstract away the details of the filesystem where source files are stored. Paths used in
//! imports must work the same way everywhere while the command-line interface must be able to work
//! with platform-specific paths to provide good user experience. This section aims to explain in
//! detail how Solidity reconciles these requirements.
//!
//! The compiler maintains an internal database (virtual filesystem or VFS for short) where each
//! source unit is assigned a unique source unit name which is an opaque and unstructured
//! identifier. When you use the import statement, you specify an import path that references a
//! source unit name. If the compiler does not find any source unit name matching the import path in
//! the VFS, it invokes the callback, which is responsible for obtaining the source code to be
//! placed under that name.
//!
//! This becomes relevant when dealing with resolved imports
//!
//! #### Relative Imports
//!
//! ```solidity
//! import "./math/math.sol";
//! import "contracts/tokens/token.sol";
//! ```
//! In the above `./math/math.sol` and `contracts/tokens/token.sol` are import paths while the
//! source unit names they translate to are `contracts/math/math.sol` and
//! `contracts/tokens/token.sol` respectively.
//!
//! #### Direct Imports
//!
//! An import that does not start with `./` or `../` is a direct import.
//!
//! ```solidity
//! import "/project/lib/util.sol";         // source unit name: /project/lib/util.sol
//! import "lib/util.sol";                  // source unit name: lib/util.sol
//! import "@openzeppelin/address.sol";     // source unit name: @openzeppelin/address.sol
//! import "https://example.com/token.sol"; // source unit name: <https://example.com/token.sol>
//! ```
//!
//! After applying any import remappings the import path simply becomes the source unit name.
//!
//! ##### Import Remapping
//!
//! ```solidity
//! import "github.com/ethereum/dapp-bin/library/math.sol"; // source unit name: dapp-bin/library/math.sol
//! ```
//!
//! If compiled with `solc github.com/ethereum/dapp-bin/=dapp-bin/` the compiler will look for the
//! file in the VFS under `dapp-bin/library/math.sol`. If the file is not available there, the
//! source unit name will be passed to the Host Filesystem Loader, which will then look in
//! `/project/dapp-bin/library/iterable_mapping.sol`
//!
//!
//! ### Caching and Change detection
//!
//! If caching is enabled in the [Project] a cache file will be created upon a successful solc
//! build. The [cache file](crate::cache::CompilerCache) stores metadata for all the files that were
//! provided to solc.
//! For every file the cache file contains a dedicated [cache entry](crate::cache::CacheEntry),
//! which represents the state of the file. A solidity file can contain several contracts, for every
//! contract a separate [artifact](crate::Artifact) is emitted. Therefor the entry also tracks all
//! artifacts emitted by a file. A solidity file can also be compiled with several solc versions.
//!
//! For example in `A(<=0.8.10) imports C(>0.4.0)` and
//! `B(0.8.11) imports C(>0.4.0)`, both `A` and `B` import `C` but there's no solc version that's
//! compatible with `A` and `B`, in which case two sets are compiled: [`A`, `C`] and [`B`, `C`].
//! This is reflected in the cache entry which tracks the file's artifacts by version.
//!
//! The cache makes it possible to detect changes during recompilation, so that only the changed,
//! dirty, files need to be passed to solc. A file will be considered as dirty if:
//!   - the file is new, not included in the existing cache
//!   - the file was modified since the last compiler run, detected by comparing content hashes
//!   - any of the imported files is dirty
//!   - the file's artifacts don't exist, were deleted.
//!
//! Recompiling a project with cache enabled detects all files that meet these criteria and provides
//! solc with only these dirty files instead of the entire source set.

use crate::{
    artifact_output::Artifacts,
    buildinfo::RawBuildInfo,
    cache::ArtifactsCache,
    compilers::{Compiler, CompilerInput, CompilerOutput, Language},
    filter::SparseOutputFilter,
    output::{AggregatedCompilerOutput, Builds},
    report,
    resolver::GraphEdges,
    ArtifactOutput, FilteredSources, Graph, Project, ProjectCompileOutput, Sources,
};
use foundry_compilers_core::error::Result;
use rayon::prelude::*;
use semver::Version;
use std::{collections::HashMap, path::PathBuf, time::Instant};

/// A set of different Solc installations with their version and the sources to be compiled
pub(crate) type VersionedSources<L> = HashMap<L, HashMap<Version, Sources>>;

/// A set of different Solc installations with their version and the sources to be compiled
pub(crate) type VersionedFilteredSources<L> = HashMap<L, HashMap<Version, FilteredSources>>;

#[derive(Debug)]
pub struct ProjectCompiler<'a, T: ArtifactOutput, C: Compiler> {
    /// Contains the relationship of the source files and their imports
    edges: GraphEdges<C::ParsedSource>,
    project: &'a Project<C, T>,
    /// how to compile all the sources
    sources: CompilerSources<C::Language>,
}

impl<'a, T: ArtifactOutput, C: Compiler> ProjectCompiler<'a, T, C> {
    /// Create a new `ProjectCompiler` to bootstrap the compilation process of the project's
    /// sources.
    pub fn new(project: &'a Project<C, T>) -> Result<Self> {
        Self::with_sources(project, project.paths.read_input_files()?)
    }

    /// Bootstraps the compilation process by resolving the dependency graph of all sources and the
    /// appropriate `Solc` -> `Sources` set as well as the compile mode to use (parallel,
    /// sequential)
    ///
    /// Multiple (`Solc` -> `Sources`) pairs can be compiled in parallel if the `Project` allows
    /// multiple `jobs`, see [`crate::Project::set_solc_jobs()`].
    pub fn with_sources(project: &'a Project<C, T>, mut sources: Sources) -> Result<Self> {
        if let Some(filter) = &project.sparse_output {
            sources.retain(|f, _| filter.is_match(f))
        }
        let graph = Graph::resolve_sources(&project.paths, sources)?;
        let (sources, edges) = graph.into_sources_by_version(
            project.offline,
            &project.locked_versions,
            &project.compiler,
        )?;

        let jobs_cnt = sources.values().map(|v| v.len()).sum::<usize>();

        let sources = if project.solc_jobs > 1 && jobs_cnt > 1 {
            // if there are multiple different versions, and we can use multiple jobs we can compile
            // them in parallel
            CompilerSources::Parallel(sources, project.solc_jobs)
        } else {
            CompilerSources::Sequential(sources)
        };

        Ok(Self { edges, project, sources })
    }

    /// Compiles all the sources of the `Project` in the appropriate mode
    ///
    /// If caching is enabled, the sources are filtered and only _dirty_ sources are recompiled.
    ///
    /// The output of the compile process can be a mix of reused artifacts and freshly compiled
    /// `Contract`s
    ///
    /// # Examples
    #[cfg_attr(not(feature = "svm-solc"), doc = "```ignore")]
    /// ```no_run
    /// use foundry_compilers::Project;
    ///
    /// let project = Project::builder().build(Default::default())?;
    /// let output = project.compile()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn compile(self) -> Result<ProjectCompileOutput<C, T>> {
        let slash_paths = self.project.slash_paths;

        // drive the compiler statemachine to completion
        let mut output = self.preprocess()?.compile()?.write_artifacts()?.write_cache()?;

        if slash_paths {
            // ensures we always use `/` paths
            output.slash_paths();
        }

        Ok(output)
    }

    /// Does basic preprocessing
    ///   - sets proper source unit names
    ///   - check cache
    fn preprocess(self) -> Result<PreprocessedState<'a, T, C>> {
        trace!("preprocessing");
        let Self { edges, project, mut sources } = self;

        // convert paths on windows to ensure consistency with the `CompilerOutput` `solc` emits,
        // which is unix style `/`
        sources.slash_paths();

        let mut cache = ArtifactsCache::new(project, edges)?;
        // retain and compile only dirty sources and all their imports
        let sources = sources.filtered(&mut cache);

        Ok(PreprocessedState { sources, cache })
    }
}

/// A series of states that comprise the [`ProjectCompiler::compile()`] state machine
///
/// The main reason is to debug all states individually
#[derive(Debug)]
struct PreprocessedState<'a, T: ArtifactOutput, C: Compiler> {
    /// Contains all the sources to compile.
    sources: FilteredCompilerSources<C::Language>,

    /// Cache that holds `CacheEntry` objects if caching is enabled and the project is recompiled
    cache: ArtifactsCache<'a, T, C>,
}

impl<'a, T: ArtifactOutput, C: Compiler> PreprocessedState<'a, T, C> {
    /// advance to the next state by compiling all sources
    fn compile(self) -> Result<CompiledState<'a, T, C>> {
        trace!("compiling");
        let PreprocessedState { sources, mut cache } = self;

        let mut output = sources.compile(&mut cache)?;

        // source paths get stripped before handing them over to solc, so solc never uses absolute
        // paths, instead `--base-path <root dir>` is set. this way any metadata that's derived from
        // data (paths) is relative to the project dir and should be independent of the current OS
        // disk. However internally we still want to keep absolute paths, so we join the
        // contracts again
        output.join_all(cache.project().root());

        Ok(CompiledState { output, cache })
    }
}

/// Represents the state after `solc` was successfully invoked
#[derive(Debug)]
struct CompiledState<'a, T: ArtifactOutput, C: Compiler> {
    output: AggregatedCompilerOutput<C>,
    cache: ArtifactsCache<'a, T, C>,
}

impl<'a, T: ArtifactOutput, C: Compiler> CompiledState<'a, T, C> {
    /// advance to the next state by handling all artifacts
    ///
    /// Writes all output contracts to disk if enabled in the `Project` and if the build was
    /// successful
    #[instrument(skip_all, name = "write-artifacts")]
    fn write_artifacts(self) -> Result<ArtifactsState<'a, T, C>> {
        let CompiledState { output, cache } = self;

        let project = cache.project();
        let ctx = cache.output_ctx();
        // write all artifacts via the handler but only if the build succeeded and project wasn't
        // configured with `no_artifacts == true`
        let compiled_artifacts = if project.no_artifacts {
            project.artifacts_handler().output_to_artifacts(
                &output.contracts,
                &output.sources,
                ctx,
                &project.paths,
            )
        } else if output.has_error(
            &project.ignored_error_codes,
            &project.ignored_file_paths,
            &project.compiler_severity_filter,
        ) {
            trace!("skip writing cache file due to solc errors: {:?}", output.errors);
            project.artifacts_handler().output_to_artifacts(
                &output.contracts,
                &output.sources,
                ctx,
                &project.paths,
            )
        } else {
            trace!(
                "handling artifact output for {} contracts and {} sources",
                output.contracts.len(),
                output.sources.len()
            );
            // this emits the artifacts via the project's artifacts handler
            let artifacts = project.artifacts_handler().on_output(
                &output.contracts,
                &output.sources,
                &project.paths,
                ctx,
            )?;

            // emits all the build infos, if they exist
            output.write_build_infos(project.build_info_path())?;

            artifacts
        };

        Ok(ArtifactsState { output, cache, compiled_artifacts })
    }
}

/// Represents the state after all artifacts were written to disk
#[derive(Debug)]
struct ArtifactsState<'a, T: ArtifactOutput, C: Compiler> {
    output: AggregatedCompilerOutput<C>,
    cache: ArtifactsCache<'a, T, C>,
    compiled_artifacts: Artifacts<T::Artifact>,
}

impl<'a, T: ArtifactOutput, C: Compiler> ArtifactsState<'a, T, C> {
    /// Writes the cache file
    ///
    /// this concludes the [`Project::compile()`] statemachine
    fn write_cache(self) -> Result<ProjectCompileOutput<C, T>> {
        let ArtifactsState { output, cache, compiled_artifacts } = self;
        let project = cache.project();
        let ignored_error_codes = project.ignored_error_codes.clone();
        let ignored_file_paths = project.ignored_file_paths.clone();
        let compiler_severity_filter = project.compiler_severity_filter;
        let has_error =
            output.has_error(&ignored_error_codes, &ignored_file_paths, &compiler_severity_filter);
        let skip_write_to_disk = project.no_artifacts || has_error;
        trace!(has_error, project.no_artifacts, skip_write_to_disk, cache_path=?project.cache_path(),"prepare writing cache file");

        let (cached_artifacts, cached_builds) =
            cache.consume(&compiled_artifacts, &output.build_infos, !skip_write_to_disk)?;

        project.artifacts_handler().handle_cached_artifacts(&cached_artifacts)?;

        let builds = Builds(
            output
                .build_infos
                .iter()
                .map(|build_info| (build_info.id.clone(), build_info.build_context.clone()))
                .chain(cached_builds)
                .map(|(id, context)| (id, context.with_joined_paths(project.paths.root.as_path())))
                .collect(),
        );

        Ok(ProjectCompileOutput {
            compiler_output: output,
            compiled_artifacts,
            cached_artifacts,
            ignored_error_codes,
            ignored_file_paths,
            compiler_severity_filter,
            builds,
        })
    }
}

/// Determines how the `solc <-> sources` pairs are executed
#[derive(Debug, Clone)]
enum CompilerSources<L> {
    /// Compile all these sequentially
    Sequential(VersionedSources<L>),
    /// Compile all these in parallel using a certain amount of jobs
    #[allow(dead_code)]
    Parallel(VersionedSources<L>, usize),
}

impl<L: Language> CompilerSources<L> {
    /// Converts all `\\` separators to `/`
    ///
    /// This effectively ensures that `solc` can find imported files like `/src/Cheats.sol` in the
    /// VFS (the `CompilerInput` as json) under `src/Cheats.sol`.
    fn slash_paths(&mut self) {
        #[cfg(windows)]
        {
            use path_slash::PathBufExt;

            fn slash_versioned_sources<L: Language>(v: &mut VersionedSources<L>) {
                v.values_mut().for_each(|versioned_sources| {
                    versioned_sources.values_mut().for_each(|sources| {
                        *sources = std::mem::take(sources)
                            .into_iter()
                            .map(|(path, source)| {
                                (PathBuf::from(path.to_slash_lossy().as_ref()), source)
                            })
                            .collect()
                    })
                });
            }

            match self {
                CompilerSources::Sequential(v) => slash_versioned_sources(v),
                CompilerSources::Parallel(v, _) => slash_versioned_sources(v),
            };
        }
    }

    /// Filters out all sources that don't need to be compiled, see [`ArtifactsCache::filter`]
    fn filtered<T: ArtifactOutput, C: Compiler<Language = L>>(
        self,
        cache: &mut ArtifactsCache<'_, T, C>,
    ) -> FilteredCompilerSources<L> {
        fn filtered_sources<T: ArtifactOutput, C: Compiler>(
            sources: VersionedSources<C::Language>,
            cache: &mut ArtifactsCache<'_, T, C>,
        ) -> VersionedFilteredSources<C::Language> {
            cache.remove_dirty_sources();

            sources
                .into_iter()
                .map(|(language, versioned_sources)| {
                    (
                        language,
                        versioned_sources
                            .into_iter()
                            .map(|(version, sources)| {
                                trace!("Filtering {} sources for {}", sources.len(), version);
                                let sources_to_compile = cache.filter(sources, &version);
                                trace!(
                                    "Detected {} sources to compile {:?}",
                                    sources_to_compile.dirty().count(),
                                    sources_to_compile.dirty_files().collect::<Vec<_>>()
                                );

                                (version, sources_to_compile)
                            })
                            .collect(),
                    )
                })
                .collect()
        }

        match self {
            Self::Sequential(s) => FilteredCompilerSources::Sequential(filtered_sources(s, cache)),
            Self::Parallel(s, j) => {
                FilteredCompilerSources::Parallel(filtered_sources(s, cache), j)
            }
        }
    }
}

/// Determines how the `solc <-> sources` pairs are executed
#[derive(Debug, Clone)]
enum FilteredCompilerSources<L> {
    /// Compile all these sequentially
    Sequential(VersionedFilteredSources<L>),
    /// Compile all these in parallel using a certain amount of jobs
    Parallel(VersionedFilteredSources<L>, usize),
}

impl<L: Language> FilteredCompilerSources<L> {
    /// Compiles all the files with `Solc`
    fn compile<C: Compiler<Language = L>, T: ArtifactOutput>(
        self,
        cache: &mut ArtifactsCache<'_, T, C>,
    ) -> Result<AggregatedCompilerOutput<C>> {
        let project = cache.project();
        let graph = cache.graph();

        let jobs_cnt = if let Self::Parallel(_, jobs_cnt) = self { Some(jobs_cnt) } else { None };

        let sparse_output = SparseOutputFilter::new(project.sparse_output.as_deref());

        let sources = self.into_sources();
        // Include additional paths collected during graph resolution.
        let mut include_paths = project.paths.include_paths.clone();
        include_paths.extend(graph.include_paths().clone());

        let mut jobs = Vec::new();
        for (language, versioned_sources) in sources {
            for (version, filtered_sources) in versioned_sources {
                if filtered_sources.is_empty() {
                    // nothing to compile
                    trace!("skip {} for empty sources set", version);
                    continue;
                }

                // depending on the composition of the filtered sources, the output selection can be
                // optimized
                let mut opt_settings = project.settings.clone();
                let (sources, actually_dirty) =
                    sparse_output.sparse_sources(filtered_sources, &mut opt_settings, graph);

                if actually_dirty.is_empty() {
                    // nothing to compile for this particular language, all dirty files are in the
                    // other language set
                    trace!("skip {} run due to empty source set", version);
                    continue;
                }

                trace!("calling {} with {} sources {:?}", version, sources.len(), sources.keys());

                let mut input = C::Input::build(sources, opt_settings, language, version.clone())
                    .with_base_path(project.paths.root.clone())
                    .with_allow_paths(project.paths.allowed_paths.clone())
                    .with_include_paths(include_paths.clone())
                    .with_remappings(project.paths.remappings.clone());

                input.strip_prefix(project.paths.root.as_path());

                jobs.push((input, actually_dirty));
            }
        }

        let results = if let Some(num_jobs) = jobs_cnt {
            compile_parallel(&project.compiler, jobs, num_jobs)
        } else {
            compile_sequential(&project.compiler, jobs)
        }?;

        let mut aggregated = AggregatedCompilerOutput::default();

        for (input, mut output, actually_dirty) in results {
            let version = input.version();

            // Mark all files as seen by the compiler
            for file in &actually_dirty {
                cache.compiler_seen(file);
            }

            let build_info = RawBuildInfo::new(&input, &output, project.build_info)?;

            output.retain_files(
                actually_dirty
                    .iter()
                    .map(|f| f.strip_prefix(project.paths.root.as_path()).unwrap_or(f)),
            );
            output.join_all(project.paths.root.as_path());

            aggregated.extend(version.clone(), build_info, output);
        }

        Ok(aggregated)
    }

    #[cfg(test)]
    #[cfg(all(feature = "project-util", feature = "svm-solc"))]
    fn sources(&self) -> &VersionedFilteredSources<L> {
        match self {
            Self::Sequential(v) => v,
            Self::Parallel(v, _) => v,
        }
    }

    fn into_sources(self) -> VersionedFilteredSources<L> {
        match self {
            Self::Sequential(v) => v,
            Self::Parallel(v, _) => v,
        }
    }
}

type CompilationResult<I, E> = Result<Vec<(I, CompilerOutput<E>, Vec<PathBuf>)>>;

/// Compiles the input set sequentially and returns a [Vec] of outputs.
fn compile_sequential<C: Compiler>(
    compiler: &C,
    jobs: Vec<(C::Input, Vec<PathBuf>)>,
) -> CompilationResult<C::Input, C::CompilationError> {
    jobs.into_iter()
        .map(|(input, actually_dirty)| {
            let start = Instant::now();
            report::compiler_spawn(
                &input.compiler_name(),
                input.version(),
                actually_dirty.as_slice(),
            );
            let output = compiler.compile(&input)?;
            report::compiler_success(&input.compiler_name(), input.version(), &start.elapsed());

            Ok((input, output, actually_dirty))
        })
        .collect()
}

/// compiles the input set using `num_jobs` threads
fn compile_parallel<C: Compiler>(
    compiler: &C,
    jobs: Vec<(C::Input, Vec<PathBuf>)>,
    num_jobs: usize,
) -> CompilationResult<C::Input, C::CompilationError> {
    // need to get the currently installed reporter before installing the pool, otherwise each new
    // thread in the pool will get initialized with the default value of the `thread_local!`'s
    // localkey. This way we keep access to the reporter in the rayon pool
    let scoped_report = report::get_default(|reporter| reporter.clone());

    // start a rayon threadpool that will execute all `Solc::compile()` processes
    let pool = rayon::ThreadPoolBuilder::new().num_threads(num_jobs).build().unwrap();

    pool.install(move || {
        jobs.into_par_iter()
            .map(move |(input, actually_dirty)| {
                // set the reporter on this thread
                let _guard = report::set_scoped(&scoped_report);

                let start = Instant::now();
                report::compiler_spawn(
                    &input.compiler_name(),
                    input.version(),
                    actually_dirty.as_slice(),
                );
                compiler.compile(&input).map(move |output| {
                    report::compiler_success(
                        &input.compiler_name(),
                        input.version(),
                        &start.elapsed(),
                    );
                    (input, output, actually_dirty)
                })
            })
            .collect()
    })
}

#[cfg(test)]
#[cfg(all(feature = "project-util", feature = "svm-solc"))]
mod tests {
    use std::path::Path;

    use foundry_compilers_artifacts::output_selection::ContractOutputSelection;

    use crate::{
        compilers::multi::MultiCompiler, project_util::TempProject, ConfigurableArtifacts,
        MinimalCombinedArtifacts, ProjectPathsConfig,
    };

    use super::*;

    fn init_tracing() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init()
            .ok();
    }

    #[test]
    fn can_preprocess() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/dapp-sample");
        let project = Project::builder()
            .paths(ProjectPathsConfig::dapptools(root).unwrap())
            .build(Default::default())
            .unwrap();

        let compiler = ProjectCompiler::new(&project).unwrap();
        let prep = compiler.preprocess().unwrap();
        let cache = prep.cache.as_cached().unwrap();
        // ensure that we have exactly 3 empty entries which will be filled on compilation.
        assert_eq!(cache.cache.files.len(), 3);
        assert!(cache.cache.files.values().all(|v| v.artifacts.is_empty()));

        let compiled = prep.compile().unwrap();
        assert_eq!(compiled.output.contracts.files().count(), 3);
    }

    #[test]
    fn can_detect_cached_files() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/dapp-sample");
        let paths = ProjectPathsConfig::builder().sources(root.join("src")).lib(root.join("lib"));
        let project = TempProject::<MultiCompiler, MinimalCombinedArtifacts>::new(paths).unwrap();

        let compiled = project.compile().unwrap();
        compiled.assert_success();

        let inner = project.project();
        let compiler = ProjectCompiler::new(inner).unwrap();
        let prep = compiler.preprocess().unwrap();
        assert!(prep.cache.as_cached().unwrap().dirty_sources.is_empty())
    }

    #[test]
    fn can_recompile_with_optimized_output() {
        let tmp = TempProject::<MultiCompiler, ConfigurableArtifacts>::dapptools().unwrap();

        tmp.add_source(
            "A",
            r#"
    pragma solidity ^0.8.10;
    import "./B.sol";
    contract A {}
   "#,
        )
        .unwrap();

        tmp.add_source(
            "B",
            r#"
    pragma solidity ^0.8.10;
    contract B {
        function hello() public {}
    }
    import "./C.sol";
   "#,
        )
        .unwrap();

        tmp.add_source(
            "C",
            r"
    pragma solidity ^0.8.10;
    contract C {
            function hello() public {}
    }
   ",
        )
        .unwrap();
        let compiled = tmp.compile().unwrap();
        compiled.assert_success();

        tmp.artifacts_snapshot().unwrap().assert_artifacts_essentials_present();

        // modify A.sol
        tmp.add_source(
            "A",
            r#"
    pragma solidity ^0.8.10;
    import "./B.sol";
    contract A {
        function testExample() public {}
    }
   "#,
        )
        .unwrap();

        let compiler = ProjectCompiler::new(tmp.project()).unwrap();
        let state = compiler.preprocess().unwrap();
        let sources = state.sources.sources();

        let cache = state.cache.as_cached().unwrap();

        // 2 clean sources
        assert_eq!(cache.cache.artifacts_len(), 2);
        assert!(cache.cache.all_artifacts_exist());
        assert_eq!(cache.dirty_sources.len(), 1);

        let len = sources.values().map(|v| v.len()).sum::<usize>();
        // single solc
        assert_eq!(len, 1);

        let filtered = &sources.values().next().unwrap().values().next().unwrap();

        // 3 contracts total
        assert_eq!(filtered.0.len(), 3);
        // A is modified
        assert_eq!(filtered.dirty().count(), 1);
        assert!(filtered.dirty_files().next().unwrap().ends_with("A.sol"));

        let state = state.compile().unwrap();
        assert_eq!(state.output.sources.len(), 1);
        for (f, source) in state.output.sources.sources() {
            if f.ends_with("A.sol") {
                assert!(source.ast.is_some());
            } else {
                assert!(source.ast.is_none());
            }
        }

        assert_eq!(state.output.contracts.len(), 1);
        let (a, c) = state.output.contracts_iter().next().unwrap();
        assert_eq!(a, "A");
        assert!(c.abi.is_some() && c.evm.is_some());

        let state = state.write_artifacts().unwrap();
        assert_eq!(state.compiled_artifacts.as_ref().len(), 1);

        let out = state.write_cache().unwrap();

        let artifacts: Vec<_> = out.into_artifacts().collect();
        assert_eq!(artifacts.len(), 3);
        for (_, artifact) in artifacts {
            let c = artifact.into_contract_bytecode();
            assert!(c.abi.is_some() && c.bytecode.is_some() && c.deployed_bytecode.is_some());
        }

        tmp.artifacts_snapshot().unwrap().assert_artifacts_essentials_present();
    }

    #[test]
    #[ignore]
    fn can_compile_real_project() {
        init_tracing();
        let paths = ProjectPathsConfig::builder()
            .root("../../foundry-integration-tests/testdata/solmate")
            .build()
            .unwrap();
        let project = Project::builder().paths(paths).build(Default::default()).unwrap();
        let compiler = ProjectCompiler::new(&project).unwrap();
        let _out = compiler.compile().unwrap();
    }

    #[test]
    fn extra_output_cached() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/dapp-sample");
        let paths = ProjectPathsConfig::builder().sources(root.join("src")).lib(root.join("lib"));
        let mut project = TempProject::<MultiCompiler>::new(paths.clone()).unwrap();

        // Compile once without enabled extra output
        project.compile().unwrap();

        // Enable extra output of abi
        project.project_mut().artifacts =
            ConfigurableArtifacts::new([], [ContractOutputSelection::Abi]);

        // Ensure that abi appears after compilation and that we didn't recompile anything
        let abi_path = project.project().paths.artifacts.join("Dapp.sol/Dapp.abi.json");
        assert!(!abi_path.exists());
        let output = project.compile().unwrap();
        assert!(output.compiler_output.is_empty());
        assert!(abi_path.exists());
    }

    #[test]
    fn can_compile_leftovers_after_sparse() {
        let mut tmp = TempProject::<MultiCompiler, ConfigurableArtifacts>::dapptools().unwrap();

        tmp.add_source(
            "A",
            r#"
pragma solidity ^0.8.10;
import "./B.sol";
contract A {}
"#,
        )
        .unwrap();

        tmp.add_source(
            "B",
            r#"
pragma solidity ^0.8.10;
contract B {}
"#,
        )
        .unwrap();

        tmp.project_mut().sparse_output = Some(Box::new(|f: &Path| f.ends_with("A.sol")));
        let compiled = tmp.compile().unwrap();
        compiled.assert_success();
        assert_eq!(compiled.artifacts().count(), 1);

        tmp.project_mut().sparse_output = None;
        let compiled = tmp.compile().unwrap();
        compiled.assert_success();
        assert_eq!(compiled.artifacts().count(), 2);
    }
}
