//! Support for compiling contracts.

use crate::{
    artifact_output::{ArtifactOutput, Artifacts, OutputContext},
    artifacts::Sources,
    buildinfo::RawBuildInfo,
    cache::{CacheEntry, CompilerCache, GroupedSources},
    error::Result,
    filter::{FilteredSources, SourceCompilationKind},
    output::Builds,
    resolver::{parse::SolData, GraphEdges},
    solc::SolcCompiler,
    utils,
    zksolc::settings::ZkSolcSettings,
    zksync::{self, artifact_output::zk::ZkContractArtifact},
    CompilerSettings, Graph, Project, ProjectPathsConfig, Source,
};
use foundry_compilers_artifacts::SolcLanguage;
use semver::Version;
use std::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet, hash_map, HashMap, HashSet},
    path::{Path, PathBuf},
};

/// The file name of the default cache file
pub const ZKSYNC_SOLIDITY_FILES_CACHE_FILENAME: &str = "zksync-solidity-files-cache.json";

// OVERRIDES
// Zksync specific overrides to generalized methods
// TODO: Most of these are needed because we use dedicated paths for zksync stuff and they live
// along the general paths in `ProjectPathsConfig`. This is pretty error prone as we need to
// detect where the paths are used and override with the zksync ones, and they are very easy to
// miss. A way to solve this would be to delete the zksync specific config and use a dedicated
// `Project` instead, having helpers to create that Project from the `solc` one.
/// Override of CompilerCache::read_joined to use zksync paths
pub fn zksync_override_compiler_cache_read_joined<L>(
    paths: &ProjectPathsConfig<L>,
) -> Result<CompilerCache<ZkSolcSettings>> {
    let mut cache = CompilerCache::read(&paths.zksync_cache)?;
    cache.join_entries(&paths.root).join_artifacts_files(&paths.zksync_artifacts);
    Ok(cache)
}

/// A helper abstraction over the [`SolFilesCache`] used to determine what files need to compiled
/// and which `Artifacts` can be reused.
#[derive(Debug)]
pub(crate) struct ArtifactsCacheInner<'a, T: ArtifactOutput> {
    /// The preexisting cache file.
    pub cache: CompilerCache<ZkSolcSettings>,

    /// All already existing artifacts.
    pub cached_artifacts: Artifacts<ZkContractArtifact>,

    /// All already existing build infos.
    pub cached_builds: Builds<SolcLanguage>,

    /// Relationship between all the files.
    pub edges: GraphEdges<SolData>,

    /// The project.
    pub project: &'a Project<SolcCompiler, T>,

    /// Files that were invalidated and removed from cache.
    /// Those are not grouped by version and purged completely.
    pub dirty_sources: HashSet<PathBuf>,

    /// Artifact+version pairs which are in scope for each solc version.
    ///
    /// Only those files will be included into cached artifacts list for each version.
    pub sources_in_scope: GroupedSources,

    /// The file hashes.
    pub content_hashes: HashMap<PathBuf, String>,
}

impl<'a, T: ArtifactOutput> ArtifactsCacheInner<'a, T> {
    /// Creates a new cache entry for the file
    fn create_cache_entry(&mut self, file: PathBuf, source: &Source) {
        let imports = self
            .edges
            .imports(&file)
            .into_iter()
            .map(|import| utils::source_name(import, self.project.root()).to_path_buf())
            .collect();

        let entry = CacheEntry {
            last_modification_date: CacheEntry::<ZkSolcSettings>::read_last_modification_date(
                &file,
            )
            .unwrap_or_default(),
            content_hash: source.content_hash(),
            source_name: utils::source_name(&file, self.project.root()).into(),
            compiler_settings: self.project.zksync_zksolc_config.settings.clone(),
            imports,
            version_requirement: self.edges.version_requirement(&file).map(|v| v.to_string()),
            // artifacts remain empty until we received the compiler output
            artifacts: Default::default(),
            seen_by_compiler: false,
        };

        self.cache.files.insert(file, entry.clone());
    }

