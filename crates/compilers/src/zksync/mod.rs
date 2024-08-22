use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use alloy_primitives::hex;
use foundry_compilers_artifacts::{zksolc::CompilerOutput, SolcLanguage};
use foundry_compilers_core::error::SolcError;

use crate::{
    buildinfo::{BuildContext, RawBuildInfo, ETHERS_FORMAT_VERSION},
    error::Result,
    resolver::parse::SolData,
    zksolc::{
        input::{StandardJsonCompilerInput, ZkSolcVersionedInput},
        settings::ZkSolcSettings,
        ZkSolcCompiler,
    },
    CompilerInput, Graph, Project, Source,
};

use md5::Digest;

use self::{artifact_output::zk::ZkArtifactOutput, compile::output::ProjectCompileOutput};

pub mod artifact_output;
pub mod compile;

pub fn project_compile(
    project: &Project<ZkSolcCompiler, ZkArtifactOutput>,
) -> Result<ProjectCompileOutput> {
    self::compile::project::ProjectCompiler::new(project)?.compile()
}

pub fn project_compile_files<P, I>(
    project: &Project<ZkSolcCompiler, ZkArtifactOutput>,
    files: I,
) -> Result<ProjectCompileOutput>
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    let sources = Source::read_all(files)?;
    self::compile::project::ProjectCompiler::with_sources(project, sources)?.compile()
}

pub fn project_standard_json_input(
    project: &Project<ZkSolcCompiler, ZkArtifactOutput>,
    target: &Path,
) -> Result<StandardJsonCompilerInput> {
    tracing::debug!(?target, "standard_json_input for zksync");
    let graph = Graph::<SolData>::resolve(&project.paths)?;
    let target_index = graph
        .files()
        .get(target)
        .ok_or_else(|| SolcError::msg(format!("cannot resolve file at {:?}", target.display())))?;

    let mut sources = Vec::new();
    let mut unique_paths = HashSet::new();
    let (path, source) = graph.node(*target_index).unpack();
    unique_paths.insert(path.clone());
    sources.push((path, source));
    sources.extend(
        graph
            .all_imported_nodes(*target_index)
            .map(|index| graph.node(index).unpack())
            .filter(|(p, _)| unique_paths.insert(p.to_path_buf())),
    );

    let root = project.root();
    let sources = sources
        .into_iter()
        .map(|(path, source)| (rebase_path(root, path), source.clone()))
        .collect();

    let mut zk_solc_settings: ZkSolcSettings = project.settings.clone();
    // strip the path to the project root from all remappings
    zk_solc_settings.settings.remappings = project
        .paths
        .remappings
        .clone()
        .into_iter()
        .map(|r| r.into_relative(project.root()).to_relative_remapping())
        .collect::<Vec<_>>();

    zk_solc_settings.settings.libraries.libs = zk_solc_settings
        .settings
        .libraries
        .libs
        .into_iter()
        .map(|(f, libs)| (f.strip_prefix(project.root()).unwrap_or(&f).to_path_buf(), libs))
        .collect();

    let input = StandardJsonCompilerInput::new(sources, zk_solc_settings.settings);

    Ok(input)
}

// Copied from compilers/lib private method
fn rebase_path(base: &Path, path: &Path) -> PathBuf {
    use path_slash::PathExt;

    let mut base_components = base.components();
    let mut path_components = path.components();

    let mut new_path = PathBuf::new();

    while let Some(path_component) = path_components.next() {
        let base_component = base_components.next();

        if Some(path_component) != base_component {
            if base_component.is_some() {
                new_path.extend(
                    std::iter::repeat(std::path::Component::ParentDir)
                        .take(base_components.count() + 1),
                );
            }

            new_path.push(path_component);
            new_path.extend(path_components);

            break;
        }
    }

    new_path.to_slash_lossy().into_owned().into()
}

pub fn build_context_new(
    input: &ZkSolcVersionedInput,
    output: &CompilerOutput,
) -> Result<BuildContext<SolcLanguage>> {
    let mut source_id_to_path = BTreeMap::new();

    let input_sources = input.sources().map(|(path, _)| path).collect::<HashSet<_>>();
    for (path, source) in output.sources.iter() {
        if input_sources.contains(path.as_path()) {
            source_id_to_path.insert(source.id, path.to_path_buf());
        }
    }

    Ok(BuildContext { source_id_to_path, language: input.language() })
}

pub fn raw_build_info_new(
    input: &ZkSolcVersionedInput,
    output: &CompilerOutput,
    full_build_info: bool,
) -> Result<RawBuildInfo<SolcLanguage>> {
    // TODO: evaluate if this should be zksolc version instead
    let version = input.solc_version.clone();
    let build_context = build_context_new(input, output)?;

    let mut hasher = md5::Md5::new();

    hasher.update(ETHERS_FORMAT_VERSION);

    let solc_short = format!("{}.{}.{}", version.major, version.minor, version.patch);
    hasher.update(&solc_short);
    hasher.update(version.to_string());

    let input = serde_json::to_value(input)?;
    hasher.update(&serde_json::to_string(&input)?);

    // create the hash for `{_format,solcVersion,solcLongVersion,input}`
    // N.B. this is not exactly the same as hashing the json representation of these values but
    // the must efficient one
    let result = hasher.finalize();
    let id = hex::encode(result);

    let mut build_info = BTreeMap::new();

    if full_build_info {
        build_info.insert("_format".to_string(), serde_json::to_value(ETHERS_FORMAT_VERSION)?);
        build_info.insert("solcVersion".to_string(), serde_json::to_value(&solc_short)?);
        build_info.insert("solcLongVersion".to_string(), serde_json::to_value(&version)?);
        build_info.insert("input".to_string(), input);
        build_info.insert("output".to_string(), serde_json::to_value(output)?);
    }

    Ok(RawBuildInfo { id, build_info, build_context })
}
