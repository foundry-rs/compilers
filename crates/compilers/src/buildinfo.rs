//! Represents an entire build

use crate::compilers::{
    CompilationError, CompilerContract, CompilerInput, CompilerOutput, Language,
};
use foundry_compilers_core::{error::Result, utils};
use semver::Version;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
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
    pub fn read(path: &Path) -> Result<Self> {
        utils::read_json_file(path)
    }
}

/// Additional context we cache for each compiler run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildContext<L> {
    /// Mapping from internal compiler source id to path of the source file.
    pub source_id_to_path: BTreeMap<u32, PathBuf>,
    /// Language of the compiler.
    pub language: L,
}

impl<L: Language> BuildContext<L> {
    pub fn new<I, E, C>(input: &I, output: &CompilerOutput<E, C>) -> Result<Self>
    where
        I: CompilerInput<Language = L>,
    {
        let mut source_id_to_path = BTreeMap::new();

        let input_sources = input.sources().map(|(path, _)| path).collect::<HashSet<_>>();
        for (path, source) in output.sources.iter() {
            if input_sources.contains(path.as_path()) {
                source_id_to_path.insert(source.id, path.to_path_buf());
            }
        }

        Ok(Self { source_id_to_path, language: input.language() })
    }

    pub fn join_all(&mut self, root: &Path) {
        self.source_id_to_path.values_mut().for_each(|path| {
            *path = root.join(path.as_path());
        });
    }

    pub fn with_joined_paths(mut self, root: &Path) -> Self {
        self.join_all(root);
        self
    }
}

