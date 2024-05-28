//! Types to apply filter to input types

use crate::{
    artifacts::output_selection::OutputSelection,
    compilers::CompilerSettings,
    resolver::{parse::SolData, GraphEdges},
    Source, Sources,
};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::{self, Formatter},
    path::{Path, PathBuf},
};

/// A predicate property that determines whether a file satisfies a certain condition
pub trait FileFilter {
    /// The predicate function that should return if the given `file` should be included.
    fn is_match(&self, file: &Path) -> bool;
}

impl<F: Fn(&Path) -> bool> FileFilter for F {
    fn is_match(&self, file: &Path) -> bool {
        (self)(file)
    }
}

/// An [FileFilter] that matches all solidity files that end with `.t.sol`
#[derive(Default)]
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

/// Wrapper around a [FileFilter] that includes files matched by the inner filter and their link
/// references obtained from [GraphEdges].
pub struct SolcSparseFileFilter<T> {
    file_filter: T,
}

impl<T> SolcSparseFileFilter<T> {
    pub fn new(file_filter: T) -> Self {
        Self { file_filter }
    }
}

impl<T: FileFilter> FileFilter for SolcSparseFileFilter<T> {
    fn is_match(&self, file: &Path) -> bool {
        self.file_filter.is_match(file)
    }
}

impl<T: FileFilter> SparseOutputFileFilter<SolData> for SolcSparseFileFilter<T> {
    fn sparse_sources(&self, file: &Path, graph: &GraphEdges<SolData>) -> Vec<PathBuf> {
        if !self.file_filter.is_match(file) {
            return vec![];
        }

        let mut sources_to_compile = vec![file.to_path_buf()];
        for import in graph.imports(file) {
            if let Some(parsed) = graph.get_parsed_source(import) {
                if !parsed.libraries.is_empty() {
                    sources_to_compile.push(import.to_path_buf());
                }
            }
        }

        sources_to_compile
    }
}

/// This trait behaves in a similar way to [FileFilter] but used to configure [OutputSelection]
/// configuration. In certain cases, we might want to include some of the file dependencies into the
/// compiler output even if we might not be directly interested in them.
///
/// Example of such case is when we are compiling Solidity file containing link references and need
/// them to be included in the output to deploy needed libraries.
pub trait SparseOutputFileFilter<D>: FileFilter {
    /// Receives path to the file and resolved project sources graph.
    ///
    /// Returns a list of paths to the files that should be compiled with full output selection.
    ///
    /// Might return an empty list if no files should be compiled.
    fn sparse_sources(&self, file: &Path, graph: &GraphEdges<D>) -> Vec<PathBuf>;
}

/// A type that can apply a filter to a set of preprocessed [FilteredSources] in order to set sparse
/// output for specific files
#[derive(Default)]
pub enum SparseOutputFilter<D> {
    /// Sets the configured [OutputSelection] for dirty files only.
    ///
    /// In other words, we request the output of solc only for files that have been detected as
    /// _dirty_.
    #[default]
    Optimized,
    /// Apply an additional filter to [FilteredSources] to
    Custom(Box<dyn SparseOutputFileFilter<D>>),
}

impl<D> SparseOutputFilter<D> {
    /// While solc needs all the files to compile the actual _dirty_ files, we can tell solc to
    /// output everything for those dirty files as currently configured in the settings, but output
    /// nothing for the other files that are _not_ dirty.
    ///
    /// This will modify the [OutputSelection] of the [CompilerSettings] so that we explicitly
    /// select the files' output based on their state.
    ///
    /// This also takes the project's graph as input, this allows us to check if the files the
    /// filter matches depend on libraries that need to be linked
    pub fn sparse_sources<S: CompilerSettings>(
        &self,
        sources: FilteredSources,
        settings: &mut S,
        graph: &GraphEdges<D>,
    ) -> Sources {
        match self {
            SparseOutputFilter::Optimized => {
                if !sources.all_dirty() {
                    Self::optimize(&sources, settings)
                }
            }
            SparseOutputFilter::Custom(f) => {
                Self::apply_custom_filter(&sources, settings, graph, &**f)
            }
        };
        sources.into()
    }

    /// applies a custom filter and prunes the output of those source files for which the filter
    /// returns `false`.
    fn apply_custom_filter<S: CompilerSettings>(
        sources: &FilteredSources,
        settings: &mut S,
        graph: &GraphEdges<D>,
        f: &dyn SparseOutputFileFilter<D>,
    ) {
        let mut full_compilation = HashSet::new();

        // populate sources which need complete compilation with data from filter
        for (file, source) in sources.0.iter() {
            if source.is_dirty() {
                for source in f.sparse_sources(file, graph) {
                    full_compilation.insert(source);
                }
            }
        }

        settings.update_output_selection(|selection| {
            trace!("optimizing output selection with custom filter");
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
        })
    }

    /// prunes all clean sources and only selects an output for dirty sources
    fn optimize<S: CompilerSettings>(sources: &FilteredSources, settings: &mut S) {
        // settings can be optimized
        trace!(
            "optimizing output selection for {}/{} sources",
            sources.clean().count(),
            sources.len()
        );

        settings.update_output_selection(|selection| {
            let selection = selection.as_mut();
            let default = selection
                .remove("*")
                .unwrap_or_else(OutputSelection::default_file_output_selection);

            let optimized = S::minimal_output_selection();

            for (file, kind) in sources.0.iter() {
                match kind {
                    SourceCompilationKind::Complete(_) => {
                        selection.insert(format!("{}", file.display()), default.clone());
                    }
                    SourceCompilationKind::Optimized(_) => {
                        trace!("using pruned output selection for {}", file.display());
                        selection.insert(format!("{}", file.display()), optimized.clone());
                    }
                }
            }
        });
    }
}

impl<D> From<Box<dyn SparseOutputFileFilter<D>>> for SparseOutputFilter<D> {
    fn from(f: Box<dyn SparseOutputFileFilter<D>>) -> Self {
        SparseOutputFilter::Custom(f)
    }
}

impl<D> fmt::Debug for SparseOutputFilter<D> {
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
        FilteredSources(
            s.into_iter().map(|(key, val)| (key, SourceCompilationKind::Complete(val))).collect(),
        )
    }
}

impl From<BTreeMap<PathBuf, SourceCompilationKind>> for FilteredSources {
    fn from(s: BTreeMap<PathBuf, SourceCompilationKind>) -> Self {
        FilteredSources(s)
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
            SourceCompilationKind::Complete(s) => s,
            SourceCompilationKind::Optimized(s) => s,
        }
    }

    /// Consumes the type and returns the underlying source
    pub fn into_source(self) -> Source {
        match self {
            SourceCompilationKind::Complete(s) => s,
            SourceCompilationKind::Optimized(s) => s,
        }
    }

    /// Whether this file should be compiled with full output selection
    pub fn is_dirty(&self) -> bool {
        matches!(self, SourceCompilationKind::Complete(_))
    }
}
