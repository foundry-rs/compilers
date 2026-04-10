//! Compiler support for Foundry's test runner.
//!
//! Types that compilers can use to provide optional source-level analysis to
//! enhance the test runner's fuzzing and configuration capabilities.
//! Compilers that don't provide these still work — the runner just won't have
//! source-seeded fuzz dictionaries or inline config overrides.

use alloy_primitives::{Address, Bytes, I256, U256};
use std::path::PathBuf;

/// A literal value extracted from source code for fuzzer dictionary seeding.
///
/// Compilers can extract constants, addresses, and other literal values from
/// their source files and return them as `FuzzLiteral`s. The test runner merges
/// these into the fuzz dictionary alongside runtime-discovered values.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FuzzLiteral {
    /// An Ethereum address literal.
    Address(Address),
    /// An unsigned integer literal.
    Uint(U256),
    /// A signed integer literal.
    Int(I256),
    /// A fixed-size byte array literal (`bytesN` where N is 1..=32).
    /// The `size` field indicates the `bytesN` width.
    FixedBytes {
        /// The raw bytes value, must have length equal to `size`.
        value: Bytes,
        /// The `bytesN` width (1..=32).
        size: u8,
    },
    /// A dynamic byte array literal (`bytes`).
    DynBytes(Bytes),
    /// A string literal.
    String(String),
}

/// Per-test inline configuration overrides.
///
/// Solidity uses `/// forge-config:` NatSpec comments for this. Other languages
/// can use their own comment or annotation syntax.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct InlineConfigEntries {
    /// The list of parsed inline config entries.
    pub entries: Vec<InlineConfigEntry>,
}

/// A single inline configuration override for a specific contract or function.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct InlineConfigEntry {
    /// The source file path (relative to project root).
    pub source: PathBuf,
    /// The contract identifier, in the form `path:ContractName`.
    pub contract: String,
    /// The function name, if this is a function-level override.
    /// `None` for contract-level overrides.
    pub function: Option<String>,
    /// The location in source (for error reporting), e.g. `"10:5"`.
    pub line: String,
    /// Raw configuration text. Each string is a single config line in the same
    /// format as `foundry.toml`, e.g. `"default.fuzz.runs = 1024"`.
    pub config_values: Vec<String>,
}
