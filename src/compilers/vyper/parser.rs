use crate::{
    compilers::ParsedSource, resolver::parse::capture_outer_and_inner, utils::RE_VYPER_VERSION,
    ProjectPathsConfig,
};
use semver::VersionReq;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct VyperParsedSource {
    version_req: Option<VersionReq>,
}

impl ParsedSource for VyperParsedSource {
    fn parse(content: &str, _file: &Path) -> Self {
        let version_req = capture_outer_and_inner(content, &RE_VYPER_VERSION, &["version"])
            .first()
            .and_then(|(cap, _)| VersionReq::parse(cap.as_str()).ok());
        VyperParsedSource { version_req }
    }

    fn version_req(&self) -> Option<&VersionReq> {
        self.version_req.as_ref()
    }

    fn resolve_imports(&self, _paths: &ProjectPathsConfig) -> Vec<PathBuf> {
        vec![]
    }
}
