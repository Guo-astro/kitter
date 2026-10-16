use super::*;
use tempfile::tempdir;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn t5b_claude_commands_and_plugin_fallback() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    write(&home.join(".claude/commands/check.md"), "Check the code.\n");
    let plugin = home.join(".claude/plugins/cache/market/demo/1");
    write(&plugin.join("skills/empty/SKILL.md"), "Body only.\n");
    write(
        &plugin.join("skills/trigger/SKILL.md"),
        "---\nname: ignored\nwhen_to_use: When checking\n---\nCheck body.\n",
    );
    write(
        &home.join(".claude/settings.json"),
        r#"{"enabledPlugins":{"demo@market":true}}"#,
    );
    write(
        &home.join(".claude/plugins/installed_plugins.json"),
        &serde_json::json!({"plugins":{"demo@market":[{"scope":"user","installPath":plugin}]}})
            .to_string(),
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    write(
        &context.home.join(".claude/skills/cache-link/SKILL.md"),
        "---\nname: custom\ndescription: Check\n---\nBody\n",
    );
    let estimate = adapters::inspect_agent(&AgentId::from("claude-code"), &context).unwrap();
    let command = estimate.skills.iter().find(|s| s.name == "check").unwrap();
    assert_eq!(command.description, "Check the code.");
    assert!(estimate.skills.iter().any(|s| s.name == "cache-link"));
    assert!(!estimate.skills.iter().any(|s| s.name == "demo:empty"));
    let trigger = estimate
        .skills
        .iter()
        .find(|s| s.name == "demo:trigger")
        .unwrap();
    assert_eq!(trigger.description, "Check body.");
    assert_eq!(trigger.when_to_use.as_deref(), Some("When checking"));
}

#[test]
fn t5b_codex_directory_disable_and_yaml_policy() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    for name in ["disabled", "manual", "automatic"] {
        write(
            &home.join(format!(".codex/skills/{name}/SKILL.md")),
            &format!(
                "---\nname: {name}\ndescription: 中文 🦀\ndisable-model-invocation: true\n---\nBody\n"
            ),
        );
    }
    write(
        &home.join(".codex/config.toml"),
        "[[skills.config]]\npath = '~/.codex/skills/disabled'\nenabled = false\n",
    );
    write(
        &home.join(".codex/skills/manual/agents/openai.yaml"),
        "policy:\n  allow_implicit_invocation: false\n",
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    let estimate = adapters::inspect_agent(&AgentId::from("codex"), &context).unwrap();
    assert!(!estimate.skills.iter().any(|s| s.name == "disabled"));
    assert_eq!(
        estimate
            .skills
            .iter()
            .find(|s| s.name == "manual")
            .unwrap()
            .visibility,
        SkillVisibility::ManualOnly
    );
    assert_eq!(
        estimate
            .skills
            .iter()
            .find(|s| s.name == "automatic")
            .unwrap()
            .visibility,
        SkillVisibility::Automatic
    );
}

#[test]
fn t5b_opencode_flat_autoinvoke_and_agent_permissions() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let root = home.join(".config/opencode");
    write(
        &root.join("skills/root.md"),
        "---\nname: Display\ndescription: 中文 🦀\nmetadata:\n  opencode/autoinvoke: 'false'\n---\nBody\n",
    );
    write(
        &root.join("skills/denied/SKILL.md"),
        "---\nname: denied\ndescription: Denied\n---\nBody\n",
    );
    write(
        &root.join("opencode.jsonc"),
        r#"{ "default_agent": "review", "agents": { "review": { "permission": { "skill": { "denied": "deny" } } } } }"#,
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    let estimate = adapters::inspect_agent(&AgentId::from("opencode"), &context).unwrap();
    let skill = estimate.skills.iter().find(|s| s.id == "root").unwrap();
    assert_eq!(skill.name, "Display");
    assert_eq!(skill.visibility, SkillVisibility::ManualOnly);
    assert!(!estimate.skills.iter().any(|s| s.id == "denied"));
}

#[cfg(test)]
fn catalog_skill(name: &str) -> EffectiveSkill {
    EffectiveSkill {
        id: name.into(),
        name: name.into(),
        description: "中文🦀".repeat(80),
        when_to_use: None,
        path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
        root_path: Some("/skills".into()),
        prompt_path: Some(format!("/skills/{name}/SKILL.md")),
        scope: SkillScope::User,
        visibility: SkillVisibility::Automatic,
        source: SkillSource::Filesystem,
    }
}

#[test]
fn t5b_claude_budget_falls_back_to_names_only() {
    let names = ["a".repeat(3980), "b".repeat(3980)];
    let skills = names
        .iter()
        .map(|name| catalog_skill(name))
        .collect::<Vec<_>>();
    let text = adapters::render_catalog(&AgentId::from("claude-code"), &skills);
    assert_eq!(
        text,
        format!(
            "<system-reminder>\n- {}\n- {}\n</system-reminder>",
            names[0], names[1]
        )
    );
}

#[test]
fn t5b_codex_budget_counts_only_included_names() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let name = "长".repeat(3000);
    write(
        &home.join(".codex/skills/long/SKILL.md"),
        &format!("---\nname: {name}\ndescription: 中文 🦀\n---\nBody\n"),
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    let estimate = adapters::inspect_agent(&AgentId::from("codex"), &context).unwrap();
    assert_eq!(estimate.discovered_count, 1);
    assert_eq!(estimate.model_visible_count, 0);
    assert!(!adapters::render_catalog(&estimate.agent, &estimate.skills).contains(&name));
}

#[test]
fn t5b_codex_catalog_sorts_unicode_names_by_code_point() {
    let skills = ["🦀", "\u{e000}", "中文"].map(catalog_skill);
    let text = adapters::render_catalog(&AgentId::from("codex"), &skills);
    let positions =
        ["中文", "\u{e000}", "🦀"].map(|name| text.find(&format!("- {name}:")).unwrap());
    assert!(positions[0] < positions[1] && positions[1] < positions[2]);
}
