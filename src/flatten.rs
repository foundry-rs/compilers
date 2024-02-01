use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::{Path, PathBuf},
};

use crate::{
    artifacts::{
        ast::SourceLocation,
        visitor::{Visitor, Walk},
        ContractDefinitionPart, ExternalInlineAssemblyReference, Identifier, IdentifierPath,
        MemberAccess, Source, SourceUnit, SourceUnitPart, Sources,
    },
    error::SolcError,
    utils, Graph, Project, ProjectCompileOutput, ProjectPathsConfig, Result,
};

/// Alternative of `SourceLocation` which includes path of the file.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct ItemLocation {
    path: PathBuf,
    start: usize,
    end: usize,
}

impl ItemLocation {
    fn try_from_source_loc(src: &SourceLocation, path: PathBuf) -> Option<Self> {
        let start = src.start?;
        let end = start + src.length?;

        Some(ItemLocation { path, start, end })
    }

    fn length(&self) -> usize {
        self.end - self.start
    }
}

/// Visitor exploring AST and collecting all references to declarations via `Identifier` and
/// `IdentifierPath` nodes.
///
/// It also collects `MemberAccess` parts. So, if we have `X.Y` expression, loc and AST ID will be
/// saved for Y only.
///
/// That way, even if we have a long `MemberAccess` expression (a.b.c.d) then the first member (a)
/// will be collected as either `Identifier` or `IdentifierPath`, and all subsequent parts (b, c, d)
/// will be collected as `MemberAccess` parts.
struct ReferencesCollector {
    path: PathBuf,
    references: HashMap<isize, HashSet<ItemLocation>>,
}

impl ReferencesCollector {
    fn process_referenced_declaration(&mut self, id: isize, src: &SourceLocation) {
        if let Some(loc) = ItemLocation::try_from_source_loc(src, self.path.clone()) {
            self.references.entry(id).or_default().insert(loc);
        }
    }
}

impl Visitor for ReferencesCollector {
    fn visit_identifier(&mut self, identifier: &Identifier) {
        if let Some(id) = identifier.referenced_declaration {
            self.process_referenced_declaration(id, &identifier.src);
        }
    }

    fn visit_identifier_path(&mut self, path: &IdentifierPath) {
        self.process_referenced_declaration(path.referenced_declaration, &path.src);
    }

    fn visit_member_access(&mut self, access: &MemberAccess) {
        if let Some(referenced_declaration) = access.referenced_declaration {
            if let (Some(src_start), Some(src_length)) = (access.src.start, access.src.length) {
                let name_length = access.member_name.len();
                // Accessed member name is in the last name.len() symbols of the expression.
                let start = src_start + src_length - name_length;
                let end = start + name_length;

                self.references.entry(referenced_declaration).or_default().insert(ItemLocation {
                    start,
                    end,
                    path: self.path.to_path_buf(),
                });
            }
        }
    }

    fn visit_external_assembly_reference(&mut self, reference: &ExternalInlineAssemblyReference) {
        self.process_referenced_declaration(reference.declaration as isize, &reference.src);
    }
}

/// Updates to be applied to the sources.
/// source_path -> (start, end, new_value)
type Updates = HashMap<PathBuf, HashSet<(usize, usize, String)>>;

struct FlatteningResult<'a> {
    /// Updated source in the order they shoud be written to the output file.
    sources: Vec<String>,
    /// Pragmas that should be present in the target file.
    pragmas: Vec<&'a str>,
    /// License identifier that should be present in the target file.
    license: Option<&'a str>,
}

impl<'a> FlatteningResult<'a> {
    fn new(
        flattener: &Flattener,
        mut updates: Updates,
        pragmas: Vec<&'a str>,
        license: Option<&'a str>,
    ) -> Self {
        let mut sources = Vec::new();

        for path in &flattener.ordered_sources {
            let mut content = flattener.sources.get(path).unwrap().content.as_bytes().to_vec();
            let mut offset: isize = 0;
            if let Some(updates) = updates.remove(path) {
                let mut updates = updates.iter().collect::<Vec<_>>();
                updates.sort_by_key(|(start, _, _)| *start);
                for (start, end, new_value) in updates {
                    let start = (*start as isize + offset) as usize;
                    let end = (*end as isize + offset) as usize;

                    content.splice(start..end, new_value.bytes());
                    offset += new_value.len() as isize - (end - start) as isize;
                }
            }
            sources.push(String::from_utf8(content).unwrap());
        }

        Self { sources, pragmas, license }
    }

