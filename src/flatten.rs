use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    path::{Path, PathBuf},
};

use crate::{
    artifacts::{
        ast::SourceLocation,
        visitor::{Visitor, Walk},
        Identifier, IdentifierPath, MemberAccess, Source, SourceUnit, SourceUnitPart, Sources,
        UserDefinedTypeName,
    },
    error::SolcError,
    utils, Graph, Project, ProjectCompileOutput, ProjectPathsConfig, Result,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct ItemLocation {
    path: PathBuf,
    start: usize,
    end: usize,
}

impl ItemLocation {
    fn try_from_source_loc(src: &SourceLocation, path: PathBuf) -> Option<Self> {
        if src.start.is_none() || src.length.is_none() {
            return None;
        }
        let start = src.start.unwrap();
        let end = start + src.length.unwrap();

        Some(ItemLocation { path, start, end })
    }
}

/// Visitor exploring AST and collecting all references to any declarations
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

    fn visit_user_defined_type_name(&mut self, type_name: &UserDefinedTypeName) {
        self.process_referenced_declaration(type_name.referenced_declaration, &type_name.src);
    }

    fn visit_member_access(&mut self, member_access: &MemberAccess) {
        if let Some(id) = member_access.referenced_declaration {
            self.process_referenced_declaration(id, &member_access.src);
        }
    }

    fn visit_identifier_path(&mut self, path: &IdentifierPath) {
        self.process_referenced_declaration(path.referenced_declaration, &path.src);
    }
}

/// Context for flattening. Stores all sources and ASTs that are in scope of the flattening target.
pub struct Flattener {
    target: PathBuf,
    sources: Sources,
    asts: Vec<(PathBuf, SourceUnit)>,
    // Sources in the order they should be written to the output file.
    ordered_sources: Vec<PathBuf>,
}

