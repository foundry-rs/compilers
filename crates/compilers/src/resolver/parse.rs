use foundry_compilers_core::utils;
use semver::VersionReq;
use solar_parse::{ast, interface::sym};
use solar_sema::interface;
use std::{
    ops::Range,
    path::{Path, PathBuf},
};

/// Solidity parser.
///
/// Holds a [`solar_sema::Compiler`] that is used to parse sources incrementally.
/// After project compilation ([`Graph::resolve`]), this will contain all sources parsed by
/// [`Graph`].
///
/// This state is currently lost on `Clone`.
///
/// [`Graph`]: crate::Graph
/// [`Graph::resolve`]: crate::Graph::resolve
#[derive(derive_more::Debug)]
pub struct SolParser {
    #[debug(ignore)]
    pub compiler: solar_sema::Compiler,
}

impl Clone for SolParser {
    fn clone(&self) -> Self {
        Self {
            compiler: solar_sema::Compiler::new(Self::session_with_opts(
                self.compiler.sess().opts.clone(),
            )),
        }
    }
}

impl SolParser {
    pub(crate) fn session_with_opts(
        opts: solar_sema::interface::config::Opts,
    ) -> solar_sema::interface::Session {
        let sess = solar_sema::interface::Session::builder()
            .with_buffer_emitter(Default::default())
            .opts(opts)
            .build();
        sess.source_map().set_file_loader(FileLoader);
        sess
    }
}

struct FileLoader;
impl interface::source_map::FileLoader for FileLoader {
    fn canonicalize_path(&self, path: &Path) -> std::io::Result<PathBuf> {
        interface::source_map::RealFileLoader.canonicalize_path(path)
    }

    fn load_stdin(&self) -> std::io::Result<String> {
        interface::source_map::RealFileLoader.load_stdin()
    }

    fn load_file(&self, path: &Path) -> std::io::Result<String> {
        interface::source_map::RealFileLoader.load_file(path).map(|s| {
            if s.contains('\r') {
                s.replace('\r', "")
            } else {
                s
            }
        })
    }

    fn load_binary_file(&self, path: &Path) -> std::io::Result<Vec<u8>> {
        interface::source_map::RealFileLoader.load_binary_file(path)
    }
}

/// Represents various information about a Solidity file.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SolData {
    pub license: Option<Spanned<String>>,
    pub version: Option<Spanned<String>>,
    pub experimental: Option<Spanned<String>>,
    pub imports: Vec<Spanned<SolImport>>,
    pub version_req: Option<VersionReq>,
    pub libraries: Vec<SolLibrary>,
    pub contract_names: Vec<String>,
    pub is_yul: bool,
    pub parse_result: Result<(), String>,
}

impl SolData {
    /// Returns the result of parsing the file.
    pub fn parse_result(&self) -> crate::Result<()> {
        self.parse_result.clone().map_err(crate::SolcError::ParseError)
    }

    #[allow(dead_code)]
    pub fn fmt_version<W: std::fmt::Write>(
        &self,
        f: &mut W,
    ) -> std::result::Result<(), std::fmt::Error> {
        if let Some(version) = &self.version {
            write!(f, "({})", version.data)?;
        }
        Ok(())
    }

    /// Extracts the useful data from a solidity source
    ///
    /// This will attempt to parse the solidity AST and extract the imports and version pragma. If
    /// parsing fails, we'll fall back to extract that info via regex
    #[instrument(name = "SolData::parse", skip_all)]
    pub fn parse(content: &str, file: &Path) -> Self {
        match crate::parse_one_source(content, file, |sess, ast| {
            SolDataBuilder::parse(content, file, Ok((sess, ast)))
        }) {
            Ok(data) => data,
            Err(e) => {
                let e = e.to_string();
                trace!("failed parsing {file:?}: {e}");
                SolDataBuilder::parse(content, file, Err(Some(e)))
            }
        }
    }

