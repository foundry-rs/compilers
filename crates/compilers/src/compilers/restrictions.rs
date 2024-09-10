use crate::Result;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use semver::VersionReq;

pub trait CompilerSettingsRestrictions: Debug + Sync + Send + Clone + Copy + Default {
    fn merge(self, other: Self) -> Result<Self>;
}

/// Combines [CompilerSettingsRestrictions] with a restrictions on compiler versions for a given
/// source file.
#[derive(Debug, Clone, Default)]
pub struct RestrictionsWithVersion<T> {
    pub version: Option<VersionReq>,
    pub restrictions: T,
}

impl<T: CompilerSettingsRestrictions> RestrictionsWithVersion<T> {
    pub fn merge(mut self, other: Self) -> Result<Self> {
        if let Some(version) = other.version {
            if let Some(self_version) = self.version.as_mut() {
                self_version.comparators.extend(version.comparators);
            } else {
                self.version = Some(version.clone());
            }
        }
        self.restrictions = self.restrictions.merge(other.restrictions)?;
        Ok(self)
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
