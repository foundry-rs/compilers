use alloy_primitives::hex;
use foundry_compilers_core::{error::SolcIoError, utils};
use md5::Digest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

/// An ordered list of files and their source
pub type Sources = BTreeMap<PathBuf, Source>;

/// Content of a solidity file
///
/// This contains the actual source code of a file
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Source {
    /// Content of the file
    ///
    /// This is an `Arc` because it may be cloned. If the [Graph](crate::resolver::Graph) of the
    /// project contains multiple conflicting versions then the same [Source] may be required by
    /// conflicting versions and needs to be duplicated.
    pub content: Arc<String>,
}

impl Source {
    /// Creates a new instance of [Source] with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self { content: Arc::new(content.into()) }
    }

    /// Reads the file's content
    #[instrument(level = "debug", skip_all, err)]
    pub fn read(file: impl AsRef<Path>) -> Result<Self, SolcIoError> {
        let file = file.as_ref();
        trace!(file=%file.display());
        let mut content = fs::read_to_string(file).map_err(|err| SolcIoError::new(err, file))?;

        // Normalize line endings to ensure deterministic metadata.
        if content.contains('\r') {
            content = content.replace("\r\n", "\n");
        }

        Ok(Self::new(content))
    }

    /// Recursively finds all source files under the given dir path and reads them all
    pub fn read_all_from(
        dir: impl AsRef<Path>,
        extensions: &[&str],
    ) -> Result<Sources, SolcIoError> {
        Self::read_all_files(utils::source_files(dir, extensions))
    }

    /// Recursively finds all solidity and yul files under the given dir path and reads them all
    pub fn read_sol_yul_from(dir: impl AsRef<Path>) -> Result<Sources, SolcIoError> {
        Self::read_all_from(dir, utils::SOLC_EXTENSIONS)
    }

    /// Reads all source files of the given vec
    ///
    /// Depending on the len of the vec it will try to read the files in parallel
    pub fn read_all_files(files: Vec<PathBuf>) -> Result<Sources, SolcIoError> {
        Self::read_all(files)
    }

    /// Reads all files
    pub fn read_all<T, I>(files: I) -> Result<Sources, SolcIoError>
    where
        I: IntoIterator<Item = T>,
        T: Into<PathBuf>,
    {
        files
            .into_iter()
            .map(Into::into)
            .map(|file| Self::read(&file).map(|source| (file, source)))
            .collect()
    }

    /// Parallelized version of `Self::read_all` that reads all files using a parallel iterator
    ///
    /// NOTE: this is only expected to be faster than `Self::read_all` if the given iterator
    /// contains at least several paths or the files are rather large.
    pub fn par_read_all<T, I>(files: I) -> Result<Sources, SolcIoError>
    where
        I: IntoIterator<Item = T>,
        <I as IntoIterator>::IntoIter: Send,
        T: Into<PathBuf> + Send,
    {
        use rayon::{iter::ParallelBridge, prelude::ParallelIterator};
        files
            .into_iter()
            .par_bridge()
            .map(Into::into)
            .map(|file| Self::read(&file).map(|source| (file, source)))
            .collect()
    }

    /// Generate a non-cryptographically secure checksum of the file's content
    pub fn content_hash(&self) -> String {
        let mut hasher = md5::Md5::new();
        hasher.update(self);
        let result = hasher.finalize();
        hex::encode(result)
    }
}

#[cfg(feature = "async")]
impl Source {
    /// async version of `Self::read`
    pub async fn async_read(file: impl AsRef<Path>) -> Result<Self, SolcIoError> {
        let file = file.as_ref();
        let mut content =
            tokio::fs::read_to_string(file).await.map_err(|err| SolcIoError::new(err, file))?;

        // Normalize line endings to ensure deterministic metadata.
        if content.contains('\r') {
            content = content.replace("\r\n", "\n");
        }

        Ok(Self::new(content))
    }

    /// Finds all source files under the given dir path and reads them all
    pub async fn async_read_all_from(
        dir: impl AsRef<Path>,
        extensions: &[&str],
    ) -> Result<Sources, SolcIoError> {
        Self::async_read_all(utils::source_files(dir.as_ref(), extensions)).await
    }

    /// async version of `Self::read_all`
    pub async fn async_read_all<T, I>(files: I) -> Result<Sources, SolcIoError>
    where
        I: IntoIterator<Item = T>,
        T: Into<PathBuf>,
    {
        futures_util::future::join_all(
            files
                .into_iter()
                .map(Into::into)
                .map(|file| async { Self::async_read(&file).await.map(|source| (file, source)) }),
        )
        .await
        .into_iter()
        .collect()
    }
}

impl AsRef<str> for Source {
    fn as_ref(&self) -> &str {
        &self.content
    }
}

impl AsRef<[u8]> for Source {
    fn as_ref(&self) -> &[u8] {
        self.content.as_bytes()
    }
}
