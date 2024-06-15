//! Types to apply filter to input types

use crate::{
    compilers::{multi::MultiCompilerParsedSource, CompilerSettings, ParsedSource},
    resolver::{parse::SolData, GraphEdges},
    Source, Sources,
};
use foundry_compilers_artifacts::output_selection::OutputSelection;
use std::{
    collections::{BTreeMap, HashSet},
    fmt::{self, Formatter},
    path::{Path, PathBuf},
};

/// A predicate property that determines whether a file satisfies a certain condition
pub trait FileFilter: dyn_clone::DynClone + Send + Sync {
    /// The predicate function that should return if the given `file` should be included.
    fn is_match(&self, file: &Path) -> bool;
}

dyn_clone::clone_trait_object!(FileFilter);

impl<F: Fn(&Path) -> bool + Clone + Send + Sync> FileFilter for F {
    fn is_match(&self, file: &Path) -> bool {
        (self)(file)
    }
}

/// An [FileFilter] that matches all solidity files that end with `.t.sol`
#[derive(Default, Clone)]
pub struct TestFileFilter {
    _priv: (),
}

impl fmt::Debug for TestFileFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestFileFilter").finish()
    }
}

impl fmt::Display for TestFileFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("TestFileFilter")
    }
}

impl FileFilter for TestFileFilter {
    fn is_match(&self, file: &Path) -> bool {
        file.file_name().and_then(|s| s.to_str()).map(|s| s.ends_with(".t.sol")).unwrap_or_default()
    }
}

pub trait MaybeSolData {
    fn sol_data(&self) -> Option<&SolData>;
}

impl MaybeSolData for SolData {
    fn sol_data(&self) -> Option<&SolData> {
        Some(self)
    }
}

impl MaybeSolData for MultiCompilerParsedSource {
    fn sol_data(&self) -> Option<&SolData> {
        match self {
            Self::Solc(data) => Some(data),
            _ => None,
        }
    }
}

/// A type that can apply a filter to a set of preprocessed [FilteredSources] in order to set sparse
/// output for specific files
#[derive(Default)]
pub enum SparseOutputFilter<'a> {
    /// Sets the configured [OutputSelection] for dirty files only.
    ///
    /// In other words, we request the output of solc only for files that have been detected as
    /// _dirty_.
    #[default]
    Optimized,
    /// Apply an additional filter to [FilteredSources] to
    Custom(&'a dyn FileFilter),
}

impl<'a> SparseOutputFilter<'a> {
    pub fn new(filter: Option<&'a dyn FileFilter>) -> Self {
        if let Some(f) = filter {
            SparseOutputFilter::Custom(f)
        } else {
            SparseOutputFilter::Optimized
        }
    }

    /// While solc needs all the files to compile the actual _dirty_ files, we can tell solc to
    /// output everything for those dirty files as currently configured in the settings, but output
    /// nothing for the other files that are _not_ dirty.
    ///
    /// This will modify the [OutputSelection] of the [CompilerSettings] so that we explicitly
    /// select the files' output based on their state.
    ///
    /// This also takes the project's graph as input, this allows us to check if the files the
    /// filter matches depend on libraries that need to be linked
    pub fn sparse_sources<D: ParsedSource, S: CompilerSettings>(
        &self,
        sources: FilteredSources,
        settings: &mut S,
        graph: &GraphEdges<D>,
    ) -> (Sources, Vec<PathBuf>) {
        let mut full_compilation: HashSet<PathBuf> = sources
            .dirty_files()
            .flat_map(|file| {
                // If we have a custom filter and file does not match, we skip it.
                if let Self::Custom(f) = self {
                    if !f.is_match(file) {
                        return vec![];
                    }
                }

                // Collect compilation dependencies for sources needing compilation.
                let mut required_sources = vec![file.clone()];
                if let Some(data) = graph.get_parsed_source(file) {
                    let imports = graph.imports(file).into_iter().filter_map(|import| {
                        graph.get_parsed_source(import).map(|data| (import.as_path(), data))
                    });
                    for import in data.compilation_dependencies(imports) {
                        let import = import.to_path_buf();

                        #[cfg(windows)]
                        let import = {
                            use path_slash::PathBufExt;

                            PathBuf::from(import.to_slash_lossy().to_string())
                        };

                        required_sources.push(import);
                    }
                }

                required_sources
            })
            .collect();

        // Remove clean sources, those will be read from cache.
        full_compilation.retain(|file| sources.0.get(file).map_or(false, |s| s.is_dirty()));

        settings.update_output_selection(|selection| {
            trace!(
                "optimizing output selection for {} sources",
                sources.len() - full_compilation.len()
            );
            let default_selection = selection
                .as_mut()
                .remove("*")
                .unwrap_or_else(OutputSelection::default_file_output_selection);

            // set output selections
            for file in sources.0.keys() {
                let key = format!("{}", file.display());
                if full_compilation.contains(file) {
                    selection.as_mut().insert(key, default_selection.clone());
                } else {
                    selection.as_mut().insert(key, OutputSelection::empty_file_output_select());
                }
            }
        });

        (sources.into(), full_compilation.into_iter().collect())
    }
}

