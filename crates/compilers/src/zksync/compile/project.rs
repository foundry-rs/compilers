use crate::{
    artifact_output::Artifacts,
    cache::ArtifactsCache,
    compilers::{zksolc::ZkSolcCompiler, CompilerInput, CompilerSettings},
    error::Result,
    filter::SparseOutputFilter,
    output::Builds,
    report,
    resolver::{parse::SolData, GraphEdges},
    zksolc::input::ZkSolcVersionedInput,
    zksync::{
        self,
        artifact_output::zk::{ZkArtifactOutput, ZkContractArtifact},
        compile::output::{AggregatedCompilerOutput, ProjectCompileOutput},
    },
    Graph, Project, Sources,
};
use foundry_compilers_artifacts::{zksolc::CompilerOutput, SolcLanguage};
use semver::Version;
use std::{collections::HashMap, path::PathBuf, time::Instant};

/// A set of different Solc installations with their version and the sources to be compiled
pub(crate) type VersionedSources<L> = HashMap<L, HashMap<Version, Sources>>;

#[derive(Debug)]
pub struct ProjectCompiler<'a> {
    /// Contains the relationship of the source files and their imports
    edges: GraphEdges<SolData>,
    project: &'a Project<ZkSolcCompiler, ZkArtifactOutput>,
    /// how to compile all the sources
    sources: CompilerSources,
}

impl<'a> ProjectCompiler<'a> {
    /// Create a new `ProjectCompiler` to bootstrap the compilation process of the project's
    /// sources.
    pub fn new(project: &'a Project<ZkSolcCompiler, ZkArtifactOutput>) -> Result<Self> {
        Self::with_sources(project, project.paths.read_input_files()?)
    }

    /// Bootstraps the compilation process by resolving the dependency graph of all sources and the
    /// appropriate `Solc` -> `Sources` set as well as the compile mode to use (parallel,
    /// sequential)
    ///
    /// Multiple (`Solc` -> `Sources`) pairs can be compiled in parallel if the `Project` allows
    /// multiple `jobs`, see [`crate::Project::set_solc_jobs()`].
    pub fn with_sources(
        project: &'a Project<ZkSolcCompiler, ZkArtifactOutput>,
        sources: Sources,
    ) -> Result<Self> {
        let graph = Graph::resolve_sources(&project.paths, sources)?;
        let (sources, edges) = graph.into_sources_by_version(
            project.offline,
            &project.locked_versions,
            &project.compiler,
        )?;
        // If there are multiple different versions, and we can use multiple jobs we can compile
        // them in parallel.
        let sources = CompilerSources { sources };
        Ok(Self { edges, project, sources })
    }

    pub fn compile(self) -> Result<ProjectCompileOutput> {
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
    fn preprocess(self) -> Result<PreprocessedState<'a>> {
        trace!("preprocessing");
        let Self { edges, project, mut sources } = self;

        // convert paths on windows to ensure consistency with the `CompilerOutput` `solc` emits,
        // which is unix style `/`
        sources.slash_paths();

        let mut cache = ArtifactsCache::new(project, edges)?;
        // retain and compile only dirty sources and all their imports
        sources.filter(&mut cache);

        Ok(PreprocessedState { sources, cache })
    }
}

/// A series of states that comprise the [`ProjectCompiler::compile()`] state machine
///
/// The main reason is to debug all states individually
#[derive(Debug)]
struct PreprocessedState<'a> {
    /// Contains all the sources to compile.
    sources: CompilerSources,

    /// Cache that holds `CacheEntry` objects if caching is enabled and the project is recompiled
    cache: ArtifactsCache<'a, ZkArtifactOutput, ZkSolcCompiler>,
}

impl<'a> PreprocessedState<'a> {
    /// advance to the next state by compiling all sources
    fn compile(self) -> Result<CompiledState<'a>> {
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

/// Represents the state after `zksolc` was successfully invoked
#[derive(Debug)]
struct CompiledState<'a> {
    output: AggregatedCompilerOutput,
    cache: ArtifactsCache<'a, ZkArtifactOutput, ZkSolcCompiler>,
}

impl<'a> CompiledState<'a> {
    /// advance to the next state by handling all artifacts
    ///
    /// Writes all output contracts to disk if enabled in the `Project` and if the build was
    /// successful
    #[instrument(skip_all, name = "write-artifacts")]
    fn write_artifacts(self) -> Result<ArtifactsState<'a>> {
        let CompiledState { output, cache } = self;

