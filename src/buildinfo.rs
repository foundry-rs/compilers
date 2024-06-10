//! Represents an entire build

use crate::{
    compilers::{CompilationError, CompilerInput, CompilerOutput, Language},
    error::Result,
    utils,
};
use alloy_primitives::hex;
use md5::Digest;
use semver::Version;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    rc::Rc,
};

pub const ETHERS_FORMAT_VERSION: &str = "ethers-rs-sol-build-info-1";

// A hardhat compatible build info representation
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo<I, O> {
    pub id: String,
    #[serde(rename = "_format")]
    pub format: String,
    pub solc_version: Version,
    pub solc_long_version: Version,
    pub input: I,
    pub output: O,
}

impl<I: DeserializeOwned, O: DeserializeOwned> BuildInfo<I, O> {
    /// Deserializes the `BuildInfo` object from the given file
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        utils::read_json_file(path)
    }
}

/// Additional context we cache for each compiler run.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct BuildContext<L> {
    /// Mapping from internal compiler source id to path of the source file.
    pub source_id_to_path: HashMap<u32, PathBuf>,
    /// Language of the compiler.
    pub language: L,
}

impl<L: Language> BuildContext<L> {
    pub fn new<I, E>(input: &I, output: &CompilerOutput<E>) -> Result<Self>
    where
        I: CompilerInput<Language = L>,
    {
        let mut source_id_to_path = HashMap::new();

        let input_sources = input.sources().map(|(path, _)| path).collect::<HashSet<_>>();
        for (path, source) in output.sources.iter() {
            if input_sources.contains(path.as_path()) {
                source_id_to_path.insert(source.id, path.to_path_buf());
            }
        }

        Ok(Self { source_id_to_path, language: input.language() })
    }

    pub fn join_all(&mut self, root: impl AsRef<Path>) {
        self.source_id_to_path.values_mut().for_each(|path| {
            *path = root.as_ref().join(path.as_path());
        });
    }
}

/// Represents `BuildInfo` object
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RawBuildInfo<L> {
    /// The hash that identifies the BuildInfo
    pub id: String,
    #[serde(flatten)]
    pub build_context: BuildContext<L>,
    /// serialized `BuildInfo` json
    #[serde(flatten)]
    pub build_info: BTreeMap<String, serde_json::Value>,
}

// === impl RawBuildInfo ===

impl<L: Language> RawBuildInfo<L> {
    /// Serializes a `BuildInfo` object
    pub fn new<I: CompilerInput<Language = L>, E: CompilationError>(
        input: &I,
        output: &CompilerOutput<E>,
        full_build_info: bool,
    ) -> Result<RawBuildInfo<L>> {
        let version = input.version().clone();
        let build_context = BuildContext::new(input, output)?;

        let mut hasher = md5::Md5::new();

        hasher.update(ETHERS_FORMAT_VERSION);

        let solc_short = format!("{}.{}.{}", version.major, version.minor, version.patch);
        hasher.update(&solc_short);
        hasher.update(&version.to_string());

        let input = serde_json::to_value(input)?;
        hasher.update(&serde_json::to_string(&input)?);

        // create the hash for `{_format,solcVersion,solcLongVersion,input}`
        // N.B. this is not exactly the same as hashing the json representation of these values but
        // the must efficient one
        let result = hasher.finalize();
        let id = hex::encode(result);

        let mut build_info = BTreeMap::new();

        if full_build_info {
            build_info.insert("_format".to_string(), serde_json::to_value(&ETHERS_FORMAT_VERSION)?);
            build_info.insert("solcVersion".to_string(), serde_json::to_value(&solc_short)?);
            build_info.insert("solcLongVersion".to_string(), serde_json::to_value(&version)?);
            build_info.insert("input".to_string(), input);
            build_info.insert("output".to_string(), serde_json::to_value(&output)?);
        }

        Ok(RawBuildInfo { id, build_info, build_context })
    }

    // We only join [BuildContext] paths here because input and output are kept in the same format
    // as compiler seen/produced them.
    pub fn join_all(&mut self, root: impl AsRef<Path>) {
        self.build_context.join_all(root);
    }
}

#[derive(Clone)]
struct BuildInfoWriter {
    buf: Rc<RefCell<Vec<u8>>>,
}

impl std::io::Write for BuildInfoWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buf.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.buf.borrow_mut().flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifacts::Error,
        compilers::{
            solc::{SolcLanguage, SolcVersionedInput},
            CompilerOutput,
        },
        Source,
    };
    use std::{collections::BTreeMap, path::PathBuf};

    #[test]
    fn build_info_serde() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            BTreeMap::from([(PathBuf::from("input.sol"), Source::new(""))]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );
        let output = CompilerOutput::<Error>::default();
        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let _info: BuildInfo<SolcVersionedInput, CompilerOutput<Error>> =
            serde_json::from_str(&serde_json::to_string(&raw_info).unwrap()).unwrap();
    }
}