impl<'a> fmt::Debug for SparseOutputFilter<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SparseOutputFilter::Optimized => f.write_str("Optimized"),
            SparseOutputFilter::Custom(_) => f.write_str("Custom"),
        }
    }
}

/// Container type for a mapping from source path to [SourceCompilationKind]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FilteredSources(pub BTreeMap<PathBuf, SourceCompilationKind>);

impl FilteredSources {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if no sources should have optimized output selection.
    pub fn all_dirty(&self) -> bool {
        self.0.values().all(|s| s.is_dirty())
    }

    /// Returns all entries that should not be optimized.
    pub fn dirty(&self) -> impl Iterator<Item = (&PathBuf, &SourceCompilationKind)> + '_ {
        self.0.iter().filter(|(_, s)| s.is_dirty())
    }

    /// Returns all entries that should be optimized.
    pub fn clean(&self) -> impl Iterator<Item = (&PathBuf, &SourceCompilationKind)> + '_ {
        self.0.iter().filter(|(_, s)| !s.is_dirty())
    }

    /// Returns all files that should not be optimized.
    pub fn dirty_files(&self) -> impl Iterator<Item = &PathBuf> + fmt::Debug + '_ {
        self.0.iter().filter_map(|(k, s)| s.is_dirty().then_some(k))
    }
}

impl From<FilteredSources> for Sources {
    fn from(sources: FilteredSources) -> Self {
        sources.0.into_iter().map(|(k, v)| (k, v.into_source())).collect()
    }
}

impl From<Sources> for FilteredSources {
    fn from(s: Sources) -> Self {
        Self(
            s.into_iter().map(|(key, val)| (key, SourceCompilationKind::Complete(val))).collect(),
        )
    }
}

impl From<BTreeMap<PathBuf, SourceCompilationKind>> for FilteredSources {
    fn from(s: BTreeMap<PathBuf, SourceCompilationKind>) -> Self {
        Self(s)
    }
}

impl AsRef<BTreeMap<PathBuf, SourceCompilationKind>> for FilteredSources {
    fn as_ref(&self) -> &BTreeMap<PathBuf, SourceCompilationKind> {
        &self.0
    }
}

impl AsMut<BTreeMap<PathBuf, SourceCompilationKind>> for FilteredSources {
    fn as_mut(&mut self) -> &mut BTreeMap<PathBuf, SourceCompilationKind> {
        &mut self.0
    }
}

/// Represents the state of a filtered [Source]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SourceCompilationKind {
    /// We need a complete compilation output for the source.
    Complete(Source),
    /// A source for which we don't need a complete output and want to optimize its compilation by
    /// reducing output selection.
    Optimized(Source),
}

impl SourceCompilationKind {
    /// Returns the underlying source
    pub fn source(&self) -> &Source {
        match self {
            Self::Complete(s) => s,
            Self::Optimized(s) => s,
        }
    }

    /// Consumes the type and returns the underlying source
    pub fn into_source(self) -> Source {
        match self {
            Self::Complete(s) => s,
            Self::Optimized(s) => s,
        }
    }

    /// Whether this file should be compiled with full output selection
    pub fn is_dirty(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}