    /// Returns the set of [Source]s that need to be compiled to produce artifacts for requested
    /// input.
    ///
    /// Source file may have one of the two [SourceCompilationKind]s:
    /// 1. [SourceCompilationKind::Complete] - the file has been modified or compiled with different
    ///    settings and its cache is invalidated. For such sources we request full data needed for
    ///    artifact construction.
    /// 2. [SourceCompilationKind::Optimized] - the file is not dirty, but is imported by a dirty
    ///    file and thus will be processed by solc. For such files we don't need full data, so we
    ///    are marking them as clean to optimize output selection later.
    fn filter(&mut self, sources: Sources, version: &Version) -> FilteredSources {
        // sources that should be passed to compiler.
        let mut compile_complete = BTreeSet::new();
        let mut compile_optimized = BTreeSet::new();

        for (file, source) in sources.iter() {
            self.sources_in_scope.insert(file.clone(), version.clone());

            // If we are missing artifact for file, compile it.
            if self.is_missing_artifacts(file, version) {
                compile_complete.insert(file.clone());
            }

            // Ensure that we have a cache entry for all sources.
            if !self.cache.files.contains_key(file) {
                self.create_cache_entry(file.clone(), source);
            }
        }

        // Prepare optimization by collecting sources which are imported by files requiring complete
        // compilation.
        for source in &compile_complete {
            for import in self.edges.imports(source) {
                if !compile_complete.contains(import) {
                    compile_optimized.insert(import.clone());
                }
            }
        }

        let filtered = sources
            .into_iter()
            .filter_map(|(file, source)| {
                if compile_complete.contains(&file) {
                    Some((file, SourceCompilationKind::Complete(source)))
                } else if compile_optimized.contains(&file) {
                    Some((file, SourceCompilationKind::Optimized(source)))
                } else {
                    None
                }
            })
            .collect();

        FilteredSources(filtered)
    }

    /// Returns whether we are missing artifacts for the given file and version.
    fn is_missing_artifacts(&self, file: &Path, version: &Version) -> bool {
        let Some(entry) = self.cache.entry(file) else {
            trace!("missing cache entry");
            return true;
        };

        // only check artifact's existence if the file generated artifacts.
        // e.g. a solidity file consisting only of import statements (like interfaces that
        // re-export) do not create artifacts
        if entry.seen_by_compiler && entry.artifacts.is_empty() {
            trace!("no artifacts");
            return false;
        }

        if !entry.contains_version(version) {
            trace!("missing linked artifacts",);
            return true;
        }

        if entry.artifacts_for_version(version).any(|artifact| {
            let missing_artifact = !self.cached_artifacts.has_artifact(&artifact.path);
            if missing_artifact {
                trace!("missing artifact \"{}\"", artifact.path.display());
            }
            missing_artifact
        }) {
            return true;
        }

        false
    }

    // Walks over all cache entires, detects dirty files and removes them from cache.
    fn find_and_remove_dirty(&mut self) {
        fn populate_dirty_files<D>(
            file: &Path,
            dirty_files: &mut HashSet<PathBuf>,
            edges: &GraphEdges<D>,
        ) {
            for file in edges.importers(file) {
                // If file is marked as dirty we either have already visited it or it was marked as
                // dirty initially and will be visited at some point later.
                if !dirty_files.contains(file) {
                    dirty_files.insert(file.to_path_buf());
                    populate_dirty_files(file, dirty_files, edges);
                }
            }
        }

        // Iterate over existing cache entries.
        let files = self.cache.files.keys().cloned().collect::<HashSet<_>>();

        let mut sources = BTreeMap::new();

        // Read all sources, marking entries as dirty on I/O errors.
        for file in &files {
            let Ok(source) = Source::read(file) else {
                self.dirty_sources.insert(file.clone());
                continue;
            };
            sources.insert(file.clone(), source);
        }

        // Build a temporary graph for walking imports. We need this because `self.edges`
        // only contains graph data for in-scope sources but we are operating on cache entries.
        if let Ok(graph) = Graph::<SolData>::resolve_sources(&self.project.paths, sources) {
            let (sources, edges) = graph.into_sources();

            // Calculate content hashes for later comparison.
            self.fill_hashes(&sources);

            // Pre-add all sources that are guaranteed to be dirty
            for file in sources.keys() {
                if self.is_dirty_impl(file) {
                    self.dirty_sources.insert(file.clone());
                }
            }

            // Perform DFS to find direct/indirect importers of dirty files.
            for file in self.dirty_sources.clone().iter() {
                populate_dirty_files(file, &mut self.dirty_sources, &edges);
            }
        } else {
            // Purge all sources on graph resolution error.
            self.dirty_sources.extend(files);
        }

        // Remove all dirty files from cache.
        for file in &self.dirty_sources {
            debug!("removing dirty file from cache: {}", file.display());
            self.cache.remove(file);
        }
    }

