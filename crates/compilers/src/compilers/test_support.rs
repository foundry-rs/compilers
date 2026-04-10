//! Compiler support for Foundry's test runner.
//!
//! Types that compilers can use to provide optional source-level analysis to
//! enhance the test runner's fuzzing and configuration capabilities.
//! Compilers that don't provide these still work — the runner just won't have
//! source-seeded fuzz dictionaries or inline config overrides.

use super::{Compiler, CompilerOutput};
use alloy_primitives::{Address, Bytes, I256, U256};

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
    /// The contract identifier, in the form `path:ContractName`.
    pub contract: String,
    /// The function name, if this is a function-level override.
    /// `None` for contract-level overrides.
    pub function: Option<String>,
    /// The location in source (for error reporting), e.g. `"10:5"`.
    pub line: String,
    /// Raw configuration lines in the same key format as `foundry.toml`,
    /// e.g. `"default.fuzz.runs = 1024"`.
    pub config_values: Vec<String>,
}

/// Optional trait that compilers can implement to provide source-level metadata
/// to Foundry's test runner.
///
/// The test runner uses this metadata to:
/// - Seed the fuzz dictionary with literals from source code
/// - Apply per-test configuration overrides (e.g. fuzz runs, invariant depth)
///
/// Compilers that don't implement this still work — the runner just uses
/// runtime-discovered values for fuzzing and global config for all tests.
///
/// # Example
///
/// ```ignore
/// impl TestRunnerSupport for MyCompiler {
///     fn fuzz_literals(
///         &self,
///         output: &CompilerOutput<Self::CompilationError, Self::CompilerContract>,
///     ) -> Vec<FuzzLiteral> {
///         // Extract address/uint/string constants from your AST
///         vec![]
///     }
/// }
/// ```
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TestRunnerSupport: Compiler {
    /// Extract literal values from compiled sources to seed the fuzzer dictionary.
    ///
    /// The runner calls this after compilation. The returned literals are merged
    /// into the fuzz dictionary alongside runtime-discovered values.
    ///
    /// Default: empty (fuzzer relies only on runtime-discovered values).
    fn fuzz_literals<E, C>(&self, _output: &CompilerOutput<E, C>) -> Vec<FuzzLiteral> {
        vec![]
    }

    /// Extract per-test configuration overrides from sources.
    ///
    /// Solidity uses `/// forge-config:` NatSpec comments for this. Other languages
    /// can use their own comment/annotation syntax.
    ///
    /// Default: no per-test overrides.
    fn inline_config<E, C>(&self, _output: &CompilerOutput<E, C>) -> InlineConfigEntries {
        InlineConfigEntries::default()
    }
}
