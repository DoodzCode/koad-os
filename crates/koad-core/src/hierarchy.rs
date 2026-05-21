//! Hierarchy Manager
//!
//! Resolves physical filesystem paths to logical Workspace Levels
//! based on the Citadel configuration.

use crate::config::KoadConfig;
use koad_proto::citadel::v5::WorkspaceLevel;
use std::path::Path;

/// Manages the resolution and validation of Workspace Levels.
pub struct HierarchyManager {
    config: KoadConfig,
}

impl HierarchyManager {
    /// Creates a new `HierarchyManager` with the provided configuration.
    pub fn new(config: KoadConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &KoadConfig {
        &self.config
    }

    /// Resolves an absolute path to its corresponding [`WorkspaceLevel`].
    pub fn resolve_level(&self, path: &Path) -> WorkspaceLevel {
        let home = &self.config.home;

        if path.starts_with(home) {
            return WorkspaceLevel::LevelCitadel;
        }

        let mut best_match: Option<(usize, WorkspaceLevel)> = None;

        for project in self.config.projects.values() {
            let project_path = Path::new(&project.path);
            if path.starts_with(project_path) {
                let depth = project_path.components().count();
                let level = match project.level.as_deref() {
                    Some("station") => WorkspaceLevel::LevelStation,
                    Some("citadel") => WorkspaceLevel::LevelCitadel,
                    Some("system") => WorkspaceLevel::LevelSystem,
                    _ => WorkspaceLevel::LevelOutpost,
                };

                if let Some(ref best) = best_match {
                    if depth > best.0 {
                        best_match = Some((depth, level));
                    }
                } else {
                    best_match = Some((depth, level));
                }
            }
        }

        if let Some((_, level)) = best_match {
            return level;
        }

        // Unrecognized paths default to LevelCitadel.
        // LevelSystem is reserved for paths explicitly registered as system-level projects.
        // An empty path (e.g. from koad-agent boot with no explicit project_root) and
        // unregistered paths both fall here — Citadel-level access is the safe default.
        WorkspaceLevel::LevelCitadel
    }

    /// Validates if an agent has permission to operate at a specific [`WorkspaceLevel`].
    pub fn validate_access(&self, agent_rank: &str, requested_level: WorkspaceLevel) -> bool {
        match requested_level {
            WorkspaceLevel::LevelSystem => agent_rank == "Admiral" || agent_rank == "Captain",
            WorkspaceLevel::LevelCitadel => {
                agent_rank == "Admiral" || agent_rank == "Captain" || agent_rank == "Officer"
            }
            WorkspaceLevel::LevelStation => agent_rank != "Crew",
            WorkspaceLevel::LevelOutpost => true,
            WorkspaceLevel::LevelUnspecified => false,
        }
    }
}
