use crate::{
    compilers::version_manager::{CompilerVersion, CompilerVersionManager, VersionManagerError},
    Solc,
};
use semver::Version;
use std::collections::HashSet;

/// [CompilerVersionManager] implementation for [Solc]. Uses [svm] to install and manage versions.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct SolcVersionManager;

impl CompilerVersionManager for SolcVersionManager {
    type Compiler = Solc;

    fn all_versions(&self) -> Vec<CompilerVersion> {
        let mut all_versions = self.installed_versions();
        let mut uniques = all_versions
            .iter()
            .map(|v| {
                let v = v.as_ref();
                (v.major, v.minor, v.patch)
            })
            .collect::<HashSet<_>>();
        all_versions.extend(
            Solc::released_versions()
                .into_iter()
                .filter(|v| uniques.insert((v.major, v.minor, v.patch)))
                .map(CompilerVersion::Remote),
        );
        all_versions.sort_unstable();
        all_versions
    }

    fn installed_versions(&self) -> Vec<CompilerVersion> {
        Solc::installed_versions().into_iter().map(CompilerVersion::Installed).collect()
    }

    fn get_installed(&self, version: &Version) -> Result<Self::Compiler, VersionManagerError> {
        #[cfg(test)]
        crate::take_solc_installer_lock!(_lock);

        let s_version = version.to_string();

        let solc = Solc::svm_home()
            .ok_or_else(|| VersionManagerError::msg("svm home dir not found"))?
            .join(s_version.as_str())
            .join(format!("solc-{s_version}"));

        if !solc.is_file() {
            return Err(VersionManagerError::VersionNotInstalled(version.clone()));
        }
        Ok(Solc::new_with_version(solc, version.clone()))
    }

    fn install(&self, version: &Version) -> Result<Self::Compiler, VersionManagerError> {
        use crate::utils::RuntimeOrHandle;

        #[cfg(test)]
        crate::take_solc_installer_lock!(_lock);

        let version = if !version.pre.is_empty() || !version.build.is_empty() {
            Version::new(version.major, version.minor, version.patch)
        } else {
            version.clone()
        };

        trace!("blocking installing solc version \"{}\"", version);
        crate::report::solc_installation_start(&version);
        // The async version `svm::install` is used instead of `svm::blocking_intsall`
        // because the underlying `reqwest::blocking::Client` does not behave well
        // inside of a Tokio runtime. See: https://github.com/seanmonstar/reqwest/issues/1017
        match RuntimeOrHandle::new().block_on(svm::install(&version)) {
            Ok(path) => {
                crate::report::solc_installation_success(&version);
                Ok(Solc::new_with_version(path, version))
            }
            Err(err) => {
                crate::report::solc_installation_error(&version, &err.to_string());
                Err(VersionManagerError::IntallationFailed(Box::new(err)))
            }
        }
    }
}
