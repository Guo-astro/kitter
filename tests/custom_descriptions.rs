use std::{fs, path::Path, process::Command};

use kitter::{SkillLibrary, SkillOrigin, SkillRecord};

fn record(origin: SkillOrigin) -> SkillRecord {
    SkillRecord {
        name: "demo".into(),
        storage_name: String::new(),
        description: String::new(),
        custom_description: None,
        origin,
        update_available: false,
        group_id: None,
        last_operated_at: 0,
    }
}

fn write_skill(source: &Path, description: &str) -> String {
    fs::create_dir_all(source).unwrap();
    let contents = format!("---\nname: demo\ndescription: {description}\n---\nInstructions\n");
    fs::write(source.join("SKILL.md"), &contents).unwrap();
    contents
}

#[test]
fn old_records_default_to_no_custom_description() {
    let record: SkillRecord = serde_json::from_str(r#"{"name":"demo"}"#).unwrap();
    assert!(record.custom_description.is_none());
}

#[test]
fn description_persists_and_blank_input_restores_current_original_without_editing_files() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let data = temp.path().join("data");
    let original = write_skill(&source, "Original");
    let mut library = SkillLibrary::open_in(&data).unwrap();
    library
        .import(
            &source,
            record(SkillOrigin::Local {
                path: source.clone(),
                source_root: None,
                install_mode: kitter::LocalInstallMode::Link,
                source_hash: None,
            }),
        )
        .unwrap();
    library
        .set_description("demo", "  我的描述\n第二行  ")
        .unwrap();
    drop(library);
    let mut library = SkillLibrary::open_in(&data).unwrap();
    let skill = library.resolve_skill("demo").unwrap();
    assert_eq!(skill.description(), "我的描述\n第二行");
    assert_eq!(skill.original_description, "Original");
    assert_eq!(
        fs::read_to_string(source.join("SKILL.md")).unwrap(),
        original
    );
    write_skill(&source, "Changed original");
    assert_eq!(
        library.resolve_skill("demo").unwrap().description(),
        "我的描述\n第二行"
    );
    for blank in ["", " \n\t\u{3000}"] {
        library.set_description("demo", "Override").unwrap();
        library
            .set_description_by_storage(&skill.record.storage_name, blank)
            .unwrap();
        let skill = library.resolve_skill("demo").unwrap();
        assert_eq!(skill.description(), "Changed original");
        assert_eq!(skill.original_description, "Changed original");
        assert!(skill.record.custom_description.is_none());
    }
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(data.join("registry.json")).unwrap()).unwrap();
    assert!(
        json["skills"][&skill.record.storage_name]
            .get("custom_description")
            .is_none()
    );
}

#[test]
fn replacement_and_reimport_preserve_override_and_refresh_original() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    write_skill(&source, "Original");
    let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
    library
        .import(&source, record(SkillOrigin::Unknown))
        .unwrap();
    library.set_description("demo", "Custom").unwrap();
    let changed = write_skill(&source, "Updated");
    library
        .replace(&source, record(SkillOrigin::Unknown))
        .unwrap();
    let skill = library.resolve_skill("demo").unwrap();
    assert_eq!(skill.description(), "Custom");
    assert_eq!(skill.original_description, "Updated");
    assert_eq!(
        fs::read_to_string(skill.path.join("SKILL.md")).unwrap(),
        changed
    );
    fs::remove_dir_all(&skill.path).unwrap();
    write_skill(&source, "Reimported");
    library
        .import(&source, record(SkillOrigin::Unknown))
        .unwrap();
    let skill = library.resolve_skill("demo").unwrap();
    assert_eq!(skill.description(), "Custom");
    assert_eq!(skill.original_description, "Reimported");
}

#[test]
fn ambiguous_and_missing_selectors_do_not_change_descriptions() {
    let temp = tempfile::tempdir().unwrap();
    let mut library = SkillLibrary::open_in(temp.path().join("data")).unwrap();
    for name in ["one", "two"] {
        let source = temp.path().join(name);
        write_skill(&source, "Original");
        library
            .import(
                &source,
                record(SkillOrigin::Local {
                    path: source.clone(),
                    source_root: None,
                    install_mode: kitter::LocalInstallMode::Link,
                    source_hash: None,
                }),
            )
            .unwrap();
    }
    assert!(library.set_description("demo", "Custom").is_err());
    assert!(library.set_description("missing", "Custom").is_err());
    let skills = library.list().unwrap();
    assert!(skills.iter().all(|s| s.record.custom_description.is_none()));
    let selected = skills.iter().find(|s| s.record.name == "demo").unwrap();
    library
        .set_description(&format!("id:{}", selected.record.storage_name), "Selected")
        .unwrap();
    assert_eq!(
        library
            .list()
            .unwrap()
            .iter()
            .filter(|s| s.description() == "Selected")
            .count(),
        1
    );
}

#[test]
fn failed_save_keeps_the_previous_description() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let data = temp.path().join("data");
    write_skill(&source, "Original");
    let mut library = SkillLibrary::open_in(&data).unwrap();
    library
        .import(&source, record(SkillOrigin::Unknown))
        .unwrap();
    library.set_description("demo", "Keep this").unwrap();
    fs::remove_file(data.join("registry.json")).unwrap();
    fs::create_dir(data.join("registry.json")).unwrap();
    assert!(library.set_description("demo", "Replacement").is_err());
    assert_eq!(
        library.resolve_skill("demo").unwrap().description(),
        "Keep this"
    );
}

#[test]
fn cli_describe_list_show_and_reset() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let data = temp.path().join("data");
    let original = write_skill(&source, "Original");
    let mut library = SkillLibrary::open_in(&data).unwrap();
    library
        .import(&source, record(SkillOrigin::Unknown))
        .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .args(args)
            .output()
            .unwrap()
    };
    for invalid in [
        vec!["describe", "demo"],
        vec!["describe", "demo", "text", "--reset"],
    ] {
        assert!(!run(&invalid).status.success());
    }
    assert!(
        run(&["describe", "demo", "Custom description"])
            .status
            .success()
    );
    for args in [vec!["list"], vec!["show", "demo"]] {
        let output = run(&args);
        assert!(output.status.success());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("Custom description")
        );
    }
    let output = run(&["show", "demo", "--json"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["description"], "Custom description");
    assert_eq!(value["original_description"], "Original");
    assert_eq!(value["custom_description"], "Custom description");
    for args in [
        vec!["describe", "demo", "--reset"],
        vec!["describe", "demo", ""],
        vec!["describe", "demo", " \t\n"],
    ] {
        assert!(run(&["describe", "demo", "Custom"]).status.success());
        assert!(run(&args).status.success());
        let output = run(&["show", "demo", "--json"]);
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["description"], "Original");
        assert!(value["custom_description"].is_null());
    }
    assert_eq!(
        fs::read_to_string(source.join("SKILL.md")).unwrap(),
        original
    );
    for (language, help, saved) in [
        (
            kitter::config::Language::En,
            "Restore the original description",
            "Description saved",
        ),
        (kitter::config::Language::ZhCn, "恢复原描述", "描述已保存"),
    ] {
        let mut config = kitter::AppConfig::load_from(&data).unwrap();
        config.language = language;
        config.save_to(&data).unwrap();
        let output = run(&["describe", "--help"]);
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains(help));
        let output = run(&["describe", "demo", "--reset"]);
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains(saved));
    }
}
