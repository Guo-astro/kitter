use super::*;
use serde_json::json;
use tempfile::tempdir;

fn write(path: &Path, content: impl AsRef<str>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content.as_ref()).unwrap();
}

fn skill(root: &Path, directory: &str, name: &str, extra: &str) {
    write(
        &root.join(directory).join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: Use {name}\n{extra}---\nInstructions for {name}.\n"
        ),
    );
}

#[cfg(test)]
pub(super) fn fixture(home: &Path, repo: &Path, project: &Path) {
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(project).unwrap();
    for base in [home, repo, project] {
        skill(&base.join(".agents/skills"), "shared", "shared", "");
        skill(
            &base.join(".agents/skills"),
            "manual",
            "manual",
            "disable-model-invocation: true\n",
        );
    }
    for root in [
        ".claude/skills",
        ".cursor/skills",
        ".copilot/skills",
        ".gemini/config/skills",
        ".factory/skills",
        ".pi/agent/skills",
        ".omp/agent/skills",
        ".grok/skills",
        ".openclaw/skills",
        ".hermes/skills",
        ".dimcode/v2/skills",
        ".trae-cn/skills",
    ] {
        skill(&home.join(root), "native", "native", "");
        skill(&home.join(root), "second", "second", "");
    }
    skill(
        &home.join(".codex/skills"),
        "codex-native",
        "codex-native",
        "",
    );
    skill(&home.join(".codex/skills"), "codex-off", "codex-off", "");
    for root in [".agents/skills", ".claude/skills", ".hermes/skills"] {
        for (name, description) in [
            ("unicode-mixed", "中文 English 🦀 ".repeat(23)),
            ("unicode-long", "中文描述🦀".repeat(240)),
        ] {
            write(
                &home.join(root).join(name).join("SKILL.md"),
                format!("---\nname: {name}\ndescription: {description}\n---\nInstructions.\n"),
            );
        }
    }

    write(
        &home.join(".codex/config.toml"),
        "[[skills.config]]\npath = '~/.codex/skills/codex-off/SKILL.md'\nenabled = false\n[plugins.\"demo@market\"]\nenabled = true\n",
    );
    let codex_plugin = home.join(".codex/plugins/cache/market/demo/1.0.0");
    write(
        &codex_plugin.join(".codex-plugin/plugin.json"),
        r#"{"name":"demo","interface":{"displayName":"Demo"},"paths":{"skills":["contributions/release"]}}"#,
    );
    skill(
        &codex_plugin.join("contributions"),
        "release",
        "release",
        "",
    );

    skill(
        &home.join(".claude/skills"),
        "claude-configured",
        "claude-configured",
        "",
    );
    write(
        &home.join(".claude/settings.json"),
        r#"{"skillOverrides":{"native":"name-only","second":"off"},"enabledPlugins":{"demo@market":true}}"#,
    );
    write(
        &home.join(".claude/commands/review.md"),
        "Review the changes.\n",
    );
    let claude_plugin = home.join("plugins/demo");
    write(
        &home.join(".claude/plugins/installed_plugins.json"),
        json!({"plugins":{"demo@market":[{"scope":"user","installPath":claude_plugin}]}})
            .to_string(),
    );
    write(
        &claude_plugin.join(".claude-plugin/plugin.json"),
        r#"{"name":"Demo","skills":"contributions/release"}"#,
    );
    skill(
        &claude_plugin.join("contributions"),
        "release",
        "release",
        "when_to_use: When shipping\n",
    );

    for base in [home.join(".config/opencode"), repo.to_path_buf()] {
        skill(
            &base.join("team-skills"),
            "release-path",
            "Release Guide",
            "",
        );
        skill(&base.join("team-skills"), "denied", "denied", "");
        write(
            &base.join("opencode.jsonc"),
            format!(
                "// Team skills\n{}",
                json!({
                    "skills": [base.join("team-skills")],
                    "permissions": [{"action":"skill","resource":"denied","effect":"deny"}]
                })
            ),
        );
    }
    skill(&home.join(".grok/skills"), "ignored", "ignored", "");
    skill(&home.join(".grok/skills"), "disabled", "disabled", "");
    for directory in ["original", "copy"] {
        skill(&home.join(".grok/skills"), directory, "collision", "");
    }
    write(
        &home.join(".grok/config.toml"),
        "[skills]\nignore = [\"skills/ignored\"]\ndisabled = [\"disabled\"]\n",
    );
    skill(&home.join(".omp/agent/skills"), "omp-off", "omp-off", "");
    write(
        &home.join(".omp/agent/settings.json"),
        r#"{"skills":{"enableClaudeUser":true,"enableCodexProject":false,"ignoredSkills":["omp-off"]}}"#,
    );
    // This root must stay excluded by the OMP project toggle.
    skill(
        &repo.join(".codex/skills"),
        "omp-excluded",
        "omp-excluded",
        "",
    );
    skill(&home.join("pi-extra"), "configured", "pi-configured", "");
    write(
        &home.join(".pi/agent/settings.json"),
        r#"{"skills":["~/pi-extra"]}"#,
    );
    write(
        &home.join(".pi/agent/trust.json"),
        json!({fs::canonicalize(repo).unwrap().to_string_lossy().as_ref(): false}).to_string(),
    );
    skill(&project.join(".pi/skills"), "local-pi", "local-pi", "");
    skill(&home.join("amp-extra"), "configured", "amp-configured", "");
    write(
        &home.join(".config/amp/settings.json"),
        r#"{"skills":{"path":"~/amp-extra"}}"#,
    );
    skill(&home.join(".openclaw/skills"), "claw-off", "claw-off", "");
    write(
        &home.join(".openclaw/openclaw.json"),
        r#"{"skills":{"entries":{"claw-off":{"enabled":false}}}}"#,
    );
    skill(
        &home.join(".hermes/shared"),
        "external",
        "hermes-external",
        "",
    );
    write(
        &home.join(".hermes/config.yaml"),
        "skills:\n  external_dirs: [shared]\n",
    );
    skill(&home.join(".dimcode/v2/skills"), "dim-off", "dim-off", "");
    write(
        &home.join(".dimcode/v2/skills.json"),
        r#"{"disabledSkillIds":["user:dim-off"]}"#,
    );
}

