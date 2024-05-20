use core::fmt;
use std::path::PathBuf;

use crate::{
    artifacts::{error::SourceLocation, Severity},
    compilers::CompilationError,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VyperSourceLocation {
    file: PathBuf,
    #[serde(rename = "lineno")]
    line: Option<u64>,
    #[serde(rename = "col_offset")]
    offset: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VyperCompilationError {
    pub message: String,
    pub severity: Severity,
    pub source_location: Option<VyperSourceLocation>,
    pub formatted_message: Option<String>,
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

impl fmt::Display for VyperCompilationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}
