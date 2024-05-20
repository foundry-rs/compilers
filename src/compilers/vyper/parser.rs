use crate::{
    compilers::ParsedSource,
    error::{Result, SolcError},
    resolver::parse::capture_outer_and_inner,
    utils::RE_VYPER_VERSION,
    ProjectPathsConfig,
};
use semver::VersionReq;
use std::path::{Path, PathBuf};
use winnow::{
    ascii::space1,
    combinator::{alt, opt, preceded, separated},
    token::take_till,
    PResult, Parser,
};

#[derive(Debug)]
pub struct VyperParsedSource {
    path: PathBuf,
    version_req: Option<VersionReq>,
    parsed_imports: Vec<Vec<String>>,
}

impl ParsedSource for VyperParsedSource {
    fn parse(content: &str, file: &Path) -> Self {
        let version_req = capture_outer_and_inner(content, &RE_VYPER_VERSION, &["version"])
            .first()
            .and_then(|(cap, _)| VersionReq::parse(cap.as_str()).ok());

        let parsed_imports = if let Ok(imports) = parse_imports(content) {
            let mut parsed = Vec::new();
            for import in imports {
                parsed.push(import.into_iter().map(|part| part.to_string()).collect());
            }
            parsed
        } else {
            Vec::new()
        };

        let path = file.to_path_buf();

        VyperParsedSource { path, version_req, parsed_imports }
    }

    fn version_req(&self) -> Option<&VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports<C>(&self, paths: &ProjectPathsConfig<C>) -> Result<Vec<PathBuf>> {
        let mut imports = Vec::new();
        'outer: for import in &self.parsed_imports {
            // skip built-in imports
            if import[0] == "vyper" {
                continue;
            }

            let mut dots_cnt = 0;
            while dots_cnt < import.len() && import[dots_cnt] == "" {
                dots_cnt += 1;
            }

            let mut candidate_dirs = Vec::new();

            if dots_cnt > 0 {
                let mut candidate_dir = Some(self.path.as_path());

                for _ in 0..dots_cnt {
                    candidate_dir = candidate_dir.and_then(|dir| dir.parent());
                }

                let candidate_dir = candidate_dir.ok_or_else(|| {
                    SolcError::msg(format!(
                        "Could not go {} levels up for import at {}",
                        dots_cnt,
                        self.path.display()
                    ))
                })?;

                candidate_dirs.push(candidate_dir);
            } else {
                if let Some(parent) = self.path.parent() {
                    candidate_dirs.push(parent);
                }
                candidate_dirs.push(paths.root.as_path());
            }

            for candidate_dir in candidate_dirs {
                let mut candidate = candidate_dir.to_path_buf();

                for part in &import[dots_cnt..] {
                    candidate = candidate.join(part);
                }

                candidate.set_extension("vy");

                if candidate.exists() {
                    imports.push(candidate);
                    continue 'outer;
                }
            }

            return Err(SolcError::msg(format!(
                "failed to resolve import {} at {}",
                import.join("."),
                self.path.display()
            )));
        }
        Ok(imports)
    }
}

fn parse_imports<'a>(content: &'a str) -> Result<Vec<Vec<&'a str>>> {
    let mut imports = Vec::new();

    for mut line in content.split('\n') {
        if let Ok(parts) = parse_import(&mut line) {
            imports.push(parts);
        }
    }

    Ok(imports)
}

fn parse_import<'a>(input: &mut &'a str) -> PResult<Vec<&'a str>> {
    (
        preceded(
            (alt(["from", "import"]), space1),
            separated(0.., take_till(0.., ['.', ' ']), '.'),
        ),
        opt(preceded((space1, "import", space1), take_till(0.., [' ']))),
    )
        .parse_next(input)
        .map(|(mut parts, last): (Vec<&str>, Option<&str>)| {
            if let Some(last) = last {
                parts.push(last);
            }
            parts
        })
}

#[cfg(test)]
mod tests {
    use winnow::Parser;

    use super::parse_import;

    #[test]
    fn can_parse_import() {
        assert_eq!(parse_import.parse("import one.two.three").unwrap(), ["one", "two", "three"]);
        assert_eq!(
            parse_import.parse("from one.two.three import four").unwrap(),
            ["one", "two", "three", "four"]
        );
        assert_eq!(parse_import.parse("from one import two").unwrap(), ["one", "two"]);
        assert_eq!(parse_import.parse("import one").unwrap(), ["one"]);
    }
}
