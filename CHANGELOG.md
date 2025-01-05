# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.9](https://github.com/foundry-rs/compilers/releases/tag/v0.12.9) - 2025-01-05

### Dependencies

- [deps] Bump solar 0.1.1 ([#237](https://github.com/foundry-rs/compilers/issues/237))

## [0.12.8](https://github.com/foundry-rs/compilers/releases/tag/v0.12.8) - 2024-12-13

### Bug Fixes

- Correctly merge restrictions ([#234](https://github.com/foundry-rs/compilers/issues/234))

### Other

- Move deny to ci ([#233](https://github.com/foundry-rs/compilers/issues/233))

## [0.12.6](https://github.com/foundry-rs/compilers/releases/tag/v0.12.6) - 2024-12-04

### Performance

- Don't request unnecessary output ([#231](https://github.com/foundry-rs/compilers/issues/231))

## [0.12.5](https://github.com/foundry-rs/compilers/releases/tag/v0.12.5) - 2024-12-04

### Refactor

- Make Contract generic for Compiler and add metadata to CompilerOutput ([#224](https://github.com/foundry-rs/compilers/issues/224))

## [0.12.4](https://github.com/foundry-rs/compilers/releases/tag/v0.12.4) - 2024-12-02

### Bug Fixes

- Add fallback parser for contract names ([#229](https://github.com/foundry-rs/compilers/issues/229))
- Fix minor grammatical issue in project documentation ([#226](https://github.com/foundry-rs/compilers/issues/226))

### Dependencies

- Bump MSRV to 1.83 ([#230](https://github.com/foundry-rs/compilers/issues/230))

## [0.12.3](https://github.com/foundry-rs/compilers/releases/tag/v0.12.3) - 2024-11-20

### Bug Fixes

- Imports regex fallback ([#225](https://github.com/foundry-rs/compilers/issues/225))

## [0.12.2](https://github.com/foundry-rs/compilers/releases/tag/v0.12.2) - 2024-11-20

### Bug Fixes

- Re-add version regex parsing ([#223](https://github.com/foundry-rs/compilers/issues/223))

### Miscellaneous Tasks

- Don't color punctuation in output diagnostics ([#222](https://github.com/foundry-rs/compilers/issues/222))

## [0.12.1](https://github.com/foundry-rs/compilers/releases/tag/v0.12.1) - 2024-11-18

### Bug Fixes

- `collect_contract_names` ([#221](https://github.com/foundry-rs/compilers/issues/221))

## [0.12.0](https://github.com/foundry-rs/compilers/releases/tag/v0.12.0) - 2024-11-18

### Bug Fixes

- [tests] Always try installing pinned solc ([#217](https://github.com/foundry-rs/compilers/issues/217))
- Outdated merge build error
- Correctly handle b as pre-release in Vyper version ([#213](https://github.com/foundry-rs/compilers/issues/213))

### Features

- Allow multiple compiler configs ([#170](https://github.com/foundry-rs/compilers/issues/170))
- Replace solang with solar ([#215](https://github.com/foundry-rs/compilers/issues/215))

### Miscellaneous Tasks

- Remove outdated `ref` patterns ([#218](https://github.com/foundry-rs/compilers/issues/218))
- Use Version::new over .parse ([#220](https://github.com/foundry-rs/compilers/issues/220))

## [0.11.5](https://github.com/foundry-rs/compilers/releases/tag/v0.11.5) - 2024-10-14

### Bug Fixes

- Accept partial first sourcemap element ([#209](https://github.com/foundry-rs/compilers/issues/209))

### Miscellaneous Tasks

- Allow adding vyper sources with `add_raw_source` w/ `.vy` / `.vyi` extension ([#211](https://github.com/foundry-rs/compilers/issues/211))

## [0.11.4](https://github.com/foundry-rs/compilers/releases/tag/v0.11.4) - 2024-10-02

### Features

- Better extra_args handling ([#208](https://github.com/foundry-rs/compilers/issues/208))

## [0.11.3](https://github.com/foundry-rs/compilers/releases/tag/v0.11.3) - 2024-09-30

### Miscellaneous Tasks

- Proper generate legacy asm extra output file ([#207](https://github.com/foundry-rs/compilers/issues/207))

## [0.11.2](https://github.com/foundry-rs/compilers/releases/tag/v0.11.2) - 2024-09-30

### Bug Fixes

- Include `evm.legacyAssembly` output ([#206](https://github.com/foundry-rs/compilers/issues/206))

### Documentation

- Fix typos ([#202](https://github.com/foundry-rs/compilers/issues/202))

### Miscellaneous Tasks

- Clippy ([#204](https://github.com/foundry-rs/compilers/issues/204))

## [0.11.1](https://github.com/foundry-rs/compilers/releases/tag/v0.11.1) - 2024-09-17

### Bug Fixes

- Actualize output selection options ([#196](https://github.com/foundry-rs/compilers/issues/196))

### Features

- Better error messages for incompatible versions ([#200](https://github.com/foundry-rs/compilers/issues/200))

### Miscellaneous Tasks

- Clippy happy ([#195](https://github.com/foundry-rs/compilers/issues/195))

## [0.10.3](https://github.com/foundry-rs/compilers/releases/tag/v0.10.3) - 2024-08-26

### Bug Fixes

- [flatten] Update license handling logic ([#184](https://github.com/foundry-rs/compilers/issues/184))

### Features

- Always provide `Default` for `MultiCompiler` ([#188](https://github.com/foundry-rs/compilers/issues/188))
- [vyper] Add experimental codegen to settings ([#186](https://github.com/foundry-rs/compilers/issues/186))
- More user-friendly error when no compiler is available ([#185](https://github.com/foundry-rs/compilers/issues/185))

## [0.10.2](https://github.com/foundry-rs/compilers/releases/tag/v0.10.2) - 2024-08-01

### Bug Fixes

- Unify logic for ignored warnings ([#179](https://github.com/foundry-rs/compilers/issues/179))
- Remove outdated build infos ([#177](https://github.com/foundry-rs/compilers/issues/177))

## [0.10.1](https://github.com/foundry-rs/compilers/releases/tag/v0.10.1) - 2024-07-26

### Bug Fixes

- Better compatibility with older AST ([#175](https://github.com/foundry-rs/compilers/issues/175))

## [0.10.0](https://github.com/foundry-rs/compilers/releases/tag/v0.10.0) - 2024-07-18

### Bug Fixes

- Fix inconsistent trailing slash in remappings ([#49](https://github.com/foundry-rs/compilers/issues/49))

### Features

- Add `eofVersion` config option ([#174](https://github.com/foundry-rs/compilers/issues/174))
- Allow passing extra cli args to solc + some cleanup ([#171](https://github.com/foundry-rs/compilers/issues/171))

## [0.9.0](https://github.com/foundry-rs/compilers/releases/tag/v0.9.0) - 2024-06-29

### Bug Fixes

- Doctests ([#154](https://github.com/foundry-rs/compilers/issues/154))
- [flatten] Small bugs ([#153](https://github.com/foundry-rs/compilers/issues/153))

### Features

- Respect `paths.libraries` for Vyper ([#159](https://github.com/foundry-rs/compilers/issues/159))

### Miscellaneous Tasks

- Improve stripping file prefixes ([#164](https://github.com/foundry-rs/compilers/issues/164))
- Improve some trace-level logs ([#163](https://github.com/foundry-rs/compilers/issues/163))
- Remove most impl AsRef<str,Path> ([#157](https://github.com/foundry-rs/compilers/issues/157))
- Clarify version cache lock ([#160](https://github.com/foundry-rs/compilers/issues/160))
- Sort derives, derive Eq more ([#161](https://github.com/foundry-rs/compilers/issues/161))
- Rename foundry-compilers-project into foundry-compilers ([#152](https://github.com/foundry-rs/compilers/issues/152))

### Other

- Symlink readme

### Refactor

- Unify sources and filtered sources ([#162](https://github.com/foundry-rs/compilers/issues/162))

<!-- generated by git-cliff -->
