# Foundry Compilers - Claude Code Assistant Guide

This guide provides comprehensive instructions for Claude Code agents working on the Foundry Compilers project. Foundry Compilers is the compilation backend for [Foundry](https://github.com/foundry-rs/foundry), supporting both Solidity and Vyper smart contract compilation.

## Project Overview

Foundry Compilers (formerly `ethers-solc`) is a Rust library that provides:
- Multi-language support (Solidity via `solc` and Vyper)
- Compilation management with caching
- Artifact handling and output management
- Source resolution and dependency management
- Build information and metadata handling

### Key Components

1. **Core Library** (`crates/compilers/`): Main compilation logic and project management
2. **Artifacts** (`crates/artifacts/`): Data structures for compiler inputs/outputs
   - `artifacts/`: Common artifact types
   - `solc/`: Solidity-specific artifacts (AST, bytecode, etc.)
   - `vyper/`: Vyper-specific artifacts
3. **Core Utilities** (`crates/core/`): Shared utilities and error handling

## Development Guidelines

### Environment Setup

- **Rust Version**: MSRV 1.87 (check `Cargo.toml` for current version)
- **Platform Support**: Linux, macOS, Windows
- **Optional Features**:
  - `svm-solc`: Automatic Solc version management
  - `project-util`: Testing utilities

### Code Quality Standards

#### Formatting
```bash
# Use nightly for formatting
cargo +nightly fmt --all

# Check formatting without changes
cargo +nightly fmt --all --check
```

#### Linting
```bash
# Run clippy with all features
cargo clippy --all-features --all-targets
# CI runs with warnings as errors
RUSTFLAGS="-D warnings" cargo clippy --all-features --all-targets
```

#### Testing
```bash
# Run all tests
cargo test --all-features

# Run doc tests
cargo test --workspace --doc --all-features

# Run specific test
cargo test test_name --all-features
```

### Project Structure

```
foundry-compilers/
├── crates/
│   ├── compilers/          # Main library
│   │   ├── src/
│   │   │   ├── compilers/  # Compiler implementations
│   │   │   │   ├── solc/   # Solidity compiler
│   │   │   │   └── vyper/  # Vyper compiler
│   │   │   ├── artifact_output/  # Artifact handling
│   │   │   ├── cache/      # Compilation caching
│   │   │   ├── compile/    # Compilation orchestration
│   │   │   ├── resolver/   # Source resolution
│   │   │   └── lib.rs      # Main API
│   │   └── tests/
│   ├── artifacts/          # Artifact definitions
│   │   ├── artifacts/      # Common artifacts
│   │   ├── solc/          # Solidity-specific
│   │   └── vyper/         # Vyper-specific
│   └── core/              # Core utilities
├── test-data/             # Test fixtures
└── benches/               # Performance benchmarks
```

### Key APIs and Patterns

#### Project Configuration
```rust
use foundry_compilers::{Project, ProjectPathsConfig};

// Configure project paths
let paths = ProjectPathsConfig::hardhat(root)?;

// Build project with settings
let project = Project::builder()
    .paths(paths)
    .settings(settings)
    .build(Default::default())?;

// Compile project
let output = project.compile()?;
```

#### Multi-Compiler Support
The project supports both Solidity and Vyper through the `MultiCompiler`:
- Automatically detects file types (`.sol`, `.vy`, `.yul`)
- Manages compiler-specific settings
- Handles mixed-language projects

#### Artifact Management
- Configurable output formats via `ArtifactOutput` trait
- Built-in implementations: `ConfigurableArtifacts`, `HardhatArtifacts`
- Support for custom artifact handlers

### Testing Guidelines

1. **Unit Tests**: Test individual functions with focused scope
2. **Integration Tests**: Located in `tests/` directory of each crate
3. **Documentation Tests**: Include examples in doc comments
4. **Test Data**: Use fixtures from `test-data/` directory

Example test pattern:
```rust
#[test]
fn test_compilation() {
    let root = utils::canonicalize("../../test-data/sample").unwrap();
    let project = Project::builder()
        .paths(ProjectPathsConfig::hardhat(&root).unwrap())
        .build(Default::default())
        .unwrap();
    
    let output = project.compile().unwrap();
    assert!(!output.has_compiler_errors());
}
```

### Common Tasks

#### Adding Compiler Support
1. Implement the `Compiler` trait
2. Add language detection in `Language` trait
3. Update `MultiCompiler` to include new compiler
4. Add artifact types in `artifacts` crate

#### Modifying AST Handling
- AST definitions in `crates/artifacts/solc/src/ast/`
- Use `#[serde(rename_all = "camelCase")]` for JSON compatibility
- Handle version differences with conditional compilation

#### Cache Management
- Cache stored in `.foundry_cache/`
- Invalidation based on source content and compiler settings
- See `crates/compilers/src/cache.rs` for implementation

### Performance Considerations

1. **Parallel Compilation**: Controlled by `solc_jobs` setting
2. **Caching**: Aggressive caching of compilation results
3. **Sparse Output**: Use `SparseOutputFilter` to limit artifacts
4. **Graph Resolution**: Efficient dependency resolution

Run benchmarks:
```bash
cargo bench --workspace
```

### Error Handling

- Use `Result<T>` with `SolcError` for all fallible operations
- Provide context with error messages
- Handle compiler-specific errors appropriately

### Contributing Workflow

1. **Before Starting**:
   - Check existing issues and PRs
   - Discuss large changes in an issue first

2. **Development**:
   - Follow conventional commits (feat:, fix:, chore:, etc.)
   - Keep commits logically grouped
   - Add tests for new functionality

3. **Submitting PRs**:
   - Ensure CI passes (formatting, clippy, tests)
   - Update documentation as needed
   - Allow edits from maintainers

### Common Commands Summary

```bash
# Development
cargo check --all-features
cargo build --all-features
cargo test --all-features

# Code Quality
cargo +nightly fmt --all
cargo clippy --all-features --all-targets

# Documentation
cargo doc --all-features --no-deps --open

# Benchmarks
cargo bench --workspace
```

### Debugging Tips

1. **Enable Tracing**: Set `RUST_LOG=foundry_compilers=trace`
2. **Inspect Cache**: Check `.foundry_cache/solidity-files-cache.json`
3. **Compilation Errors**: Use `output.diagnostics()` for detailed errors
4. **Graph Issues**: Use `Graph::resolve()` to debug dependency problems

### Important Notes

- **No Grammar/Spelling PRs**: Focus on functional improvements
- **Breaking Changes**: Clearly mark in commit messages
- **Version Support**: Maintain compatibility with supported Solc/Vyper versions
- **Platform Testing**: Ensure changes work on Linux, macOS, and Windows

### Resources

- [API Documentation](https://docs.rs/foundry-compilers)
- [Foundry Book](https://book.getfoundry.sh/)
- [Telegram Dev Chat](https://t.me/foundry_rs)