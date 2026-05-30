//! # KoadOS Command Sandbox
//!
//! Enforces security policies on agent commands before they are executed.
//! The Sandbox is config-driven, replacing the hardcoded legacy logic with
//! dynamic policies defined in `kernel.toml`.
//!
//! ## Modules
//! - Root: [`Sandbox`] — policy evaluation (blacklist, sanctuary, rank bypass).
//! - [`container`]: [`ContainerSandbox`] — Docker/Podman execution isolation
//!   (requires the `container` cargo feature).
//!
//! ## Principles
//! - **Sanctuary Rule**: Protects sensitive system paths from modification.
//! - **Blacklist Enforcement**: Blocks dangerous primitives (sudo, rm -rf).
//! - **Identity Awareness**: Allows administrative bypass for Admiral/Captain ranks.

pub mod container;

use koad_core::config::KoadConfig;
use tracing::warn;

/// Result of a sandbox policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyResult {
    /// Command is permitted.
    Allowed,
    /// Command is denied with a reason.
    Denied(String),
}

/// The Sandbox enforces security policies based on configuration and identity.
pub struct Sandbox {
    config: KoadConfig,
}

impl Sandbox {
    /// Create a new sandbox with the given configuration.
    pub fn new(config: KoadConfig) -> Self {
        Self { config }
    }

    /// Evaluates a command against the active policies.
    pub fn evaluate(&self, agent_name: &str, agent_rank: &str, command: &str) -> PolicyResult {
        if !self.config.sandbox.enabled {
            return PolicyResult::Allowed;
        }

        // 1. Administrative Bypass (Admiral/Captain)
        if agent_rank == "Admiral" || agent_rank == "Captain" {
            return PolicyResult::Allowed;
        }

        let cmd_lower = command.to_lowercase();

        // 2. Blacklist Check
        for blacklisted in &self.config.sandbox.blacklist {
            if cmd_lower.contains(&blacklisted.to_lowercase()) {
                warn!(agent = %agent_name, command = %command, match = %blacklisted, "Sandbox: Blacklist violation");
                return PolicyResult::Denied(format!(
                    "Command contains blacklisted phrase: '{}'",
                    blacklisted
                ));
            }
        }

        // 3. Sanctuary Check (Paths)
        if let PolicyResult::Denied(reason) = self.check_sanctuary(agent_name, command) {
            return PolicyResult::Denied(reason);
        }

        // 4. Efficiency Sentinel (Token Protection)
        if let PolicyResult::Denied(reason) = self.check_efficiency(agent_name, command) {
            return PolicyResult::Denied(reason);
        }

        PolicyResult::Allowed
    }

    fn check_efficiency(&self, agent_name: &str, command: &str) -> PolicyResult {
        // Enforce the "No-Read" Rule: Prevent full file reads of large files.
        // Heuristic: If read_file is used without start_line/end_line.
        if command.contains("read_file")
            && !command.contains("start_line")
            && !command.contains("end_line")
        {
            warn!(agent = %agent_name, command = %command, "Sandbox: Efficiency violation (Full file read)");
            return PolicyResult::Denied(
                "AIS ENFORCEMENT: Reading entire files is forbidden. \
                 Use the Crate API Maps in your Context Packet for discovery, \
                 or use `read_file` with `start_line` and `end_line` parameters."
                    .to_string(),
            );
        }
        PolicyResult::Allowed
    }

    fn check_sanctuary(&self, agent_name: &str, command: &str) -> PolicyResult {
        // Basic heuristic: if it looks like they are trying to touch a protected path
        // (Similar to legacy logic, but using config paths)
        for path in &self.config.sandbox.sanctuary {
            if command.contains(path)
                && (command.contains("rm ")
                    || command.contains("mv ")
                    || command.contains("cp ")
                    || command.contains("echo ")
                    || command.contains(">")
                    || command.contains("nano ")
                    || command.contains("vim "))
            {
                warn!(agent = %agent_name, command = %command, path = %path, "Sandbox: Sanctuary violation");
                return PolicyResult::Denied(format!(
                    "Attempt to modify protected path: '{}'",
                    path
                ));
            }
        }
        PolicyResult::Allowed
    }
}

use crate::container::{ContainerSandbox, SandboxResult};
use async_trait::async_trait;

/// Trait for pluggable sandbox execution backends.
#[async_trait]
pub trait SandboxRunner: Send + Sync {
    async fn execute(&self, command: &str) -> anyhow::Result<SandboxResult>;
}

#[async_trait]
impl SandboxRunner for ContainerSandbox {
    async fn execute(&self, command: &str) -> anyhow::Result<SandboxResult> {
        ContainerSandbox::execute(self, command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koad_core::config::KoadConfig;

    fn mock_config() -> KoadConfig {
        serde_json::from_str(
            r#"{
            "home": "/tmp/.koad-os",
            "system": { "version": "test" },
            "network": {
                "gateway_port": 7700,
                "gateway_addr": "127.0.0.1",
                "citadel_grpc_port": 50051,
                "citadel_grpc_addr": "127.0.0.1",
                "cass_grpc_port": 50052,
                "cass_grpc_addr": "127.0.0.1",
                "redis_socket": "/tmp/redis.sock",
                "citadel_socket": "/tmp/citadel.sock"
            },
            "storage": { "db_name": "koad_test.db", "drain_interval_secs": 60 },
            "sandbox": {
                "enabled": true,
                "blacklist": ["sudo ", "su "],
                "sanctuary": [".koad-os"]
            }
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn test_sandbox_denies_blacklist() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate("test-agent", "Crew", "sudo rm -rf /");
        assert!(matches!(res, PolicyResult::Denied(_)));
    }

    #[test]
    fn test_sandbox_denies_sanctuary() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate("test-agent", "Crew", "rm -rf .koad-os/koad.db");
        assert!(matches!(res, PolicyResult::Denied(_)));
    }

    #[test]
    fn test_sandbox_allows_safe_command() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate("test-agent", "Crew", "cargo build");
        assert_eq!(res, PolicyResult::Allowed);
    }

    #[test]
    fn test_sandbox_denies_full_read() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate("test-agent", "Crew", "read_file(file_path: 'main.rs')");
        assert!(matches!(res, PolicyResult::Denied(_)));
    }

    #[test]
    fn test_sandbox_allows_surgical_read() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate(
            "test-agent",
            "Crew",
            "read_file(file_path: 'main.rs', start_line: 1, end_line: 10)",
        );
        assert_eq!(res, PolicyResult::Allowed);
    }

    #[test]
    fn test_sandbox_admin_bypass() {
        let sandbox = Sandbox::new(mock_config());
        let res = sandbox.evaluate("tyr", "Captain", "sudo apt update");
        assert_eq!(res, PolicyResult::Allowed);
    }
}
