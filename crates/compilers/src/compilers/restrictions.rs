use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use semver::VersionReq;

pub trait CompilerSettingsRestrictions: Copy + Debug + Sync + Send + Clone + Default {
    fn merge(self, other: Self) -> Option<Self>;
}

/// Combines [CompilerSettingsRestrictions] with a restrictions on compiler versions for a given
/// source file.
#[derive(Debug, Clone, Default)]
pub struct RestrictionsWithVersion<T> {
    pub version: Option<VersionReq>,
    pub restrictions: T,
}

impl<T: CompilerSettingsRestrictions> RestrictionsWithVersion<T> {
    pub fn merge(&mut self, other: Self) {
        if let Some(version) = other.version {
            if let Some(self_version) = self.version.as_mut() {
                self_version.comparators.extend(version.comparators);
            } else {
                self.version = Some(version.clone());
            }
        }
        self.restrictions.merge(other.restrictions);
    }
}

impl<T> Deref for RestrictionsWithVersion<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.restrictions
    }
}

impl<T> DerefMut for RestrictionsWithVersion<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.restrictions
    }
}
