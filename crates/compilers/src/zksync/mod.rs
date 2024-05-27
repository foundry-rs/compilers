use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use alloy_primitives::hex;
use foundry_compilers_artifacts::{zksolc::CompilerOutput, SolcLanguage};

use crate::{
    buildinfo::{BuildContext, RawBuildInfo, ETHERS_FORMAT_VERSION},
    compilers::solc::SolcCompiler,
    error::Result,
    zksolc::input::ZkSolcVersionedInput,
    ArtifactOutput, CompilerInput, Project, Source,
};

use md5::Digest;

use self::compile::output::ProjectCompileOutput;

pub mod artifact_output;
pub mod cache;
pub mod compile;
pub mod config;

/// Returns the path to the artifacts directory
pub fn project_artifacts_path<T: ArtifactOutput>(project: &Project<SolcCompiler, T>) -> &PathBuf {
    &project.paths.zksync_artifacts
}

/// Returns the path to the cache file
pub fn project_cache_path<T: ArtifactOutput>(project: &Project<SolcCompiler, T>) -> &PathBuf {
    &project.paths.zksync_cache
}

pub fn project_compile(project: &Project<SolcCompiler>) -> Result<ProjectCompileOutput> {
    self::compile::project::ProjectCompiler::new(project)?.compile()
}

pub fn project_compile_files<P, I>(
    project: &Project<SolcCompiler>,
    files: I,
) -> Result<ProjectCompileOutput>
where
    I: IntoIterator<Item = P>,
    P: Into<PathBuf>,
{
    let sources = Source::read_all(files)?;
    self::compile::project::ProjectCompiler::with_sources(project, sources)?.compile()
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
