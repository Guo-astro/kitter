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
    let home = dirs::home_dir()?;
    if crate::effective_skills::same_directory(project, &home) {
        global_target_root(project, target)
    } else {
        target.project_root(project, &home)
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
    fn all_builtin_agents_support_project_contexts() {
        let global_only = AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| agent.global_only)
            .map(|agent| agent.id.as_str())
            .collect::<Vec<_>>();
        assert!(global_only.is_empty());
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
                .env_remove("GROK_HOME")
                .env_remove("HERMES_HOME")
                .env_remove("OPENCLAW_STATE_DIR")
                .env_remove("CLAWDBOT_STATE_DIR")
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let home = temp.path().join("home");
        std::fs::create_dir_all(project.join(".git")).unwrap();
        std::fs::create_dir_all(home.join(".hermes")).unwrap();
        std::fs::write(
            home.join(".hermes/config.yaml"),
            format!(
                "skills:\n  trusted_project_dirs: [{}]\n",
                serde_json::to_string(&project).unwrap()
            ),
        )
        .unwrap();
        let expected = [
            ("universal", ".agents/skills", ".agents/skills"),
            ("codex", ".codex/skills", ".codex/skills"),
            ("claude-code", ".claude/skills", ".claude/skills"),
            ("cursor", ".cursor/skills", ".cursor/skills"),
            ("opencode", ".opencode/skills", ".config/opencode/skills"),
            ("pi", ".pi/skills", ".pi/agent/skills"),
            ("omp", ".omp/skills", ".omp/agent/skills"),
            ("dimagent", ".agents/skills", ".agents/skills"),
            ("grok", ".grok/skills", ".grok/skills"),
            ("openclaw", "skills", ".openclaw/skills"),
            ("hermes", ".hermes/skills", ".hermes/skills"),
            ("droid", ".factory/skills", ".factory/skills"),
            ("amp", ".agents/skills", ".config/agents/skills"),
            ("antigravity", ".agent/skills", ".gemini/config/skills"),
            ("copilot", ".github/skills", ".copilot/skills"),
            ("trae", ".trae/skills", ".trae-cn/skills"),
        ];
        let targets = AgentRegistry::global().install_targets();
        assert_eq!(targets.len(), expected.len());
        for (target, (id, local, global)) in targets.iter().zip(expected) {
            assert_eq!(target.as_str(), id);
            assert_eq!(
                target.project_root(&project, &home),
                Some(project.join(local))
            );
            assert_eq!(global_target_root(&home, target), Some(home.join(global)));
        }
        let unknown = InstallTarget::from("missing-plugin");
        assert_eq!(installation_root(&project, &unknown), None);
        assert_eq!(global_target_root(&home, &unknown), None);
    }
}