    pub(crate) fn parse_from(
        sess: &solar_sema::interface::Session,
        s: &solar_sema::Source<'_>,
    ) -> Self {
        let content = s.file.src.as_str();
        let file = s.file.name.as_real().unwrap();
        let ast = s.ast.as_ref().map(|ast| (sess, ast)).ok_or(None);
        SolDataBuilder::parse(content, file, ast)
    }

    /// Parses the version pragma and returns the corresponding SemVer version requirement.
    ///
    /// See [`parse_version_req`](Self::parse_version_req).
    pub fn parse_version_pragma(pragma: &str) -> Option<Result<VersionReq, semver::Error>> {
        let version = utils::find_version_pragma(pragma)?.as_str();
        Some(Self::parse_version_req(version))
    }

    /// Returns the corresponding SemVer version requirement for the solidity version.
    ///
    /// Note: This is a workaround for the fact that `VersionReq::parse` does not support whitespace
    /// separators and requires comma separated operators. See [VersionReq].
    pub fn parse_version_req(version: &str) -> Result<VersionReq, semver::Error> {
        let version = version.replace(' ', ",");

        // Somehow, Solidity semver without an operator is considered to be "exact",
        // but lack of operator automatically marks the operator as Caret, so we need
        // to manually patch it? :shrug:
        let exact = !matches!(version.get(..1), Some("*" | "^" | "=" | ">" | "<" | "~"));
        let mut version = VersionReq::parse(&version)?;
        if exact {
            version.comparators[0].op = semver::Op::Exact;
        }

        Ok(version)
    }
}

#[derive(Default)]
struct SolDataBuilder {
    version: Option<Spanned<String>>,
    experimental: Option<Spanned<String>>,
    imports: Vec<Spanned<SolImport>>,
    libraries: Vec<SolLibrary>,
    contract_names: Vec<String>,
    parse_err: Option<String>,
}