impl Flattener {
    pub fn new(project: &Project, output: &ProjectCompileOutput, target: &Path) -> Result<Self> {
        // Performs DFS to collect all dependencies of a target
        fn collect_deps(
            path: &PathBuf,
            paths: &ProjectPathsConfig,
            graph: &Graph,
            deps: &mut HashSet<PathBuf>,
            ordered_deps: &mut Vec<PathBuf>,
        ) -> Result<()> {
            if deps.insert(path.clone()) {
                let target_dir = path.parent().ok_or_else(|| {
                    SolcError::msg(format!(
                        "failed to get parent directory for \"{}\"",
                        path.display()
                    ))
                })?;

                let node_id = graph.files().get(path).ok_or_else(|| {
                    SolcError::msg(format!("cannot resolve file at {}", path.display()))
                })?;

                let mut imports = graph.node(*node_id).imports().clone();
                imports.sort_by_key(|i| i.loc().start);

                for import in imports {
                    let path = paths.resolve_import(target_dir, import.data().path())?;
                    collect_deps(&path, paths, graph, deps, ordered_deps)?;
                }
                ordered_deps.push(path.clone());
            }
            Ok(())
        }

        let graph = Graph::resolve(&project.paths)?;

        let mut ordered_deps = Vec::new();
        collect_deps(
            &target.into(),
            &project.paths,
            &graph,
            &mut HashSet::new(),
            &mut ordered_deps,
        )?;

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

    pub fn flatten(&self) -> String {
        let declarations = self.collect_top_level_declarations();
        let references = self.collect_references();

        // path -> (start, end, new_value)[]
        let mut updates = HashMap::new();

        // Goes over all known declarations and collects all references to later replace them with
        // full name of declaration.
        //
        // If we find more than 1 declaration with the same name, it's name is getting changed.
        // Two Counter contracts will be renamed to Counter_0 and Counter_1
        for (name, ids) in declarations {
            let mut declaration_name = name.to_string();
            let needs_rename = ids.len() > 1;

            let mut ids = ids.clone().into_iter().collect::<Vec<_>>();
            // `loc.path` is expected to be different for each id because there can't be 2 top-level
            // eclarations with the same name
            //
            // Sorting by loc.path to make the renaming process deterministic
            ids.sort_by(|(_, loc_0), (_, loc_1)| loc_0.path.cmp(&loc_1.path));

            for (i, (id, loc)) in ids.iter().enumerate() {
                if needs_rename {
                    declaration_name = format!("{}_{}", name, i);
                }
                updates.entry(loc.path.clone()).or_insert_with(Vec::new).push((
                    loc.start,
                    loc.end,
                    declaration_name.clone(),
                ));
                if let Some(references) = references.get(&(*id as isize)) {
                    for loc in references {
                        updates.entry(loc.path.clone()).or_insert_with(Vec::new).push((
                            loc.start,
                            loc.end,
                            declaration_name.clone(),
                        ));
                    }
                }
            }
        }

        // Replace all imports with empty strings
        for loc in self.collect_imports() {
            updates.entry(loc.path.clone()).or_insert_with(Vec::new).push((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }

        let pragmas = self.collect_pragmas();

        // Pragmas that will be used in the resulted file
        let mut target_pragmas = Vec::new();

        for loc in &pragmas {
            if loc.path == self.target {
                target_pragmas.push(loc);
            }
            updates.entry(loc.path.clone()).or_insert_with(Vec::new).push((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }

        target_pragmas.sort_by_key(|loc| loc.start);
        let target_pragmas =
            target_pragmas.iter().map(|loc| self.read_location(loc)).collect::<Vec<_>>();

        let mut target_license = None;

        for loc in &self.collect_licenses() {
            if loc.path == self.target {
                target_license = Some(self.read_location(loc));
            }
            updates.entry(loc.path.clone()).or_insert_with(Vec::new).push((
                loc.start,
                loc.end,
                "".to_string(),
            ));
        }

        let mut result = String::new();

        if let Some(target_license) = target_license {
            result.push_str(target_license);
            result.push('\n');
        }
        for pragma in target_pragmas {
            result.push_str(pragma);
            result.push('\n');
        }

        let mut updated_sources = HashMap::new();

        for (path, source) in &self.sources {
            let mut content = source.content.as_bytes().to_vec();
            let mut offset: isize = 0;
            if let Some(updates) = updates.get_mut(path) {
                updates.sort_by(|(start_0, _, _), (start_1, _, _)| start_0.cmp(start_1));
                for (start, end, new_value) in updates {
                    let start = (*start as isize + offset) as usize;
                    let end = (*end as isize + offset) as usize;

                    content.splice(start..end, new_value.bytes());
                    offset += new_value.len() as isize - (end - start) as isize;
                }
            }
            updated_sources.insert(path, content);
        }

        for path in &self.ordered_sources {
            let content = updated_sources.get(path).unwrap();
            result.push_str(&String::from_utf8_lossy(content));
            result.push_str("\n\n");
        }

        format!("{}\n", utils::RE_THREE_OR_MORE_NEWLINES.replace_all(&result, "\n\n").trim())
    }

    // Processes all ASTs and collects all top-level declarations in the form of
    // a mapping from name to (declaration id, source location)
    fn collect_top_level_declarations(&self) -> HashMap<&String, HashSet<(usize, ItemLocation)>> {
        self.asts
            .iter()
            .flat_map(|(path, ast)| {
                ast.nodes
                    .iter()
                    .filter_map(|node| match node {
                        SourceUnitPart::ContractDefinition(contract) => {
                            Some((&contract.name, contract.id, &contract.src))
                        }
                        SourceUnitPart::EnumDefinition(enum_) => {
                            Some((&enum_.name, enum_.id, &enum_.src))
                        }
                        SourceUnitPart::StructDefinition(struct_) => {
                            Some((&struct_.name, struct_.id, &struct_.src))
                        }
                        SourceUnitPart::FunctionDefinition(function) => {
                            Some((&function.name, function.id, &function.src))
                        }
                        SourceUnitPart::VariableDeclaration(variable) => {
                            Some((&variable.name, variable.id, &variable.src))
                        }
                        SourceUnitPart::UserDefinedValueTypeDefinition(value_type) => {
                            Some((&value_type.name, value_type.id, &value_type.src))
                        }
                        _ => None,
                    })
                    .map(|(name, id, src)| {
                        // Find location of name in source
                        let content: &str = &self.sources.get(path).unwrap().content;
                        let start = src.start.unwrap();
                        let end = start + src.length.unwrap();

                        let name_start = content[start..end].find(name).unwrap();
                        let name_end = name_start + name.len();

                        let loc = ItemLocation {
                            path: path.clone(),
                            start: start + name_start,
                            end: start + name_end,
                        };

                        (name, (id, loc))
                    })
            })
            .fold(HashMap::new(), |mut acc, (name, (id, item_location))| {
                acc.entry(name).or_default().insert((id, item_location));
                acc
            })
    }

    // Collects all references to any declaration in the form of a mapping from declaration id to
    // set of source locations it appears in
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

    // Collects all imports locations
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

    // Collects all pragma directives locations
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

    // Collects all SPDX-License-Identifier locations
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

    // Reads a value of a given location
    fn read_location(&self, loc: &ItemLocation) -> &str {
        let content: &str = &self.sources.get(&loc.path).unwrap().content;
        &content[loc.start..loc.end]
    }
}