/// Represents `BuildInfo` object
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn new<I: CompilerInput<Language = L>, E: CompilationError, C: CompilerContract>(
        input: &I,
        output: &CompilerOutput<E, C>,
        full_build_info: bool,
    ) -> Result<Self> {
        let version = input.version().clone();
        let build_context = BuildContext::new(input, output)?;

        let solc_short = format!("{}.{}.{}", version.major, version.minor, version.patch);
        let input = serde_json::to_value(input)?;
        let id = utils::unique_hash_many([
            ETHERS_FORMAT_VERSION,
            &version.to_string(),
            &serde_json::to_string(&input)?,
        ]);

        let mut build_info = BTreeMap::new();

        if full_build_info {
            build_info.insert("_format".to_string(), serde_json::to_value(ETHERS_FORMAT_VERSION)?);
            build_info.insert("solcVersion".to_string(), serde_json::to_value(&solc_short)?);
            build_info.insert("solcLongVersion".to_string(), serde_json::to_value(&version)?);
            build_info.insert("input".to_string(), input);
            build_info.insert("output".to_string(), serde_json::to_value(output)?);
        }

        Ok(Self { id, build_info, build_context })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilers::solc::SolcVersionedInput;
    use foundry_compilers_artifacts::{
        sources::Source, Contract, Error, SolcLanguage, SourceFile, Sources,
    };
    use std::path::PathBuf;

    #[test]
    fn build_info_serde() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::from([(PathBuf::from("input.sol"), Source::new(""))]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );
        let output = CompilerOutput::<Error, Contract>::default();
        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let _info: BuildInfo<SolcVersionedInput, CompilerOutput<Error, Contract>> =
            serde_json::from_str(&serde_json::to_string(&raw_info).unwrap()).unwrap();
    }

    #[test]
    fn sources_serialized_by_source_id() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::from([
                (PathBuf::from("z_last.sol"), Source::new("")),
                (PathBuf::from("a_first.sol"), Source::new("")),
                (PathBuf::from("m_middle.sol"), Source::new("")),
            ]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );

        let mut output = CompilerOutput::<Error, Contract>::default();
        output.sources.insert(PathBuf::from("z_last.sol"), SourceFile { id: 0, ast: None });
        output.sources.insert(PathBuf::from("a_first.sol"), SourceFile { id: 2, ast: None });
        output.sources.insert(PathBuf::from("m_middle.sol"), SourceFile { id: 1, ast: None });

        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        let output_start = json_str.find(r#""output":"#).unwrap();
        let output_section = &json_str[output_start..];

        let z_pos = output_section.find("z_last.sol").unwrap();
        let m_pos = output_section.find("m_middle.sol").unwrap();
        let a_pos = output_section.find("a_first.sol").unwrap();

        assert!(
            z_pos < m_pos,
            "z_last.sol (id=0) should appear before m_middle.sol (id=1) in output.sources"
        );
        assert!(
            m_pos < a_pos,
            "m_middle.sol (id=1) should appear before a_first.sol (id=2) in output.sources"
        );
    }

    #[test]
    fn sources_ordering_empty() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::new(),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );

        let output = CompilerOutput::<Error, Contract>::default();
        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        assert!(json_str.contains(r#""sources":{}"#));
    }

    #[test]
    fn sources_ordering_single_source() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::from([(PathBuf::from("only.sol"), Source::new(""))]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );

        let mut output = CompilerOutput::<Error, Contract>::default();
        output.sources.insert(PathBuf::from("only.sol"), SourceFile { id: 42, ast: None });

        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        assert!(json_str.contains(r#""only.sol":{"id":42"#));
    }

    #[test]
    fn sources_ordering_with_gaps_in_ids() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::from([
                (PathBuf::from("a.sol"), Source::new("")),
                (PathBuf::from("b.sol"), Source::new("")),
                (PathBuf::from("c.sol"), Source::new("")),
            ]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );

        let mut output = CompilerOutput::<Error, Contract>::default();
        output.sources.insert(PathBuf::from("a.sol"), SourceFile { id: 100, ast: None });
        output.sources.insert(PathBuf::from("b.sol"), SourceFile { id: 5, ast: None });
        output.sources.insert(PathBuf::from("c.sol"), SourceFile { id: 50, ast: None });

        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        let output_start = json_str.find(r#""output":"#).unwrap();
        let output_section = &json_str[output_start..];

        let b_pos = output_section.find("b.sol").unwrap();
        let c_pos = output_section.find("c.sol").unwrap();
        let a_pos = output_section.find("a.sol").unwrap();

        assert!(b_pos < c_pos, "b.sol (id=5) should appear before c.sol (id=50)");
        assert!(c_pos < a_pos, "c.sol (id=50) should appear before a.sol (id=100)");
    }

    #[test]
    fn sources_ordering_roundtrip() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();
        let input = SolcVersionedInput::build(
            Sources::from([
                (PathBuf::from("z.sol"), Source::new("")),
                (PathBuf::from("a.sol"), Source::new("")),
            ]),
            Default::default(),
            SolcLanguage::Solidity,
            v,
        );

        let mut output = CompilerOutput::<Error, Contract>::default();
        output.sources.insert(PathBuf::from("z.sol"), SourceFile { id: 0, ast: None });
        output.sources.insert(PathBuf::from("a.sol"), SourceFile { id: 1, ast: None });

        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        let parsed: BuildInfo<SolcVersionedInput, CompilerOutput<Error, Contract>> =
            serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed.output.sources.len(), 2);
        assert_eq!(parsed.output.sources.get(&PathBuf::from("z.sol")).unwrap().id, 0);
        assert_eq!(parsed.output.sources.get(&PathBuf::from("a.sol")).unwrap().id, 1);
    }

    #[test]
    fn sources_ordering_many_sources() {
        let v: Version = "0.8.4+commit.c7e474f2".parse().unwrap();

        let sources: Sources = (0..50)
            .map(|i| (PathBuf::from(format!("contract_{:02}.sol", 49 - i)), Source::new("")))
            .collect();

        let input =
            SolcVersionedInput::build(sources, Default::default(), SolcLanguage::Solidity, v);

        let mut output = CompilerOutput::<Error, Contract>::default();
        for i in 0..50u32 {
            output.sources.insert(
                PathBuf::from(format!("contract_{:02}.sol", 49 - i)),
                SourceFile { id: i, ast: None },
            );
        }

        let raw_info = RawBuildInfo::new(&input, &output, true).unwrap();
        let json_str = serde_json::to_string(&raw_info).unwrap();

        let output_start = json_str.find(r#""output":"#).unwrap();
        let output_section = &json_str[output_start..];

        let mut last_pos = 0;
        for i in 0..50 {
            let filename = format!("contract_{:02}.sol", 49 - i);
            let pos = output_section.find(&filename).unwrap();
            assert!(
                pos > last_pos || i == 0,
                "Sources should be ordered by ID: {filename} (id={i}) at wrong position"
            );
            last_pos = pos;
        }
    }
}
