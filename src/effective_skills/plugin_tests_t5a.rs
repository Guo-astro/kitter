use super::*;
use tempfile::tempdir;

fn write(path: &Path, content: impl AsRef<str>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content.as_ref()).unwrap();
}

fn skill(root: &Path, name: &str, description: &str, extra: &str) {
    write(
        &root.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n{extra}---\nInstructions.\n"),
    );
}

fn inspect(agent: &str, context: &DiscoveryContext) -> AgentContextEstimate {
    adapters::inspect_agent(&AgentId::from(agent), context).unwrap()
}

#[test]
fn amp_config_formats_and_precedence() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    let context = DiscoveryContext {
        home: home.clone(),
        cwd: repo.clone(),
        repository_root: Some(repo.clone()),
    };
    skill(
        &home.join(".config/agents/skills"),
        "same",
        "User first",
        "",
    );
    skill(&repo.join(".agents/skills"), "same", "Project second", "");
    for (file, config, relative) in [
        (
            ".config/amp/settings.json",
            "// JSONC\n{\"skills\":{\"path\":\"one\"}}",
            "one",
        ),
        (
            ".config/amp/config.json",
            "{\"skills\":{\"paths\":[\"two\", 2]}}",
            "two",
        ),
        (".amp/settings.json", "{\"skillPath\":\"three\"}", "three"),
        (
            ".amp/config.json",
            "{\"skills\":{\"path\":null,\"paths\":[\"ignored\"]}}",
            "ignored",
        ),
    ] {
        let file = if file.starts_with(".config") {
            home.join(file)
        } else {
            repo.join(file)
        };
        skill(
            &file.parent().unwrap().join(relative),
            relative,
            "Configured",
            "",
        );
        write(&file, config);
    }
    let estimate = inspect("amp", &context);
    assert_eq!(estimate.discovered_count, 4);
    assert_eq!(estimate.skills[0].description, "User first");
    assert!(estimate.skills.iter().any(|s| s.name == "three"));
    assert!(!estimate.skills.iter().any(|s| s.name == "ignored"));
    let global = inspect(
        "amp",
        &DiscoveryContext {
            cwd: home.clone(),
            home,
            repository_root: None,
        },
    );
    assert_eq!(global.discovered_count, 3);
}

#[test]
fn migrated_agents_keep_validation_and_visibility_rules() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");
    for root in [
        ".agents/skills",
        ".hermes/skills",
        ".openclaw/skills",
        ".gemini/config/skills",
        ".trae-cn/skills",
    ] {
        skill(
            &home.join(root),
            "manual",
            "Manual",
            "disable-model-invocation: true\n",
        );
        skill(&home.join(root), "Invalid_Name", "Invalid name", "");
    }
    let context = DiscoveryContext {
        home,
        cwd: repo,
        repository_root: None,
    };
    for agent in [
        "cursor",
        "copilot",
        "antigravity",
        "droid",
        "amp",
        "hermes",
        "openclaw",
        "trae",
    ] {
        let estimate = inspect(agent, &context);
        let unvalidated = matches!(agent, "hermes" | "openclaw");
        assert_eq!(
            estimate.skills.iter().any(|s| s.name == "Invalid_Name"),
            unvalidated,
            "{agent}"
        );
        let automatic = matches!(agent, "antigravity" | "amp" | "hermes");
        assert_eq!(
            estimate.manual_only_count,
            usize::from(!automatic),
            "{agent}"
        );
    }
}

#[test]
fn hermes_external_dirs_and_unicode_catalog() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let hermes = home.join(".hermes");
    let description = "中文🦀".repeat(30);
    skill(&hermes.join("skills"), "native", &description, "");
    skill(&home.join("shared"), "external", "External", "");
    write(
        &hermes.join("config.yaml"),
        "skills:\n  external_dirs: [~/shared, 4]\n",
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    let estimate = inspect("hermes", &context);
    assert_eq!(estimate.discovered_count, 2);
    let text = adapters::render_catalog(&AgentId::from("hermes"), &estimate.skills);
    assert_eq!(
        text,
        format!(
            "<available_skills>\n- native: {}…\n- external: External\n</available_skills>",
            description.chars().take(56).collect::<String>()
        )
    );
    assert_eq!(estimate.estimated_tokens, text.len().div_ceil(4));
}

#[test]
fn openclaw_plugin_load_paths_and_switches() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    let state = home.join(".openclaw");
    let plugin = state.join("custom/demo");
    skill(&plugin.join("skills"), "plugin-skill", "Plugin", "");
    write(&plugin.join("entry.js"), "");
    write(
        &plugin.join("openclaw.plugin.json"),
        r#"{"id":"demo","name":"Demo","skills":["skills","../escape","/absolute"]}"#,
    );
    skill(&home.join("extra"), "extra", "Extra", "");
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    for (switches, expected) in [
        (r#""allow":["demo"]"#, true),
        (r#""allow":[]"#, false),
        (r#""deny":["demo"]"#, false),
        (r#""entries":{"demo":{"enabled":false}}"#, false),
        (r#""enabled":false"#, false),
    ] {
        write(
            &state.join("openclaw.json"),
            format!(
                r#"{{// JSONC
          "plugins":{{"load":{{"paths":["custom/demo/entry.js","custom/demo"]}},{switches}}},
          "skills":{{"load":{{"extraDirs":["~/extra",2]}}}}
        }}"#
            ),
        );
        let estimate = inspect("openclaw", &context);
        assert_eq!(estimate.discovered_count, 1 + usize::from(expected));
        let plugins = estimate.plugin_skills().collect::<Vec<_>>();
        assert_eq!(plugins.len(), usize::from(expected));
        if expected {
            assert_eq!(plugins[0].source.plugin_display_name(), Some("Demo"));
        }
    }
}

#[test]
fn openclaw_catalog_truncates_unicode_by_characters() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    skill(
        &home.join(".openclaw/skills"),
        "long",
        &"中文🦀<&".repeat(8_000),
        "",
    );
    let context = DiscoveryContext {
        cwd: home.clone(),
        home,
        repository_root: None,
    };
    let estimate = inspect("openclaw", &context);
    let text = adapters::render_catalog(&AgentId::from("openclaw"), &estimate.skills);
    assert_eq!(text.chars().count(), 30_000);
    assert!(text.ends_with('…'));
    assert!(text.contains("&lt;&amp;"));
    assert_eq!(estimate.model_visible_count, 1);
    assert_eq!(estimate.estimated_tokens, text.len().div_ceil(4));
}
