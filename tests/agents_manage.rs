use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn cli(home: &Path, args: &[&str], locale: &str) -> Output {
    fs::create_dir_all(home).unwrap();
    fs::write(
        home.join("config.json"),
        serde_json::to_vec(&serde_json::json!({
            "language": if locale.starts_with("zh") { "zh_cn" } else { "en" },
            "library_dir": home.join("skills"),
        }))
        .unwrap(),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_kitter"))
        .env("KITTER_HOME", home)
        .env("LANG", locale)
        .env("LC_ALL", locale)
        .args(args)
        .output()
        .unwrap()
}
fn success(home: &Path, args: &[&str]) -> String {
    let output = cli(home, args, "en_US.UTF-8");
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn list(home: &Path) -> Value {
    serde_json::from_str(&success(home, &["agents", "--json"])).unwrap()
}

#[test]
fn local_add_disable_move_update_remove_and_list() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let source = temporary.path().join("source");
    fs::create_dir_all(&source).unwrap();
    let file = source.join("arbitrary.js");
    fs::write(&file, "export default {apiVersion:1,id:'maki',name:'Maki',install:{project:'.maki/skills',global:'~/.maki/skills'}}").unwrap();
    assert!(
        success(&home, &["agents", "add", source.to_str().unwrap()]).contains("Added Agent: maki")
    );
    let listing = list(&home);
    let maki = listing["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["id"] == "maki")
        .unwrap();
    assert_eq!(maki["enabled"], true);
    assert_eq!(maki["source"]["kind"], "local");
    assert_eq!(
        maki["source"]["path"],
        file.canonicalize().unwrap().to_str().unwrap()
    );
    let codex = listing["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|agent| agent["id"] == "codex")
        .unwrap();
    assert!(codex.get("source").is_none());
    success(&home, &["agents", "disable", "maki"]);
    assert!(
        success(&home, &["agents"])
            .lines()
            .any(|line| line.contains("maki") && line.ends_with("disabled"))
    );
    let zh = cli(&home, &["agents"], "zh_CN.UTF-8");
    assert!(String::from_utf8_lossy(&zh.stdout).contains("已停用"));
    let listing = list(&home);
    assert_eq!(
        listing["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["id"] == "maki")
            .unwrap()["enabled"],
        false
    );
    let skill = temporary.path().join("test");
    fs::create_dir(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: test\ndescription: Test skill.\n---\nTest.\n",
    )
    .unwrap();
    success(&home, &["add", "local", skill.to_str().unwrap()]);
    let install = cli(
        &home,
        &[
            "install",
            "test",
            "--project",
            temporary.path().to_str().unwrap(),
            "--target",
            "maki",
        ],
        "en_US.UTF-8",
    );
    assert!(!install.status.success());
    assert!(
        String::from_utf8_lossy(&install.stderr).contains("Unknown target: maki"),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    success(&home, &["agents", "move", "maki", "--before", "codex"]);
    assert_eq!(list(&home)["agents"][0]["id"], "maki");
    success(&home, &["agents", "move", "maki", "--after", "codex"]);
    assert_eq!(list(&home)["agents"][1]["id"], "maki");
    success(&home, &["agents", "enable", "maki"]);
    assert!(success(&home, &["agents", "update"]).contains("All Agents are up to date."));
    fs::write(
        &file,
        "export default {apiVersion:1,id:'maki',name:'New Maki'}",
    )
    .unwrap();
    assert!(success(&home, &["agents", "update"]).contains("Updated Agent: maki"));
    success(&home, &["agents", "remove", "maki"]);
    assert!(
        !list(&home)["agents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["id"] == "maki")
    );
}

#[test]
fn add_only_rejects_unavailable_ids_and_skips_invalid_candidates() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().join("home");
    let source = temporary.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("first.js"),
        "export default {apiVersion:1,id:'first',name:'First'}",
    )
    .unwrap();
    fs::write(
        source.join("second.js"),
        "export default {apiVersion:1,id:'second',name:'Second'}",
    )
    .unwrap();
    fs::write(source.join("invalid.js"), "invalid").unwrap();
    let path = source.to_str().unwrap();
    let output = cli(
        &home,
        &["agents", "add", path, "--only", "missing"],
        "en_US.UTF-8",
    );
    assert!(!output.status.success());
    assert!(!home.join("agents/first.js").exists());
    let output = cli(
        &home,
        &["agents", "add", path, "--only", "first"],
        "en_US.UTF-8",
    );
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Skipped invalid:"));
    assert!(home.join("agents/first.js").exists());
    assert!(!home.join("agents/second.js").exists());
    success(&home, &["agents", "add", path]);
    let output = cli(&home, &["agents", "add", path], "en_US.UTF-8");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Already added"));
}

#[test]
fn move_requires_exactly_one_direction_and_broken_settings_stay_intact() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path();
    for args in [
        vec!["agents", "move", "codex"],
        vec![
            "agents",
            "move",
            "codex",
            "--before",
            "claude-code",
            "--after",
            "cursor",
        ],
    ] {
        assert!(!cli(home, &args, "en_US.UTF-8").status.success());
    }
    fs::write(home.join("agents.json"), "{broken").unwrap();
    let listing = list(home);
    assert_eq!(listing["diagnostics"].as_array().unwrap().len(), 1);
    assert!(listing["diagnostics"][0]["id"].is_null());
    assert!(!listing["agents"].as_array().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(home.join("agents.json")).unwrap(),
        "{broken"
    );
}
