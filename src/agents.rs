use crate::InstallTarget;
use std::path::{Path, PathBuf};

/// User-level roots must not be derived from project-relative directories.
pub fn global_target_root(home: &Path, target: &InstallTarget) -> Option<PathBuf> {
    target.global_dir(home)
}

/// OMP profiles use separate agent directories; the default profile honors Pi's
/// shared environment variable, but keeps its own default directory.
pub(crate) fn omp_agent_dir(home: &Path) -> PathBuf {
    let profile = std::env::var("OMP_PROFILE")
        .or_else(|_| std::env::var("PI_PROFILE"))
        .unwrap_or_default();
    let profile = profile.trim();
    if !profile.is_empty()
        && profile != "default"
        && profile.len() <= 64
        && profile
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && profile.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_' | b'.')
        })
    {
        return home.join(".omp/profiles").join(profile).join("agent");
    }
    std::env::var_os("PI_CODING_AGENT_DIR")
        .map(|value| {
            let value = value.to_string_lossy();
            value.strip_prefix("~/").map_or_else(
                || PathBuf::from(value.as_ref()),
                |relative| home.join(relative),
            )
        })
        .unwrap_or_else(|| home.join(".omp/agent"))
}

pub fn installation_root(project: &Path, target: &InstallTarget) -> Option<PathBuf> {
    if dirs::home_dir().as_deref() == Some(project) {
        global_target_root(project, target)
    } else {
        target.project_dir().map(|dir| project.join(dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_plugins::AgentRegistry;

    #[test]
    fn agent_icon_order_matches_usage_priority() {
        let ids = AgentRegistry::global()
            .agents()
            .iter()
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "codex",
                "claude-code",
                "cursor",
                "opencode",
                "pi",
                "omp",
                "dimagent",
                "grok",
                "openclaw",
                "hermes",
                "droid",
                "amp",
                "antigravity",
                "copilot",
                "trae",
            ]
        );
    }

    #[test]
    fn openclaw_and_hermes_are_global_only() {
        let global_only = AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| agent.global_only)
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(global_only, vec!["openclaw", "hermes"]);
        assert!(
            AgentRegistry::global()
                .agents()
                .iter()
                .filter(|agent| agent.global_only)
                .all(|agent| agent.reads.is_empty())
        );
    }

    #[test]
    fn shared_agents_also_have_their_provider_specific_targets() {
        for id in [
            "codex",
            "cursor",
            "opencode",
            "pi",
            "grok",
            "droid",
            "antigravity",
            "copilot",
        ] {
            let agent = AgentRegistry::global()
                .agents()
                .iter()
                .find(|agent| agent.id.as_str() == id)
                .unwrap();
            assert!(
                agent.supports_target(&InstallTarget::universal()),
                "{id} should consume .agents/skills"
            );
            assert!(
                agent.reads.iter().any(|target| !target.is_universal()),
                "{id} should have a provider-specific project target"
            );
        }
    }

    #[test]
    fn compatibility_badges_cover_shared_provider_specific_roots() {
        let cursor = AgentRegistry::global()
            .agents()
            .iter()
            .find(|agent| agent.id.as_str() == "cursor")
            .unwrap();
        assert!(cursor.supports_target(&InstallTarget::universal()));
        assert!(cursor.supports_target(&InstallTarget::from("cursor")));
        assert!(cursor.supports_target(&InstallTarget::from("claude-code")));
        assert!(cursor.supports_target(&InstallTarget::from("codex")));

        let openclaw = AgentRegistry::global()
            .agents()
            .iter()
            .find(|agent| agent.id.as_str() == "openclaw")
            .unwrap();
        assert!(!openclaw.supports_target(&InstallTarget::universal()));
    }
    #[test]
    fn install_roots_preserve_existing_directories() {
        if std::env::var_os("KITTER_TEST_ROOTS").is_none() {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "agents::tests::install_roots_preserve_existing_directories",
                ])
                .env("KITTER_TEST_ROOTS", "1")
                .env_remove("CODEX_HOME")
                .env_remove("PI_CODING_AGENT_DIR")
                .env_remove("XDG_CONFIG_HOME")
                .env_remove("OMP_PROFILE")
                .env_remove("PI_PROFILE")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let home = temp.path().join("home");
        let expected = [
            ("universal", ".agents/skills", ".agents/skills"),
            ("codex", ".codex/skills", ".codex/skills"),
            ("claude-code", ".claude/skills", ".claude/skills"),
            ("cursor", ".cursor/skills", ".cursor/skills"),
            ("opencode", ".opencode/skills", ".config/opencode/skills"),
            ("pi", ".pi/skills", ".pi/agent/skills"),
            ("omp", ".omp/skills", ".omp/agent/skills"),
            ("grok", ".grok/skills", ".grok/skills"),
            ("droid", ".factory/skills", ".factory/skills"),
            ("antigravity", ".agent/skills", ".gemini/config/skills"),
            ("copilot", ".github/skills", ".copilot/skills"),
        ];
        let targets = AgentRegistry::global().install_targets();
        assert_eq!(targets.len(), expected.len());
        for (target, (id, local, global)) in targets.iter().zip(expected) {
            assert_eq!(target.as_str(), id);
            assert_eq!(
                installation_root(&project, target),
                Some(project.join(local))
            );
            assert_eq!(global_target_root(&home, target), Some(home.join(global)));
        }
        let unknown = InstallTarget::from("missing-plugin");
        assert_eq!(installation_root(&project, &unknown), None);
        assert_eq!(global_target_root(&home, &unknown), None);
    }
}
