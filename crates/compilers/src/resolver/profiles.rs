use crate::{ArtifactOutput, Compiler, CompilerSettings, Project};
use alloy_primitives::hex;
use md5::Digest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default)]
pub struct CompilationProfiles<S> {
    profiles: Vec<CompilationProfile<S>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilationProfile<S> {
    pub settings: S,
    pub id: String,
}

impl<S: serde::Serialize> CompilationProfile<S> {
    pub fn new(settings: S) -> Self {
        let mut hasher = md5::Md5::new();
        let ser = serde_json::to_string(&settings).unwrap();
        hasher.update(ser.as_bytes());
        let id = hex::encode(hasher.finalize());

        Self { settings, id }
    }
}

impl<S: CompilerSettings> CompilationProfiles<S> {
    pub fn new<C: Compiler<Settings = S>, T: ArtifactOutput>(project: &Project<C, T>) -> Self {
        let mut profiles = Self::default();

        profiles.add(project.settings.clone());

        profiles
    }

    pub fn add(&mut self, settings: S) -> usize {
        self.profiles.push(CompilationProfile::new(settings));

        self.profiles.len() - 1
    }

    pub fn find_or_create(&mut self, restrictions: &S::Restrictions) -> usize {
        if let Some((idx, _)) = self
            .profiles
            .iter()
            .enumerate()
            .find(|(_, profile)| profile.settings.satisfies_restrictions(restrictions))
        {
            return idx;
        }

        self.add(self.profiles[0].settings.apply_restrictions(restrictions))
    }

    pub fn get(&self, idx: usize) -> &CompilationProfile<S> {
        self.profiles.get(idx).unwrap()
    }
}