    fn is_dirty_impl(&self, file: &Path) -> bool {
        let Some(hash) = self.content_hashes.get(file) else {
            trace!("missing content hash");
            return true;
        };

        let Some(entry) = self.cache.entry(file) else {
            trace!("missing cache entry");
            return true;
        };

        if entry.content_hash != *hash {
            trace!("content hash changed");
            return true;
        }

        if !self.project.zksync_zksolc_config.settings.can_use_cached(&entry.compiler_settings) {
            trace!("zksolc config not compatible");
            return true;
        }

        /*
        // If any requested extra files are missing for any artifact, mark source as dirty to
        // generate them
        for artifacts in self.cached_artifacts.values() {
            for artifacts in artifacts.values() {
                for artifact_file in artifacts {
                    if self.project.artifacts_handler().is_dirty(artifact_file).unwrap_or(true) {
                        return true;
                    }
                }
            }
        }
        */

        // all things match, can be reused
        false
    }

    /// Adds the file's hashes to the set if not set yet
    fn fill_hashes(&mut self, sources: &Sources) {
        for (file, source) in sources {
            if let hash_map::Entry::Vacant(entry) = self.content_hashes.entry(file.clone()) {
                entry.insert(source.content_hash());
            }
        }
    }
}

/// Abstraction over configured caching which can be either non-existent or an already loaded cache
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum ArtifactsCache<'a, T: ArtifactOutput> {
    /// Cache nothing on disk
    Ephemeral(GraphEdges<SolData>, &'a Project<SolcCompiler, T>),
    /// Handles the actual cached artifacts, detects artifacts that can be reused
    Cached(ArtifactsCacheInner<'a, T>),
}

impl<'a, T: ArtifactOutput> ArtifactsCache<'a, T> {
    pub fn new(project: &'a Project<SolcCompiler, T>, edges: GraphEdges<SolData>) -> Result<Self> {
        /// Returns the [SolFilesCache] to use
        ///
        /// Returns a new empty cache if the cache does not exist or `invalidate_cache` is set.
        fn get_cache<T: ArtifactOutput>(
            project: &Project<SolcCompiler, T>,
            invalidate_cache: bool,
        ) -> CompilerCache<ZkSolcSettings> {
            // the currently configured paths
            let paths = project.paths.zksync_paths_relative();

            if !invalidate_cache && zksync::project_cache_path(project).exists() {
                if let Ok(cache) = zksync_override_compiler_cache_read_joined(&project.paths) {
                    if cache.paths == paths {
                        // unchanged project paths
                        return cache;
                    }
                }
            }

            // new empty cache
            CompilerCache::<ZkSolcSettings>::new(Default::default(), paths)
        }

        let cache = if project.cached {
            // we only read the existing cache if we were able to resolve the entire graph
            // if we failed to resolve an import we invalidate the cache so don't get any false
            // positives
            let invalidate_cache = !edges.unresolved_imports().is_empty();

            // read the cache file if it already exists
            let mut cache = get_cache(project, invalidate_cache);

            cache.remove_missing_files();

            // read all artifacts
            let cached_artifacts = if project.paths.zksync_artifacts.exists() {
                trace!("reading artifacts from cache...");
                // if we failed to read the whole set of artifacts we use an empty set
                let artifacts = cache.read_artifacts().unwrap_or_default();
                trace!("read {} artifacts from cache", artifacts.artifact_files().count());
                artifacts
            } else {
                Default::default()
            };

            trace!("reading build infos from cache...");
            let cached_builds = cache.read_builds(&project.paths.build_infos).unwrap_or_default();

            let cache = ArtifactsCacheInner {
                cache,
                cached_artifacts,
                cached_builds,
                edges,
                project,
                dirty_sources: Default::default(),
                content_hashes: Default::default(),
                sources_in_scope: Default::default(),
            };

            ArtifactsCache::Cached(cache)
        } else {
            // nothing to cache
            ArtifactsCache::Ephemeral(edges, project)
        };

        Ok(cache)
    }

    /// Returns the graph data for this project
    pub fn graph(&self) -> &GraphEdges<SolData> {
        match self {
            ArtifactsCache::Ephemeral(graph, _) => graph,
            ArtifactsCache::Cached(inner) => &inner.edges,
        }
    }

    #[cfg(test)]
    #[allow(unused)]
    #[doc(hidden)]
    // only useful for debugging for debugging purposes
    pub fn as_cached(&self) -> Option<&ArtifactsCacheInner<'a, T>> {
        match self {
            ArtifactsCache::Ephemeral(_, _) => None,
            ArtifactsCache::Cached(cached) => Some(cached),
        }
    }

    pub fn output_ctx(&self) -> OutputContext<'_> {
        match self {
            ArtifactsCache::Ephemeral(_, _) => Default::default(),
            ArtifactsCache::Cached(inner) => OutputContext::new(&inner.cache),
        }
    }

    pub fn project(&self) -> &'a Project<SolcCompiler, T> {
        match self {
            ArtifactsCache::Ephemeral(_, project) => project,
            ArtifactsCache::Cached(cache) => cache.project,
        }
    }

    /// Adds the file's hashes to the set if not set yet
    pub fn remove_dirty_sources(&mut self) {
        match self {
            ArtifactsCache::Ephemeral(_, _) => {}
            ArtifactsCache::Cached(cache) => cache.find_and_remove_dirty(),
        }
    }

    /// Filters out those sources that don't need to be compiled
    pub fn filter(&mut self, sources: Sources, version: &Version) -> FilteredSources {
        match self {
            ArtifactsCache::Ephemeral(_, _) => sources.into(),
            ArtifactsCache::Cached(cache) => cache.filter(sources, version),
        }
    }

    /// Consumes the `Cache`, rebuilds the `SolFileCache` by merging all artifacts that were
    /// filtered out in the previous step (`Cache::filtered`) and the artifacts that were just
    /// compiled and written to disk `written_artifacts`.
    ///
    /// Returns all the _cached_ artifacts.
    pub fn consume(
        self,
        written_artifacts: &Artifacts<ZkContractArtifact>,
        written_build_infos: &Vec<RawBuildInfo<SolcLanguage>>,
        write_to_disk: bool,
    ) -> Result<(Artifacts<ZkContractArtifact>, Builds<SolcLanguage>)> {
        let ArtifactsCache::Cached(cache) = self else {
            trace!("no cache configured, ephemeral");
            return Ok(Default::default());
        };

        let ArtifactsCacheInner {
            mut cache,
            mut cached_artifacts,
            cached_builds,
            dirty_sources,
            sources_in_scope,
            project,
            ..
        } = cache;

        // Remove cached artifacts which are out of scope, dirty or appear in `written_artifacts`.
        cached_artifacts.0.retain(|file, artifacts| {
            let file = Path::new(file);
            artifacts.retain(|name, artifacts| {
                artifacts.retain(|artifact| {
                    let version = &artifact.version;

                    if !sources_in_scope.contains(file, version) {
                        return false;
                    }
                    if dirty_sources.contains(file) {
                        return false;
                    }
                    if written_artifacts.find_artifact(file, name, version).is_some() {
                        return false;
                    }
                    true
                });
                !artifacts.is_empty()
            });
            !artifacts.is_empty()
        });

        // Update cache entries with newly written artifacts. We update data for any artifacts as
        // `written_artifacts` always contain the most recent data.
        for (file, artifacts) in written_artifacts.as_ref() {
            let file_path = Path::new(file);
            // Only update data for existing entries, we should have entries for all in-scope files
            // by now.
            if let Some(entry) = cache.files.get_mut(file_path) {
                entry.merge_artifacts(artifacts);
            }
        }

        for build_info in written_build_infos {
            cache.builds.insert(build_info.id.clone());
        }

        // write to disk
        if write_to_disk {
            // make all `CacheEntry` paths relative to the project root and all artifact
            // paths relative to the artifact's directory
            cache
                .strip_entries_prefix(project.root())
                .strip_artifact_files_prefixes(zksync::project_artifacts_path(project));
            cache.write(zksync::project_cache_path(project))?;
        }

        Ok((cached_artifacts, cached_builds))
    }

    /// Marks the cached entry as seen by the compiler, if it's cached.
    pub fn compiler_seen(&mut self, file: &Path) {
        if let ArtifactsCache::Cached(cache) = self {
            if let Some(entry) = cache.cache.entry_mut(file) {
                entry.seen_by_compiler = true;
            }
        }
    }
}
