use std::path::PathBuf;

use crate::{
    artifacts::{error::SourceLocation, Severity},
    compilers::CompilationError,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VyperSourceLocation {
    file: PathBuf,
    #[serde(rename = "lineno")]
    line: u64,
    #[serde(rename = "col_offset")]
    offset: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VyperCompilationError {
    pub message: String,
    pub severity: Severity,
    pub source_location: Option<VyperSourceLocation>,
}

impl CompilationError for VyperCompilationError {
    fn is_warning(&self) -> bool {
        self.severity.is_warning()
    }

    fn is_error(&self) -> bool {
        self.severity.is_error()
    }

    fn source_location(&self) -> Option<SourceLocation> {
        None
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn error_code(&self) -> Option<u64> {
        None
    }
}