fn visible(skills: &[EffectiveSkill]) -> Vec<EffectiveSkill> {
    skills
        .iter()
        .filter(|skill| {
            matches!(
                skill.visibility,
                SkillVisibility::Automatic | SkillVisibility::NameOnly
            )
        })
        .cloned()
        .collect()
}

fn snapshot(project: &Path, home: &Path, repo: &Path) -> serde_json::Value {
    let normalize = |value: &str| {
        value
            .replace('\\', "/")
            .replace(&home.to_string_lossy().replace('\\', "/"), "$HOME")
            .replace(&repo.to_string_lossy().replace('\\', "/"), "$REPO")
    };
    let mut estimates = estimate_project_with_home(project, home);
    let mut snapshots = Vec::new();
    for estimate in &mut estimates {
        let original_catalog =
            adapters::render_catalog(&estimate.agent, &visible(&estimate.skills));
        estimate.skills.retain(|skill| {
            let inside = skill.path.starts_with(home)
                || skill.path.starts_with(repo)
                || matches!(skill.source, SkillSource::Builtin);
            if !inside {
                estimate.discovered_count -= 1;
                match skill.visibility {
                    SkillVisibility::Automatic => estimate.model_visible_count -= 1,
                    SkillVisibility::NameOnly => {
                        estimate.model_visible_count -= 1;
                        estimate.name_only_count -= 1;
                    }
                    SkillVisibility::ManualOnly => estimate.manual_only_count -= 1,
                    SkillVisibility::Conditional => estimate.conditional_count -= 1,
                }
            }
            inside
        });
        assert!(
            estimate.skills.len() >= 2,
            "{} needs two skills",
            crate::agent_plugins::AgentRegistry::global()
                .agent(estimate.agent.as_str())
                .unwrap()
                .name
                .as_str()
        );
        let normalized_catalog = normalize(&adapters::render_catalog(
            &estimate.agent,
            &visible(&estimate.skills),
        ));
        for skill in &mut estimate.skills {
            skill.path = normalize(&skill.path.to_string_lossy()).into();
            skill.root_path = skill
                .root_path
                .as_ref()
                .map(|path| normalize(&path.to_string_lossy()).into());
            skill.prompt_path = skill.prompt_path.as_ref().map(|path| normalize(path));
        }
        // Remove only the token cost of host paths and external skills. Keep
        // the production estimate so changes to its calculation remain visible.
        estimate.estimated_tokens =
            (estimate.estimated_tokens as isize + approx_token_count(&normalized_catalog) as isize
                - approx_token_count(&original_catalog) as isize) as usize;
        let mut snapshot = serde_json::to_value(estimate).unwrap();
        snapshot["catalog"] = normalized_catalog.into();
        snapshots.push(snapshot);
    }
    serde_json::to_value(snapshots).unwrap()
}

#[test]
fn all_agents_golden() {
    // env::var/var_os in effective_skills and the OMP helper in agents.rs,
    // including names passed through env_truthy/compatible_filesystem_roots.
    for name in [
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "GROK_HOME",
        "PI_CODING_AGENT_DIR",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_CONFIG",
        "OPENCODE_PROJECT_CONFIG",
        "OPENCODE_DISABLE_EXTERNAL_SKILLS",
        "OPENCODE_DISABLE_CLAUDE_CODE",
        "OPENCODE_DISABLE_CLAUDE_CODE_SKILLS",
        "XDG_CONFIG_HOME",
        "COPILOT_SKILLS_DIRS",
        "AMP_SKILL_PATHS",
        "OPENCLAW_STATE_DIR",
        "CLAWDBOT_STATE_DIR",
        "OPENCLAW_CONFIG_PATH",
        "HERMES_HOME",
        "DIMCODE_HOME",
        "OMP_PROFILE",
        "PI_PROFILE",
    ] {
        if env::var_os(name).is_some() {
            eprintln!("Skipping effective_skills golden: {name} is set");
            return;
        }
    }
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let project = repo.join("subdir");
    fixture(&home, &repo, &project);
    let result = json!({
        "project": snapshot(&project, &home, &repo),
        "global": snapshot(&home, &home, &repo),
    });
    assert_eq!(result["global"].as_array().unwrap().len(), 15);
    assert_eq!(result["project"].as_array().unwrap().len(), 15);
    let actual = format!("{}\n", serde_json::to_string_pretty(&result).unwrap());
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/effective_skills/testdata/golden.json");
    if env::var("KITTER_UPDATE_GOLDEN").as_deref() == Ok("1") {
        fs::write(path, actual).unwrap();
    } else {
        // Desktop dependencies enable serde_json's preserve_order feature.
        // Compare JSON values so object key order does not change acceptance.
        let expected: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(result, expected);
    }
}
