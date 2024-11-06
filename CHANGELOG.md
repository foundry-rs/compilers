# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.6](https://github.com/foundry-rs/compilers/releases/tag/v0.11.6) - 2024-10-16

### Bug Fixes

- Correctly handle b as pre-release in Vyper version ([#213](https://github.com/foundry-rs/compilers/issues/213))
- Accept partial first sourcemap element ([#209](https://github.com/foundry-rs/compilers/issues/209))

### Miscellaneous Tasks

- Release 0.11.5
- Allow adding vyper sources with `add_raw_source` w/ `.vy` / `.vyi` extension ([#211](https://github.com/foundry-rs/compilers/issues/211))
- [`ci`] Fix deny (add `ZLib` exception) ([#212](https://github.com/foundry-rs/compilers/issues/212))

## [0.11.4](https://github.com/foundry-rs/compilers/releases/tag/v0.11.4) - 2024-10-02

### Bug Fixes

- Include `evm.legacyAssembly` output ([#206](https://github.com/foundry-rs/compilers/issues/206))
- Ast Node Bindings ([#199](https://github.com/foundry-rs/compilers/issues/199))
- Actualize output selection options ([#196](https://github.com/foundry-rs/compilers/issues/196))

### Dependencies

- [deps] Bump alloy ([#193](https://github.com/foundry-rs/compilers/issues/193))

### Documentation

- Fix typos ([#202](https://github.com/foundry-rs/compilers/issues/202))

### Features

- Better extra_args handling ([#208](https://github.com/foundry-rs/compilers/issues/208))
- Better error messages for incompatible versions ([#200](https://github.com/foundry-rs/compilers/issues/200))

### Miscellaneous Tasks

- Release 0.11.4
- Release 0.11.3
- Proper generate legacy asm extra output file ([#207](https://github.com/foundry-rs/compilers/issues/207))
- Release 0.11.2
- Clippy ([#204](https://github.com/foundry-rs/compilers/issues/204))
- Use serde_json::from_str ([#203](https://github.com/foundry-rs/compilers/issues/203))
- Release 0.11.1
- Improve error handling in source map parsing ([#201](https://github.com/foundry-rs/compilers/issues/201))
- Clippy happy ([#195](https://github.com/foundry-rs/compilers/issues/195))
- Fix up the README example ([#194](https://github.com/foundry-rs/compilers/issues/194))
- Release 0.11.0

## [0.10.3](https://github.com/foundry-rs/compilers/releases/tag/v0.10.3) - 2024-08-26

### Bug Fixes

- [flatten] Update license handling logic ([#184](https://github.com/foundry-rs/compilers/issues/184))
- Unify logic for ignored warnings ([#179](https://github.com/foundry-rs/compilers/issues/179))
- Remove outdated build infos ([#177](https://github.com/foundry-rs/compilers/issues/177))
- Make remappings resolution more deterministic ([#176](https://github.com/foundry-rs/compilers/issues/176))

### Documentation

- Docs fix spelling issues ([#190](https://github.com/foundry-rs/compilers/issues/190))

### Features

- Always provide `Default` for `MultiCompiler` ([#188](https://github.com/foundry-rs/compilers/issues/188))
- [vyper] Add experimental codegen to settings ([#186](https://github.com/foundry-rs/compilers/issues/186))
- More user-friendly error when no compiler is available ([#185](https://github.com/foundry-rs/compilers/issues/185))
- Sanitize EVM version for vyper ([#181](https://github.com/foundry-rs/compilers/issues/181))

### Miscellaneous Tasks

- Release 0.10.3
- Release 0.10.2

### Other

- Incorrect Default EVM Version for Solidity Compiler 0.4.21-0.5.4 ([#189](https://github.com/foundry-rs/compilers/issues/189))
- Update README to link docs and update install instructions ([#180](https://github.com/foundry-rs/compilers/issues/180))

## [0.10.1](https://github.com/foundry-rs/compilers/releases/tag/v0.10.1) - 2024-07-26

### Bug Fixes

- Better compatibility with older AST ([#175](https://github.com/foundry-rs/compilers/issues/175))

### Features

- Add Prague evm version ([#166](https://github.com/foundry-rs/compilers/issues/166))

### Miscellaneous Tasks

- Release 0.10.1

## [0.10.0](https://github.com/foundry-rs/compilers/releases/tag/v0.10.0) - 2024-07-18

### Bug Fixes

- Allow empty modifier body in AST ([#169](https://github.com/foundry-rs/compilers/issues/169))
- Avoid errors when parsing empty sourcemap ([#165](https://github.com/foundry-rs/compilers/issues/165))
- Fix inconsistent trailing slash in remappings ([#49](https://github.com/foundry-rs/compilers/issues/49))

### Features

- Add `eofVersion` config option ([#174](https://github.com/foundry-rs/compilers/issues/174))
- Allow passing extra cli args to solc + some cleanup ([#171](https://github.com/foundry-rs/compilers/issues/171))

### Miscellaneous Tasks

- Release 0.10.0

## [0.9.0](https://github.com/foundry-rs/compilers/releases/tag/v0.9.0) - 2024-06-29

### Bug Fixes

- Doctests ([#154](https://github.com/foundry-rs/compilers/issues/154))
- [flatten] Small bugs ([#153](https://github.com/foundry-rs/compilers/issues/153))

### Dependencies

- Cleanup workspace deps ([#158](https://github.com/foundry-rs/compilers/issues/158))

### Features

- Respect `paths.libraries` for Vyper ([#159](https://github.com/foundry-rs/compilers/issues/159))

### Miscellaneous Tasks

- Release 0.9.0
- Improve stripping file prefixes ([#164](https://github.com/foundry-rs/compilers/issues/164))
- Improve some trace-level logs ([#163](https://github.com/foundry-rs/compilers/issues/163))
- Remove most impl AsRef<str,Path> ([#157](https://github.com/foundry-rs/compilers/issues/157))
- Clarify version cache lock ([#160](https://github.com/foundry-rs/compilers/issues/160))
- Sort derives, derive Eq more ([#161](https://github.com/foundry-rs/compilers/issues/161))
- [meta] Update CODEOWNERS
- Release 0.8.0
- Rename foundry-compilers-project into foundry-compilers ([#152](https://github.com/foundry-rs/compilers/issues/152))
- Clippy
- Move lints to workspace ([#149](https://github.com/foundry-rs/compilers/issues/149))
- Remove unused files and workflow ([#148](https://github.com/foundry-rs/compilers/issues/148))

### Other

- Symlink readme
- Sync workflows

### Performance

- Cache --version output ([#144](https://github.com/foundry-rs/compilers/issues/144))

### Refactor

- Unify sources and filtered sources ([#162](https://github.com/foundry-rs/compilers/issues/162))
- [flatten] Move compilation logic into `Flattener` ([#143](https://github.com/foundry-rs/compilers/issues/143))
- Extract artifacts to a separate crate ([#142](https://github.com/foundry-rs/compilers/issues/142))

### Testing

- Use similar-asserts ([#145](https://github.com/foundry-rs/compilers/issues/145))

## [0.7.0](https://github.com/foundry-rs/compilers/releases/tag/v0.7.0) - 2024-06-11

### Bug Fixes

- Always fix windows line endings ([#139](https://github.com/foundry-rs/compilers/issues/139))

### Features

- Track and cache context of each compiler invocation ([#140](https://github.com/foundry-rs/compilers/issues/140))

### Miscellaneous Tasks

- Release 0.7.0

## [0.6.2](https://github.com/foundry-rs/compilers/releases/tag/v0.6.2) - 2024-06-06

### Bug Fixes

- Better tracking of cache entries ([#138](https://github.com/foundry-rs/compilers/issues/138))

### Miscellaneous Tasks

- Release 0.6.2

## [0.6.1](https://github.com/foundry-rs/compilers/releases/tag/v0.6.1) - 2024-06-05

### Bug Fixes

- Small sparse output updates ([#137](https://github.com/foundry-rs/compilers/issues/137))
- Version resolution ([#136](https://github.com/foundry-rs/compilers/issues/136))
- Vyper 0.4 support ([#134](https://github.com/foundry-rs/compilers/issues/134))

### Miscellaneous Tasks

- Release 0.6.1
- Sync cliff.toml

### Refactor

- Sparse output ([#135](https://github.com/foundry-rs/compilers/issues/135))

## [0.6.0](https://github.com/foundry-rs/compilers/releases/tag/v0.6.0) - 2024-06-03

### Dependencies

- [deps] Bump itertools ([#133](https://github.com/foundry-rs/compilers/issues/133))

### Features

- Allow multiple languages for compilers ([#128](https://github.com/foundry-rs/compilers/issues/128))

### Miscellaneous Tasks

- Release 0.6.0

## [0.5.2](https://github.com/foundry-rs/compilers/releases/tag/v0.5.2) - 2024-06-01

### Features

- Make CompactContractBytecodeCow implement Artifact ([#130](https://github.com/foundry-rs/compilers/issues/130))

### Miscellaneous Tasks

- Release 0.5.2
- Clippy ([#132](https://github.com/foundry-rs/compilers/issues/132))

### Performance

- Reduce size of source map ([#131](https://github.com/foundry-rs/compilers/issues/131))

## [0.5.1](https://github.com/foundry-rs/compilers/releases/tag/v0.5.1) - 2024-05-23

### Bug Fixes

- Update vyper path resolution logic ([#127](https://github.com/foundry-rs/compilers/issues/127))
- Relax trait bounds ([#126](https://github.com/foundry-rs/compilers/issues/126))

### Miscellaneous Tasks

- Release 0.5.1

## [0.5.0](https://github.com/foundry-rs/compilers/releases/tag/v0.5.0) - 2024-05-21

### Features

- Vyper imports parser ([#125](https://github.com/foundry-rs/compilers/issues/125))

### Miscellaneous Tasks

- Release 0.5.0
- Swap generics on `Project` ([#124](https://github.com/foundry-rs/compilers/issues/124))

## [0.4.3](https://github.com/foundry-rs/compilers/releases/tag/v0.4.3) - 2024-05-13

### Bug Fixes

- Re-enable yul settings sanitization ([#122](https://github.com/foundry-rs/compilers/issues/122))

### Miscellaneous Tasks

- Release 0.4.3

## [0.4.2](https://github.com/foundry-rs/compilers/releases/tag/v0.4.2) - 2024-05-13

### Bug Fixes

- Do not remove dirty artifacts from disk ([#123](https://github.com/foundry-rs/compilers/issues/123))

### Miscellaneous Tasks

- Release 0.4.2

## [0.4.1](https://github.com/foundry-rs/compilers/releases/tag/v0.4.1) - 2024-05-07

### Bug Fixes

- Absolute paths in build info ([#121](https://github.com/foundry-rs/compilers/issues/121))

### Features

- Add a few Solc install helpers back ([#120](https://github.com/foundry-rs/compilers/issues/120))

### Miscellaneous Tasks

- Release 0.4.1

## [0.4.0](https://github.com/foundry-rs/compilers/releases/tag/v0.4.0) - 2024-05-03

### Features

- Compiler abstraction ([#115](https://github.com/foundry-rs/compilers/issues/115))

### Miscellaneous Tasks

- Release 0.4.0

## [0.3.20](https://github.com/foundry-rs/compilers/releases/tag/v0.3.20) - 2024-04-30

### Bug Fixes

- Short-circuit symlink cycle ([#117](https://github.com/foundry-rs/compilers/issues/117))
- Add checks for != root folder ([#116](https://github.com/foundry-rs/compilers/issues/116))

### Miscellaneous Tasks

- Release 0.3.20

## [0.3.19](https://github.com/foundry-rs/compilers/releases/tag/v0.3.19) - 2024-04-22

### Bug Fixes

- Remove `simpleCounterForLoopUncheckedIncrement` from `--ir-minimum` ([#114](https://github.com/foundry-rs/compilers/issues/114))
- Add YulCase and YulTypedName to NodeType ([#111](https://github.com/foundry-rs/compilers/issues/111))
- Use serde default for optimizer ([#109](https://github.com/foundry-rs/compilers/issues/109))
- Replace line endings on Windows to enforce deterministic metadata ([#108](https://github.com/foundry-rs/compilers/issues/108))

### Miscellaneous Tasks

- Release 0.3.19

## [0.3.18](https://github.com/foundry-rs/compilers/releases/tag/v0.3.18) - 2024-04-19

### Miscellaneous Tasks

- Release 0.3.18
- Warn unused ([#106](https://github.com/foundry-rs/compilers/issues/106))

### Other

- Update yansi to 1.0 ([#107](https://github.com/foundry-rs/compilers/issues/107))

## [0.3.17](https://github.com/foundry-rs/compilers/releases/tag/v0.3.17) - 2024-04-17

### Bug Fixes

- Dirty files detection ([#105](https://github.com/foundry-rs/compilers/issues/105))

### Features

- Additional helpers for contract name -> path lookup ([#103](https://github.com/foundry-rs/compilers/issues/103))

### Miscellaneous Tasks

- Release 0.3.17

## [0.3.16](https://github.com/foundry-rs/compilers/releases/tag/v0.3.16) - 2024-04-17

### Bug Fixes

- Invalidate cache for out-of-scope entries ([#104](https://github.com/foundry-rs/compilers/issues/104))

### Features

- Optimization field (simpleCounterForLoopUncheckedIncrement) ([#100](https://github.com/foundry-rs/compilers/issues/100))

### Miscellaneous Tasks

- Release 0.3.16
- Remove main fn ([#101](https://github.com/foundry-rs/compilers/issues/101))

## [0.3.15](https://github.com/foundry-rs/compilers/releases/tag/v0.3.15) - 2024-04-12

### Dependencies

- [deps] Bump svm to 0.5 ([#97](https://github.com/foundry-rs/compilers/issues/97))

### Miscellaneous Tasks

- Release 0.3.15
- Derive `Clone` for `Project` ([#98](https://github.com/foundry-rs/compilers/issues/98))

## [0.3.14](https://github.com/foundry-rs/compilers/releases/tag/v0.3.14) - 2024-04-03

### Bug Fixes

- Set evmversion::cancun as default ([#94](https://github.com/foundry-rs/compilers/issues/94))

### Dependencies

- Bump alloy-core ([#96](https://github.com/foundry-rs/compilers/issues/96))

### Miscellaneous Tasks

- Release 0.3.14

## [0.3.13](https://github.com/foundry-rs/compilers/releases/tag/v0.3.13) - 2024-03-18

### Miscellaneous Tasks

- Release 0.3.13
- Svm04 ([#93](https://github.com/foundry-rs/compilers/issues/93))

## [0.3.12](https://github.com/foundry-rs/compilers/releases/tag/v0.3.12) - 2024-03-18

### Miscellaneous Tasks

- Release 0.3.12
- Update svm ([#92](https://github.com/foundry-rs/compilers/issues/92))

## [0.3.11](https://github.com/foundry-rs/compilers/releases/tag/v0.3.11) - 2024-03-13

### Miscellaneous Tasks

- Release 0.3.11

### Refactor

- Caching logic ([#90](https://github.com/foundry-rs/compilers/issues/90))

## [0.3.10](https://github.com/foundry-rs/compilers/releases/tag/v0.3.10) - 2024-03-11

### Features

- Use cached artifacts if solc config is almost the same ([#87](https://github.com/foundry-rs/compilers/issues/87))

### Miscellaneous Tasks

- Release 0.3.10

### Other

- Helper for `OutputSelection` ([#89](https://github.com/foundry-rs/compilers/issues/89))
- Add `CARGO_TERM_COLOR` env ([#86](https://github.com/foundry-rs/compilers/issues/86))

### Refactor

- Extra files logic ([#88](https://github.com/foundry-rs/compilers/issues/88))

## [0.3.9](https://github.com/foundry-rs/compilers/releases/tag/v0.3.9) - 2024-02-22

### Bug Fixes

- Account for Solc inexplicably not formatting the message ([#85](https://github.com/foundry-rs/compilers/issues/85))

### Miscellaneous Tasks

- Release 0.3.9

## [0.3.8](https://github.com/foundry-rs/compilers/releases/tag/v0.3.8) - 2024-02-22

### Bug Fixes

- Always treat errors as error ([#84](https://github.com/foundry-rs/compilers/issues/84))
- Make solc emit ir with extra_output_files=ir ([#82](https://github.com/foundry-rs/compilers/issues/82))

### Miscellaneous Tasks

- Release 0.3.8
- Use Path::new instead of PathBuf::from ([#83](https://github.com/foundry-rs/compilers/issues/83))

## [0.3.7](https://github.com/foundry-rs/compilers/releases/tag/v0.3.7) - 2024-02-20

### Bug Fixes

- Don't bother formatting old solc errors ([#81](https://github.com/foundry-rs/compilers/issues/81))
- Empty error message formatting ([#77](https://github.com/foundry-rs/compilers/issues/77))

### Miscellaneous Tasks

- Release 0.3.7
- Print compiler input as JSON in traces ([#79](https://github.com/foundry-rs/compilers/issues/79))
- Remove unused imports ([#80](https://github.com/foundry-rs/compilers/issues/80))
- Reduce trace output ([#78](https://github.com/foundry-rs/compilers/issues/78))

## [0.3.6](https://github.com/foundry-rs/compilers/releases/tag/v0.3.6) - 2024-02-13

### Miscellaneous Tasks

- Release 0.3.6

### Other

- Small flattener features ([#75](https://github.com/foundry-rs/compilers/issues/75))

## [0.3.5](https://github.com/foundry-rs/compilers/releases/tag/v0.3.5) - 2024-02-10

### Bug Fixes

- Fix `DoWhileStatement` AST ([#74](https://github.com/foundry-rs/compilers/issues/74))

### Miscellaneous Tasks

- Release 0.3.5

## [0.3.4](https://github.com/foundry-rs/compilers/releases/tag/v0.3.4) - 2024-02-09

### Dependencies

- Option to ignore warnings from dependencies in foundry.toml ([#69](https://github.com/foundry-rs/compilers/issues/69))

### Miscellaneous Tasks

- Release 0.3.4

## [0.3.3](https://github.com/foundry-rs/compilers/releases/tag/v0.3.3) - 2024-02-08

### Miscellaneous Tasks

- Release 0.3.3

### Other

- Helper method for `Libraries` ([#72](https://github.com/foundry-rs/compilers/issues/72))

## [0.3.2](https://github.com/foundry-rs/compilers/releases/tag/v0.3.2) - 2024-02-07

### Bug Fixes

- Also cleanup build info dir ([#71](https://github.com/foundry-rs/compilers/issues/71))

### Miscellaneous Tasks

- Release 0.3.2

## [0.3.1](https://github.com/foundry-rs/compilers/releases/tag/v0.3.1) - 2024-02-02

### Miscellaneous Tasks

- Release 0.3.1

### Other

- Flatten fix ([#68](https://github.com/foundry-rs/compilers/issues/68))

## [0.3.0](https://github.com/foundry-rs/compilers/releases/tag/v0.3.0) - 2024-01-31

### Dependencies

- Remove unnecessary dependencies ([#65](https://github.com/foundry-rs/compilers/issues/65))
- Bump to 0.8.24 in tests ([#59](https://github.com/foundry-rs/compilers/issues/59))

### Miscellaneous Tasks

- Release 0.3.0
- Enable some lints ([#64](https://github.com/foundry-rs/compilers/issues/64))
- Remove wasm cfgs ([#61](https://github.com/foundry-rs/compilers/issues/61))
- Add more tracing around spawning Solc ([#57](https://github.com/foundry-rs/compilers/issues/57))
- Rename output to into_output ([#56](https://github.com/foundry-rs/compilers/issues/56))
- Add some tracing ([#55](https://github.com/foundry-rs/compilers/issues/55))

### Other

- Flatten fixes ([#63](https://github.com/foundry-rs/compilers/issues/63))
- Update actions@checkout ([#66](https://github.com/foundry-rs/compilers/issues/66))
- Add concurrency to ci.yml ([#62](https://github.com/foundry-rs/compilers/issues/62))
- Fix tests name ([#60](https://github.com/foundry-rs/compilers/issues/60))

### Refactor

- Rewrite examples without wrapper functions and with no_run ([#58](https://github.com/foundry-rs/compilers/issues/58))

### Testing

- Ignore old solc version test ([#67](https://github.com/foundry-rs/compilers/issues/67))

## [0.2.5](https://github.com/foundry-rs/compilers/releases/tag/v0.2.5) - 2024-01-29

### Miscellaneous Tasks

- Release 0.2.5
- [clippy] Make clippy happy ([#54](https://github.com/foundry-rs/compilers/issues/54))

### Other

- New flattening impl ([#52](https://github.com/foundry-rs/compilers/issues/52))

## [0.2.4](https://github.com/foundry-rs/compilers/releases/tag/v0.2.4) - 2024-01-27

### Dependencies

- Bump svm builds ([#53](https://github.com/foundry-rs/compilers/issues/53))

### Miscellaneous Tasks

- Release 0.2.4

## [0.2.3](https://github.com/foundry-rs/compilers/releases/tag/v0.2.3) - 2024-01-26

### Features

- Add EVM version Cancun ([#51](https://github.com/foundry-rs/compilers/issues/51))

### Miscellaneous Tasks

- Release 0.2.3
- Add unreleased section to cliff.toml
- Add error severity fn helpers ([#48](https://github.com/foundry-rs/compilers/issues/48))

### Other

- Small fixes to typed AST ([#50](https://github.com/foundry-rs/compilers/issues/50))

## [0.2.2](https://github.com/foundry-rs/compilers/releases/tag/v0.2.2) - 2024-01-19

### Miscellaneous Tasks

- Release 0.2.2

### Other

- Rewrite dirty files discovery ([#45](https://github.com/foundry-rs/compilers/issues/45))

## [0.2.1](https://github.com/foundry-rs/compilers/releases/tag/v0.2.1) - 2024-01-10

### Miscellaneous Tasks

- Release 0.2.1
- Exclude useless directories
- Exclude useless directories

## [0.2.0](https://github.com/foundry-rs/compilers/releases/tag/v0.2.0) - 2024-01-10

### Dependencies

- [deps] Bump alloy ([#42](https://github.com/foundry-rs/compilers/issues/42))

### Miscellaneous Tasks

- Release 0.2.0

## [0.1.4](https://github.com/foundry-rs/compilers/releases/tag/v0.1.4) - 2024-01-06

### Bug Fixes

- Account for unicode width in error syntax highlighting ([#40](https://github.com/foundry-rs/compilers/issues/40))

### Miscellaneous Tasks

- Release 0.1.4

## [0.1.3](https://github.com/foundry-rs/compilers/releases/tag/v0.1.3) - 2024-01-05

### Features

- Add evmVersion to settings ([#41](https://github.com/foundry-rs/compilers/issues/41))
- Use Box<dyn> in sparse functions ([#39](https://github.com/foundry-rs/compilers/issues/39))

### Miscellaneous Tasks

- Release 0.1.3
- Clippies and such ([#38](https://github.com/foundry-rs/compilers/issues/38))
- Purge tracing imports ([#37](https://github.com/foundry-rs/compilers/issues/37))

## [0.1.2](https://github.com/foundry-rs/compilers/releases/tag/v0.1.2) - 2023-12-29

### Bug Fixes

- Create valid Standard JSON to verify for projects with symlinks ([#35](https://github.com/foundry-rs/compilers/issues/35))
- Create verifiable Standard JSON for projects with external files ([#36](https://github.com/foundry-rs/compilers/issues/36))

### Features

- Add more getter methods to bytecode structs ([#30](https://github.com/foundry-rs/compilers/issues/30))

### Miscellaneous Tasks

- Release 0.1.2
- Add `set_compiled_artifacts` to ProjectCompileOutput impl ([#33](https://github.com/foundry-rs/compilers/issues/33))

### Other

- Trim test matrix ([#32](https://github.com/foundry-rs/compilers/issues/32))

### Styling

- Update rustfmt config ([#31](https://github.com/foundry-rs/compilers/issues/31))

## [0.1.1](https://github.com/foundry-rs/compilers/releases/tag/v0.1.1) - 2023-11-23

### Bug Fixes

- Default Solidity language string ([#28](https://github.com/foundry-rs/compilers/issues/28))
- [`ci`] Put flags inside matrix correctly ([#20](https://github.com/foundry-rs/compilers/issues/20))

### Dependencies

- Bump Alloy
- Bump solc ([#21](https://github.com/foundry-rs/compilers/issues/21))

### Miscellaneous Tasks

- Release 0.1.1
- [meta] Update CODEOWNERS
- Remove LosslessAbi ([#27](https://github.com/foundry-rs/compilers/issues/27))

### Performance

- Don't prettify json when not necessary ([#24](https://github.com/foundry-rs/compilers/issues/24))

### Styling

- Toml
- More test in report/compiler.rs and Default trait for CompilerInput ([#19](https://github.com/foundry-rs/compilers/issues/19))

## [0.1.0](https://github.com/foundry-rs/compilers/releases/tag/v0.1.0) - 2023-11-07

### Bug Fixes

- Add changelog.sh ([#18](https://github.com/foundry-rs/compilers/issues/18))

### Dependencies

- Bump solang parser to 0.3.3 ([#11](https://github.com/foundry-rs/compilers/issues/11))
- Remove unneeded deps ([#4](https://github.com/foundry-rs/compilers/issues/4))

### Features

- [`ci`] Add unused deps workflow ([#15](https://github.com/foundry-rs/compilers/issues/15))
- Migration to Alloy ([#3](https://github.com/foundry-rs/compilers/issues/3))
- [`ci`] Add deny deps CI ([#6](https://github.com/foundry-rs/compilers/issues/6))
- [`ci`] Add & enable ci/cd ([#1](https://github.com/foundry-rs/compilers/issues/1))
- Move ethers-solc into foundry-compilers

### Miscellaneous Tasks

- Release 0.1.0
- Add missing cargo.toml fields + changelog tag ([#17](https://github.com/foundry-rs/compilers/issues/17))
- Add missing telegram url ([#14](https://github.com/foundry-rs/compilers/issues/14))
- Remove alloy-dyn-abi as its an unused dep ([#12](https://github.com/foundry-rs/compilers/issues/12))
- Make clippy happy ([#10](https://github.com/foundry-rs/compilers/issues/10))
- Run ci on main ([#5](https://github.com/foundry-rs/compilers/issues/5))
- Add more files to gitignore ([#2](https://github.com/foundry-rs/compilers/issues/2))
- Correct readme

### Other

- Repo improvements ([#13](https://github.com/foundry-rs/compilers/issues/13))

<!-- generated by git-cliff -->
