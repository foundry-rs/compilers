use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use semver::VersionReq;

pub trait CompilerSettingsRestrictions: Debug + Sync + Send + Clone + Default {
    fn merge(&mut self, other: &Self);
}

/// Combines [CompilerVersionRestriction] with a restrictions on compiler versions for a given
/// source file.
#[derive(Debug, Clone, Default)]
pub struct RestrictionsWithVersion<T> {
    pub version: Option<VersionReq>,
    pub settings: T,
}

impl<T> Deref for RestrictionsWithVersion<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

impl<T> DerefMut for RestrictionsWithVersion<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}