    fn get_flattened_target(&self) -> String {
        let mut result = String::new();

        if let Some(license) = &self.license {
            result.push_str(&format!("{}\n", license));
        }
        for pragma in &self.pragmas {
            result.push_str(&format!("{}\n", pragma));
        }
        for source in &self.sources {
            result.push_str(&format!("{}\n\n", source));
        }

        format!("{}\n", utils::RE_THREE_OR_MORE_NEWLINES.replace_all(&result, "\n\n").trim())
    }
}

/// Context for flattening. Stores all sources and ASTs that are in scope of the flattening target.
pub struct Flattener {
    /// Target file to flatten.
    target: PathBuf,
    /// Sources including only target and it dependencies (imports of any depth).
    sources: Sources,
    /// Vec of (path, ast) pairs.
    asts: Vec<(PathBuf, SourceUnit)>,
    /// Sources in the order they should be written to the output file.
    ordered_sources: Vec<PathBuf>,
}

impl Flattener {
    /// Compilation output is expected to contain all artifacts for all sources.
    /// Flattener caller is expected to resolve all imports of target file, compile them and pass
    /// into this function.
    pub fn new(project: &Project, output: &ProjectCompileOutput, target: &Path) -> Result<Self> {
        let input_files = output
            .artifacts_with_files()
            .map(|(file, _, _)| PathBuf::from(file))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let sources = Source::read_all_files(input_files)?;
        let graph = Graph::resolve_sources(&project.paths, sources)?;

        let ordered_deps = collect_ordered_deps(&target.to_path_buf(), &project.paths, &graph)?;

        let sources = Source::read_all(&ordered_deps)?;

        // Convert all ASTs from artifacts to strongly typed ASTs
        let mut asts: Vec<(PathBuf, SourceUnit)> = Vec::new();
        for (path, ast) in output.artifacts_with_files().filter_map(|(path, _, artifact)| {
            if let Some(ast) = artifact.ast.as_ref() {
                if sources.contains_key(&PathBuf::from(path)) {
                    Some((path, ast))
                } else {
                    None
                }
            } else {
                None
            }
        }) {
            asts.push((PathBuf::from(path), serde_json::from_str(&serde_json::to_string(ast)?)?));
        }

        Ok(Flattener { target: target.into(), sources, asts, ordered_sources: ordered_deps })
    }

    /// Flattens target file and returns the result as a string
    ///
    /// Flattening process includes following steps:
    /// 1. Find all file-level definitions and rename references to them via aliased or qualified
    ///    imports.
    /// 2. Find all duplicates among file-level definitions and rename them to avoid conflicts.
    /// 3. Remove all imports.
    /// 4. Remove all pragmas except for the ones in the target file.
    /// 5. Remove all license identifiers except for the one in the target file.
    pub fn flatten(&self) -> String {
        let mut updates = Updates::new();

        let top_level_names = self.rename_top_level_definitions(&mut updates);
        self.rename_contract_level_types_references(&top_level_names, &mut updates);
        self.remove_qualified_imports(&mut updates);
        self.update_inheritdocs(&top_level_names, &mut updates);

        self.remove_imports(&mut updates);
        let target_pragmas = self.process_pragmas(&mut updates);
        let target_license = self.process_licenses(&mut updates);

        self.flatten_result(updates, target_pragmas, target_license).get_flattened_target()
    }