impl SolDataBuilder {
    fn parse(
        content: &str,
        file: &Path,
        ast: Result<
            (&solar_sema::interface::Session, &solar_parse::ast::SourceUnit<'_>),
            Option<String>,
        >,
    ) -> SolData {
        let mut builder = Self::default();
        match ast {
            Ok((sess, ast)) => builder.parse_from_ast(sess, ast),
            Err(err) => {
                builder.parse_from_regex(content);
                if let Some(err) = err {
                    builder.parse_err = Some(err);
                }
            }
        }
        builder.build(content, file)
    }

    fn parse_from_ast(
        &mut self,
        sess: &solar_sema::interface::Session,
        ast: &solar_parse::ast::SourceUnit<'_>,
    ) {
        eprintln!("parse_from_ast");
        for item in ast.items.iter() {
            let loc = sess.source_map().span_to_source(item.span).unwrap().1;
            dbg!((item.description(), item.name()), item.span, &loc);
            match &item.kind {
                ast::ItemKind::Pragma(pragma) => match &pragma.tokens {
                    ast::PragmaTokens::Version(name, req) if name.name == sym::solidity => {
                        self.version = Some(Spanned::new(req.to_string(), loc));
                    }
                    ast::PragmaTokens::Custom(name, value) if name.as_str() == "experimental" => {
                        let value =
                            value.as_ref().map(|v| v.as_str().to_string()).unwrap_or_default();
                        self.experimental = Some(Spanned::new(value, loc));
                    }
                    _ => {}
                },

                ast::ItemKind::Import(import) => {
                    let path = import.path.value.to_string();
                    let aliases = match &import.items {
                        ast::ImportItems::Plain(None) => &[][..],
                        ast::ImportItems::Plain(Some(alias)) | ast::ImportItems::Glob(alias) => {
                            &[(*alias, None)][..]
                        }
                        ast::ImportItems::Aliases(aliases) => aliases,
                    };
                    let sol_import = SolImport::new(PathBuf::from(path)).set_aliases(
                        aliases
                            .iter()
                            .map(|(id, alias)| match alias {
                                Some(al) => SolImportAlias::Contract(
                                    al.name.to_string(),
                                    id.name.to_string(),
                                ),
                                None => SolImportAlias::File(id.name.to_string()),
                            })
                            .collect(),
                    );
                    self.imports.push(Spanned::new(sol_import, loc));
                }

                ast::ItemKind::Contract(contract) => {
                    if contract.kind.is_library() {
                        self.libraries
                            .push(SolLibrary { is_inlined: library_is_inlined(contract) });
                    }
                    self.contract_names.push(contract.name.to_string());
                }

                _ => {}
            }
        }
    }

    fn parse_from_regex(&mut self, content: &str) {
        eprintln!("parse_from_regex");
        if self.version.is_none() {
            self.version = utils::capture_outer_and_inner(
                content,
                &utils::RE_SOL_PRAGMA_VERSION,
                &["version"],
            )
            .first()
            .map(|(cap, name)| Spanned::new(name.as_str().to_owned(), cap.range()));
        }
        if self.imports.is_empty() {
            self.imports = capture_imports(content);
        }
        if self.contract_names.is_empty() {
            utils::RE_CONTRACT_NAMES.captures_iter(content).for_each(|cap| {
                self.contract_names.push(cap[1].to_owned());
            });
        }
    }

    fn build(self, content: &str, file: &Path) -> SolData {
        let Self { version, experimental, imports, libraries, contract_names, parse_err } = self;
        let license = content.lines().next().and_then(|line| {
            utils::capture_outer_and_inner(
                line,
                &utils::RE_SOL_SDPX_LICENSE_IDENTIFIER,
                &["license"],
            )
            .first()
            .map(|(cap, l)| Spanned::new(l.as_str().to_owned(), cap.range()))
        });
        let version_req = version.as_ref().and_then(|v| SolData::parse_version_req(v.data()).ok());
        dbg!(SolData {
            license,
            version,
            experimental,
            imports,
            version_req,
            libraries,
            contract_names,
            is_yul: file.extension().is_some_and(|ext| ext == "yul"),
            parse_result: parse_err.map(Err).unwrap_or(Ok(())),
        })
    }
}

#[derive(Clone, Debug)]
pub struct SolImport {
    path: PathBuf,
    aliases: Vec<SolImportAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolImportAlias {
    File(String),
    Contract(String, String),
}

impl SolImport {
    pub fn new(path: PathBuf) -> Self {
        Self { path, aliases: vec![] }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn aliases(&self) -> &[SolImportAlias] {
        &self.aliases
    }

    fn set_aliases(mut self, aliases: Vec<SolImportAlias>) -> Self {
        self.aliases = aliases;
        self
    }
}

/// Minimal representation of a contract inside a solidity file
#[derive(Clone, Debug)]
pub struct SolLibrary {
    pub is_inlined: bool,
}

impl SolLibrary {
    /// Returns `true` if all functions of this library will be inlined.
    ///
    /// This checks if all functions are either internal or private, because internal functions can
    /// only be accessed from within the current contract or contracts deriving from it. They cannot
    /// be accessed externally. Since they are not exposed to the outside through the contract’s
    /// ABI, they can take parameters of internal types like mappings or storage references.
    ///
    /// See also <https://docs.soliditylang.org/en/latest/contracts.html#libraries>
    pub fn is_inlined(&self) -> bool {
        self.is_inlined
    }
}

/// A spanned item.
#[derive(Clone, Debug)]
pub struct Spanned<T> {
    /// The byte range of `data` in the file.
    pub span: Range<usize>,
    /// The data of the item.
    pub data: T,
}

impl<T> Spanned<T> {
    /// Creates a new data unit with the given data and location.
    pub fn new(data: T, span: Range<usize>) -> Self {
        Self { data, span }
    }

    /// Returns the underlying data.
    pub fn data(&self) -> &T {
        &self.data
    }