        let project = cache.project();
        let ctx = cache.output_ctx();
        // write all artifacts via the handler but only if the build succeeded and project wasn't
        // configured with `no_artifacts == true`
        let compiled_artifacts = if project.no_artifacts {
            project.artifacts.zksync_output_to_artifacts(
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
            project.artifacts.zksync_output_to_artifacts(
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
            let artifacts = project.artifacts.zksync_on_output(
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
struct ArtifactsState<'a> {
    output: AggregatedCompilerOutput,
    cache: ArtifactsCache<'a, ZkArtifactOutput, ZkSolcCompiler>,
    compiled_artifacts: Artifacts<ZkContractArtifact>,
}

impl<'a> ArtifactsState<'a> {
    /// Writes the cache file
    ///
    /// this concludes the [`Project::compile()`] statemachine
    fn write_cache(self) -> Result<ProjectCompileOutput> {
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

        //project.artifacts_handler().handle_cached_artifacts(&cached_artifacts)?;
        //
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
struct CompilerSources {
    sources: VersionedSources<SolcLanguage>,
}

impl CompilerSources {
    /// Converts all `\\` separators to `/`
    ///
    /// This effectively ensures that `solc` can find imported files like `/src/Cheats.sol` in the
    /// VFS (the `ZkSolcInput` as json) under `src/Cheats.sol`.
    fn slash_paths(&mut self) {
        #[cfg(windows)]
        {
            use path_slash::PathBufExt;

            self.sources.values_mut().for_each(|versioned_sources| {
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
    }

    /// Filters out all sources that don't need to be compiled, see [`ArtifactsCache::filter`]
    fn filter(&mut self, cache: &mut ArtifactsCache<'_, ZkArtifactOutput, ZkSolcCompiler>) {
        cache.remove_dirty_sources();
        for versioned_sources in self.sources.values_mut() {
            for (version, sources) in versioned_sources {
                trace!("Filtering {} sources for {}", sources.len(), version);
                cache.filter(sources, version);
                trace!(
                    "Detected {} sources to compile {:?}",
                    sources.dirty().count(),
                    sources.dirty_files().collect::<Vec<_>>()
                );
            }
        }
    }

    /// Compiles all the files with `Solc`
    fn compile(
        self,
        cache: &mut ArtifactsCache<'_, ZkArtifactOutput, ZkSolcCompiler>,
    ) -> Result<AggregatedCompilerOutput> {
        let project = cache.project();
        let graph = cache.graph();

        let sparse_output = SparseOutputFilter::new(project.sparse_output.as_deref());

        // Include additional paths collected during graph resolution.
        let mut include_paths = project.paths.include_paths.clone();
        include_paths.extend(graph.include_paths().clone());

        let mut jobs = Vec::new();
        for (language, versioned_sources) in self.sources {
            for (version, sources) in versioned_sources {
                if sources.is_empty() {
                    // nothing to compile
                    trace!("skip {} for empty sources set", version);
                    continue;
                }

                // depending on the composition of the filtered sources, the output selection can be
                // optimized
                let mut opt_settings = project.settings.clone();
                let actually_dirty =
                    sparse_output.sparse_sources(&sources, &mut opt_settings, graph);

                if actually_dirty.is_empty() {
                    // nothing to compile for this particular language, all dirty files are in the
                    // other language set
                    trace!("skip {} run due to empty source set", version);
                    continue;
                }

                trace!("calling {} with {} sources {:?}", version, sources.len(), sources.keys());
                let zksync_settings = project
                    .settings
                    .clone()
                    .with_base_path(&project.paths.root)
                    .with_allow_paths(&project.paths.allowed_paths)
                    .with_include_paths(&include_paths)
                    .with_remappings(&project.paths.remappings);

                let mut input = ZkSolcVersionedInput::build(
                    sources,
                    zksync_settings,
                    language,
                    version.clone(),
                );

                input.strip_prefix(project.paths.root.as_path());

                jobs.push((input, actually_dirty));
            }
        }

        let results = compile_sequential(&project.compiler, jobs)?;

        let mut aggregated = AggregatedCompilerOutput::default();

        for (input, mut output, actually_dirty) in results {
            let version = input.version();

            // Mark all files as seen by the compiler
            for file in &actually_dirty {
                cache.compiler_seen(file);
            }

            let build_info = zksync::raw_build_info_new(&input, &output, project.build_info)?;

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
}

/// Compiles the input set sequentially and returns an aggregated set of the solc `CompilerOutput`s
fn compile_sequential(
    zksolc: &ZkSolcCompiler,
    jobs: Vec<(ZkSolcVersionedInput, Vec<PathBuf>)>,
) -> Result<Vec<(ZkSolcVersionedInput, CompilerOutput, Vec<PathBuf>)>> {
    jobs.into_iter()
        .map(|(input, actually_dirty)| {
            let start = Instant::now();
            report::compiler_spawn(
                &input.compiler_name(),
                input.version(),
                actually_dirty.as_slice(),
            );
            let output = zksolc.zksync_compile(&input)?;
            report::compiler_success(&input.compiler_name(), input.version(), &start.elapsed());

            Ok((input, output, actually_dirty))
        })
        .collect()
}