    fn flatten_result<'a>(
        &'a self,
        updates: Updates,
        target_pragmas: Vec<&'a str>,
        target_license: Option<&'a str>,
    ) -> FlatteningResult<'_> {
        FlatteningResult::new(self, updates, target_pragmas, target_license)
    }

    /// Finds and goes over all references to file-level definitions and updates them to match
    /// definition name. This is needed for two reasons:
    /// 1. We want to rename all aliased or qualified imports.
    /// 2. We want to find any duplicates and rename them to avoid conflicts.
    ///
    /// If we find more than 1 declaration with the same name, it's name is getting changed.
    /// Two Counter contracts will be renamed to Counter_0 and Counter_1
    ///
    /// Returns mapping from top-level declaration id to its name (possibly updated)
    fn rename_top_level_definitions(&self, updates: &mut Updates) -> HashMap<usize, String> {
        let top_level_definitions = self.collect_top_level_definitions();
        let references = self.collect_references();

        let mut top_level_names = HashMap::new();

        for (name, ids) in top_level_definitions {
            let mut definition_name = name.to_string();
            let needs_rename = ids.len() > 1;

            let mut ids = ids.clone().into_iter().collect::<Vec<_>>();
            if needs_rename {
                // `loc.path` is expected to be different for each id because there can't be 2
                // top-level eclarations with the same name in the same file.
                //
                // Sorting by index loc.path in sorted files to make the renaming process
                // deterministic.
                ids.sort_by_key(|(_, loc)| {
                    self.ordered_sources.iter().position(|p| p == &loc.path).unwrap()
                });
            }
            for (i, (id, loc)) in ids.iter().enumerate() {
                if needs_rename {
                    definition_name = format!("{}_{}", name, i);
                }
                updates.entry(loc.path.clone()).or_default().insert((
                    loc.start,
                    loc.end,
                    definition_name.clone(),
                ));
                if let Some(references) = references.get(&(*id as isize)) {
                    for loc in references {
                        updates.entry(loc.path.clone()).or_default().insert((
                            loc.start,
                            loc.end,
                            definition_name.clone(),
                        ));
                    }
                }

                top_level_names.insert(*id, definition_name.clone());
            }
        }
        top_level_names
    }

    /// This is not very clean, but in most cases effective enough method to remove qualified
    /// imports from sources.
    ///
    /// Every qualified import part is an `Identifier` with `referencedDeclaration` field matching
    /// ID of one of the import directives.
    ///
    /// This approach works by firstly collecting all IDs of import directives, and then looks for
    /// any references of them. Once the reference is found, it's full length is getting removed
    /// from source + 1 charater ('.')
    ///
    /// This should work correctly for vast majority of cases, however there are situations for
    /// which such approach won't work, most of which are related to code being formatted in an
    /// uncommon way.
    fn remove_qualified_imports(&self, updates: &mut Updates) {
        let imports_ids = self
            .asts
            .iter()
            .flat_map(|(_, ast)| {
                ast.nodes.iter().filter_map(|node| match node {
                    SourceUnitPart::ImportDirective(directive) => Some(directive.id),
                    _ => None,
                })
            })
            .collect::<HashSet<_>>();

        let references = self.collect_references();

        for (id, locs) in references {
            if !imports_ids.contains(&(id as usize)) {
                continue;
            }

            for loc in locs {
                updates.entry(loc.path).or_default().insert((
                    loc.start,
                    loc.end + 1,
                    "".to_string(),
                ));
            }
        }
    }

    /// Here we are going through all references to items defined in scope of contracts and updating
    /// them to be using correct parent contract name.
    ///
    /// This will only operate on references from `IdentifierPath` nodes.
    fn rename_contract_level_types_references(
        &self,
        top_level_names: &HashMap<usize, String>,
        updates: &mut Updates,
    ) {
        let contract_level_definitions = self.collect_contract_level_definitions();

        for (path, ast) in &self.asts {
            for node in &ast.nodes {
                let mut collector =
                    ReferencesCollector { path: self.target.clone(), references: HashMap::new() };

                node.walk(&mut collector);

                let references = collector.references;

                for (id, locs) in references {
                    if let Some((name, contract_id)) =
                        contract_level_definitions.get(&(id as usize))
                    {
                        for loc in &locs {
                            // If child item is referenced directly by it's name it's either defined
                            // in the same contract or in one of it's base contracts, so we don't
                            // have to change anything.
                            // Comparing lengths is enough because such items cannot be aliased.
                            if loc.length() == name.len() {
                                continue;
                            }
                            // If it was referenced somehow else, we rename it to `Parent.Child`
                            // format.
                            let parent_name = top_level_names.get(contract_id).unwrap();
                            updates.entry(path.clone()).or_default().insert((
                                loc.start,
                                loc.end,
                                format!("{}.{}", parent_name, name),
                            ));
                        }
                    }
                }
            }
        }
    }

    /// Finds all @inheritdoc tags in natspec comments and tries replacing them.
    ///
    /// We will either replace contract name or remove @inheritdoc tag completely to avoid
    /// generating invalid source code.
    fn update_inheritdocs(&self, top_level_names: &HashMap<usize, String>, updates: &mut Updates) {
        trace!("updating @inheritdoc tags");
        for (path, ast) in &self.asts {
            // Collect all exported symbols for this source unit
            // @inheritdoc value is either one of those or qualified import path which we don't
            // support
            let exported_symbols = ast
                .exported_symbols
                .iter()
                .filter_map(
                    |(name, ids)| {
                        if !ids.is_empty() {
                            Some((name.as_str(), ids[0]))
                        } else {
                            None
                        }
                    },
                )
                .collect::<HashMap<_, _>>();

            // Collect all docs in all contracts
            let docs = ast
                .nodes
                .iter()
                .filter_map(|node| match node {
                    SourceUnitPart::ContractDefinition(d) => Some(d),
                    _ => None,
                })
                .flat_map(|contract| {
                    contract.nodes.iter().filter_map(|node| match node {
                        ContractDefinitionPart::EventDefinition(event) => {
                            event.documentation.as_ref()
                        }
                        ContractDefinitionPart::ErrorDefinition(error) => {
                            error.documentation.as_ref()
                        }
                        ContractDefinitionPart::FunctionDefinition(func) => {
                            func.documentation.as_ref()
                        }
                        ContractDefinitionPart::VariableDeclaration(var) => {
                            var.documentation.as_ref()
                        }
                        _ => None,
                    })
                });

            docs.for_each(|doc| {
                let src_start = doc.src.start.unwrap();
                let src_end = src_start + doc.src.length.unwrap();

                // Documentation node has `text` field, however, it does not contain
                // slashes and we can't use if to find positions.
                let content: &str = &self.sources.get(path).unwrap().content[src_start..src_end];
                let tag_len = "@inheritdoc".len();

                if let Some(tag_start) = content.find("@inheritdoc") {
                    trace!("processing doc with content {:?}", content);
                    if let Some(name_start) = content[tag_start + tag_len..]
                        .find(|c| c != ' ')
                        .map(|p| p + tag_start + tag_len)
                    {
                        let name_end = content[name_start..]
                            .find([' ', '\n', '*', '/'])
                            .map(|p| p + name_start)
                            .unwrap_or(content.len());

                        let name = &content[name_start..name_end];
                        trace!("found name {name}");

                        let mut new_name = None;

                        if let Some(ast_id) = exported_symbols.get(name) {
                            if let Some(name) = top_level_names.get(ast_id) {
                                new_name = Some(name);
                            } else {
                                trace!("ast id {ast_id} cannot be matched to top-level identifier");
                                trace!("known top-level identifiers: {:?}", top_level_names);
                            }
                        }

                        match new_name {
                            Some(new_name) => {
                                trace!("updating tag value with {new_name}");
                                updates.entry(path.to_path_buf()).or_default().insert((
                                    src_start + name_start,
                                    src_start + name_end,
                                    new_name.to_string(),
                                ));
                            }
                            None => {
                                trace!("name is unknown, removing @inheritdoc tag");
                                updates.entry(path.to_path_buf()).or_default().insert((
                                    src_start + tag_start,
                                    src_start + name_end,
                                    "".to_string(),
                                ));
                            }
                        }
                    }
                }
            });
        }
    }

    /// Processes all ASTs and collects all top-level definitions in the form of
    /// a mapping from name to (definition id, source location)
    fn collect_top_level_definitions(&self) -> HashMap<&String, HashSet<(usize, ItemLocation)>> {
        self.asts
            .iter()
            .flat_map(|(path, ast)| {
                ast.nodes
                    .iter()
                    .filter_map(|node| match node {
                        SourceUnitPart::ContractDefinition(contract) => Some((
                            &contract.name,
                            contract.id,
                            &contract.src,
                            &contract.name_location,
                        )),
                        SourceUnitPart::EnumDefinition(enum_) => {
                            Some((&enum_.name, enum_.id, &enum_.src, &enum_.name_location))
                        }
                        SourceUnitPart::StructDefinition(struct_) => {
                            Some((&struct_.name, struct_.id, &struct_.src, &struct_.name_location))
                        }
                        SourceUnitPart::FunctionDefinition(func) => {
                            Some((&func.name, func.id, &func.src, &func.name_location))
                        }
                        SourceUnitPart::VariableDeclaration(var) => {
                            Some((&var.name, var.id, &var.src, &var.name_location))
                        }
                        SourceUnitPart::UserDefinedValueTypeDefinition(type_) => {
                            Some((&type_.name, type_.id, &type_.src, &type_.name_location))
                        }
                        _ => None,
                    })
                    .map(|(name, id, src, maybe_name_src)| {
                        let loc = match maybe_name_src {
                            Some(src) => {
                                ItemLocation::try_from_source_loc(src, path.clone()).unwrap()
                            }
                            None => {
                                // Find location of name in source
                                let content: &str = &self.sources.get(path).unwrap().content;
                                let start = src.start.unwrap();
                                let end = start + src.length.unwrap();

                                let name_start = content[start..end].find(name).unwrap();
                                let name_end = name_start + name.len();

                                ItemLocation {
                                    path: path.clone(),
                                    start: start + name_start,
                                    end: start + name_end,
                                }
                            }
                        };

                        (name, (id, loc))
                    })
            })
            .fold(HashMap::new(), |mut acc, (name, (id, item_location))| {
                acc.entry(name).or_default().insert((id, item_location));
                acc
            })
    }

    /// Collect all contract-level definitions in the form of a mapping from definition id to
    /// (definition name, contract id)
    fn collect_contract_level_definitions(&self) -> HashMap<usize, (&String, usize)> {
        self.asts
            .iter()
            .flat_map(|(_, ast)| {
                ast.nodes.iter().filter_map(|node| match node {
                    SourceUnitPart::ContractDefinition(contract) => {
                        Some((contract.id, &contract.nodes))
                    }
                    _ => None,
                })
            })
            .flat_map(|(contract_id, nodes)| {
                nodes.iter().filter_map(move |node| match node {
                    ContractDefinitionPart::EnumDefinition(enum_) => {
                        Some((enum_.id, (&enum_.name, contract_id)))
                    }
                    ContractDefinitionPart::ErrorDefinition(error) => {
                        Some((error.id, (&error.name, contract_id)))
                    }
                    ContractDefinitionPart::EventDefinition(event) => {
                        Some((event.id, (&event.name, contract_id)))
                    }
                    ContractDefinitionPart::StructDefinition(struct_) => {
                        Some((struct_.id, (&struct_.name, contract_id)))
                    }
                    ContractDefinitionPart::FunctionDefinition(function) => {
                        Some((function.id, (&function.name, contract_id)))
                    }
                    ContractDefinitionPart::VariableDeclaration(variable) => {
                        Some((variable.id, (&variable.name, contract_id)))
                    }
                    ContractDefinitionPart::UserDefinedValueTypeDefinition(value_type) => {
                        Some((value_type.id, (&value_type.name, contract_id)))
                    }
                    _ => None,
                })
            })
            .collect()
    }

    /// Collects all references to any declaration in the form of a mapping from declaration id to
    /// set of source locations it appears in
    fn collect_references(&self) -> HashMap<isize, HashSet<ItemLocation>> {
        self.asts
            .iter()
            .flat_map(|(path, ast)| {
                let mut collector =
                    ReferencesCollector { path: path.clone(), references: HashMap::new() };
                ast.walk(&mut collector);
                collector.references
            })
            .fold(HashMap::new(), |mut acc, (id, locs)| {
                acc.entry(id).or_default().extend(locs);
                acc
            })
    }

    /// Removes all imports from all sources.
    fn remove_imports(&self, updates: &mut Updates) {
        for loc in self.collect_imports() {
            updates.entry(loc.path.clone()).or_default().insert((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }
    }

    // Collects all imports locations.
    fn collect_imports(&self) -> HashSet<ItemLocation> {
        self.asts
            .iter()
            .flat_map(|(path, ast)| {
                ast.nodes.iter().filter_map(|node| match node {
                    SourceUnitPart::ImportDirective(import) => {
                        ItemLocation::try_from_source_loc(&import.src, path.clone())
                    }
                    _ => None,
                })
            })
            .collect()
    }

    /// Removes all pragma directives from all sources. Returns Vec of pragmas that were found in
    /// target file.
    fn process_pragmas(&self, updates: &mut Updates) -> Vec<&str> {
        // Pragmas that will be used in the resulted file
        let mut target_pragmas = Vec::new();

        let pragmas = self.collect_pragmas();

        let mut seen_experimental = false;

        for loc in &pragmas {
            let pragma_content = self.read_location(loc);
            if pragma_content.contains("experimental") {
                if !seen_experimental {
                    seen_experimental = true;
                    target_pragmas.push(loc);
                }
            } else if loc.path == self.target {
                target_pragmas.push(loc);
            }

            updates.entry(loc.path.clone()).or_default().insert((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }

        target_pragmas.sort_by_key(|loc| loc.start);
        target_pragmas.iter().map(|loc| self.read_location(loc)).collect::<Vec<_>>()
    }

    // Collects all pragma directives locations.
    fn collect_pragmas(&self) -> HashSet<ItemLocation> {
        self.asts
            .iter()
            .flat_map(|(path, ast)| {
                ast.nodes.iter().filter_map(|node| match node {
                    SourceUnitPart::PragmaDirective(import) => {
                        ItemLocation::try_from_source_loc(&import.src, path.clone())
                    }
                    _ => None,
                })
            })
            .collect()
    }

    /// Removes all license identifiers from all sources. Returns licesnse identifier from target
    /// file, if any.
    fn process_licenses(&self, updates: &mut Updates) -> Option<&str> {
        let mut target_license = None;

        for loc in &self.collect_licenses() {
            if loc.path == self.target {
                target_license = Some(self.read_location(loc));
            }
            updates.entry(loc.path.clone()).or_default().insert((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }

        target_license
    }

    // Collects all SPDX-License-Identifier locations.
    fn collect_licenses(&self) -> HashSet<ItemLocation> {
        self.sources
            .iter()
            .flat_map(|(path, source)| {
                let mut licenses = HashSet::new();
                if let Some(license_start) = source.content.find("SPDX-License-Identifier:") {
                    let start =
                        source.content[..license_start].rfind('\n').map(|i| i + 1).unwrap_or(0);
                    let end = start
                        + source.content[start..]
                            .find('\n')
                            .unwrap_or(source.content.len() - start);
                    licenses.insert(ItemLocation { path: path.clone(), start, end });
                }
                licenses
            })
            .collect()
    }

    // Reads value from the given location of a source file.
    fn read_location(&self, loc: &ItemLocation) -> &str {
        let content: &str = &self.sources.get(&loc.path).unwrap().content;
        &content[loc.start..loc.end]
    }
}

/// Performs DFS to collect all dependencies of a target
fn collect_deps(
    path: &PathBuf,
    paths: &ProjectPathsConfig,
    graph: &Graph,
    deps: &mut HashSet<PathBuf>,
) -> Result<()> {
    if deps.insert(path.clone()) {
        let target_dir = path.parent().ok_or_else(|| {
            SolcError::msg(format!("failed to get parent directory for \"{}\"", path.display()))
        })?;

        let node_id = graph
            .files()
            .get(path)
            .ok_or_else(|| SolcError::msg(format!("cannot resolve file at {}", path.display())))?;

        for import in graph.node(*node_id).imports() {
            let path = paths.resolve_import(target_dir, import.data().path())?;
            collect_deps(&path, paths, graph, deps)?;
        }
    }
    Ok(())
}

/// We want to make order in which sources are written to resulted flattened file
/// deterministic.
///
/// We can't just sort files alphabetically as it might break compilation, because Solidity
/// does not allow base class definitions to appear after derived contract
/// definitions.
///
/// Instead, we sort files by the number of their dependencies (imports of any depth) in ascending
/// order. If files have the same number of dependencies, we sort them alphabetically.
/// Target file is always placed last.
pub fn collect_ordered_deps(
    path: &PathBuf,
    paths: &ProjectPathsConfig,
    graph: &Graph,
) -> Result<Vec<PathBuf>> {
    let mut deps = HashSet::new();
    collect_deps(path, paths, graph, &mut deps)?;

    // Remove path prior counting dependencies
    // It will be added later to the end of resulted Vec
    deps.remove(path);

    let mut paths_with_deps_count = Vec::new();
    for path in deps {
        let mut path_deps = HashSet::new();
        collect_deps(&path, paths, graph, &mut path_deps)?;
        paths_with_deps_count.push((path_deps.len(), path));
    }

    paths_with_deps_count.sort();

    let mut ordered_deps =
        paths_with_deps_count.into_iter().map(|(_, path)| path).collect::<Vec<_>>();

    ordered_deps.push(path.clone());

    Ok(ordered_deps)
}