    /// Returns the location.
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// Returns the location adjusted by an offset.
    ///
    /// Used to determine new position of the unit within the file after content manipulation.
    pub fn loc_by_offset(&self, offset: isize) -> Range<usize> {
        utils::range_by_offset(&self.span, offset)
    }
}

fn library_is_inlined(contract: &ast::ItemContract<'_>) -> bool {
    contract
        .body
        .iter()
        .filter_map(|item| match &item.kind {
            ast::ItemKind::Function(f) => Some(f),
            _ => None,
        })
        .all(|f| {
            !matches!(
                f.header.visibility.map(|v| *v),
                Some(ast::Visibility::Public | ast::Visibility::External)
            )
        })
}

/// Capture the import statement information together with aliases
pub fn capture_imports(content: &str) -> Vec<Spanned<SolImport>> {
    let mut imports = vec![];
    for cap in utils::RE_SOL_IMPORT.captures_iter(content) {
        if let Some(name_match) = ["p1", "p2", "p3", "p4"].iter().find_map(|name| cap.name(name)) {
            let statement_match = cap.get(0).unwrap();
            let mut aliases = vec![];
            for alias_cap in utils::RE_SOL_IMPORT_ALIAS.captures_iter(statement_match.as_str()) {
                if let Some(alias) = alias_cap.name("alias") {
                    let alias = alias.as_str().to_owned();
                    let import_alias = match alias_cap.name("target") {
                        Some(target) => SolImportAlias::Contract(alias, target.as_str().to_owned()),
                        None => SolImportAlias::File(alias),
                    };
                    aliases.push(import_alias);
                }
            }
            let sol_import =
                SolImport::new(PathBuf::from(name_match.as_str())).set_aliases(aliases);
            imports.push(Spanned::new(sol_import, statement_match.range()));
        }
    }
    imports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_version(version_req: Option<&str>, src: &str) {
        let data = SolData::parse(src, "test.sol".as_ref());
        assert_eq!(data.version_req, version_req.map(|v| v.parse().unwrap()), "src:\n{src}");
    }

    #[track_caller]
    fn assert_contract_names(names: &[&str], src: &str) {
        let data = SolData::parse(src, "test.sol".as_ref());
        assert_eq!(data.contract_names, names, "src:\n{src}");
    }

    #[test]
    fn soldata_parsing() {
        assert_version(None, "");
        assert_version(None, "contract C { }");

        // https://github.com/foundry-rs/foundry/issues/9349
        assert_version(
            Some(">=0.4.22, <0.6"),
            r#"
pragma solidity >=0.4.22 <0.6;

contract BugReport {
    function() external payable {
        deposit();
    }
    function deposit() public payable {}
}
        "#,
        );

        assert_contract_names(
            &["A", "B69$_", "C_", "$D"],
            r#"
    contract A {}
library B69$_ {}
abstract contract C_ {} interface $D {}

uint constant x = .1e10;
uint constant y = .1 ether;
        "#,
        );
    }

    #[test]
    fn can_capture_curly_imports() {
        let content = r#"
import { T } from "../Test.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {DsTest} from "ds-test/test.sol";
"#;

        let captured_imports =
            capture_imports(content).into_iter().map(|s| s.data.path).collect::<Vec<_>>();

        let expected =
            utils::find_import_paths(content).map(|m| m.as_str().into()).collect::<Vec<PathBuf>>();

        assert_eq!(captured_imports, expected);

        assert_eq!(
            captured_imports,
            vec![
                PathBuf::from("../Test.sol"),
                "@openzeppelin/contracts/utils/ReentrancyGuard.sol".into(),
                "ds-test/test.sol".into(),
            ],
        );
    }

    #[test]
    fn cap_capture_aliases() {
        let content = r#"
import * as T from "./Test.sol";
import { DsTest as Test } from "ds-test/test.sol";
import "ds-test/test.sol" as Test;
import { FloatMath as Math, Math as FloatMath } from "./Math.sol";
"#;

        let caputred_imports =
            capture_imports(content).into_iter().map(|s| s.data.aliases).collect::<Vec<_>>();
        assert_eq!(
            caputred_imports,
            vec![
                vec![SolImportAlias::File("T".into())],
                vec![SolImportAlias::Contract("Test".into(), "DsTest".into())],
                vec![SolImportAlias::File("Test".into())],
                vec![
                    SolImportAlias::Contract("Math".into(), "FloatMath".into()),
                    SolImportAlias::Contract("FloatMath".into(), "Math".into()),
                ],
            ]
        );
    }
}
