use std::{fs, process::Command};

#[test]
fn cli_readds_missing_files_and_removes_stale_records() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source/demo");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\n").unwrap();
    let data = temp.path().join("data");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let source = source.to_str().unwrap();
    run(&["add", "local", source]);
    let mut library = kitter::SkillLibrary::open_in(data.clone()).unwrap();
    let storage = library.record("demo").unwrap().storage_name;
    // Rename preserves the platform link semantics without following its target.
    let destination = library.config.library_dir.join(&storage);
    fs::rename(&destination, temp.path().join("first-link")).unwrap();
    run(&["add", "local", source]);
    assert!(destination.join("SKILL.md").is_file());
    fs::rename(&destination, temp.path().join("second-link")).unwrap();
    run(&["remove", "demo", "demo"]);
    library = kitter::SkillLibrary::open_in(data.clone()).unwrap();
    assert!(library.record_by_storage(&storage).is_err());
    assert!(std::path::Path::new(source).join("SKILL.md").is_file());
    run(&["add", "local", source]);
    assert!(destination.join("SKILL.md").is_file());
}

#[test]
fn cli_confirms_batch_replacement_and_repeated_install_is_noop() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let sources = temp.path().join("sources");
    let project = temp.path().join("project");
    let destination = project.join(".agents/skills/demo");
    fs::create_dir_all(&destination).unwrap();
    fs::write(destination.join("keep"), "old files").unwrap();
    for name in ["demo", "other"] {
        fs::create_dir_all(sources.join(name)).unwrap();
        fs::write(
            sources.join(name).join("SKILL.md"),
            format!("---\nname: {name}\n---\n"),
        )
        .unwrap();
    }
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &data)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        run(&["add", "local", sources.to_str().unwrap()])
            .status
            .success()
    );
    let args = [
        "install",
        "demo",
        "other",
        "--project",
        project.to_str().unwrap(),
        "--target",
        "universal",
    ];
    let rejected = run(&args);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--yes"));
    assert!(destination.join("keep").is_file());
    assert!(!project.join(".agents/skills/other").exists());
    assert!(
        run(&[args.as_slice(), &["--yes"]].concat())
            .status
            .success()
    );
    assert!(!destination.join("keep").exists());
    assert_eq!(
        destination.canonicalize().unwrap(),
        sources.join("demo").canonicalize().unwrap()
    );
    assert!(sources.join("demo/SKILL.md").is_file());
    assert!(run(&args).status.success());

    assert!(run(&["preset", "create", "Example"]).status.success());
    assert!(run(&["preset", "add", "Example", "demo"]).status.success());
    fs::rename(&destination, temp.path().join("saved-link")).unwrap();
    fs::create_dir_all(&destination).unwrap();
    fs::write(destination.join("keep"), "old files").unwrap();
    let preset_args = [
        "preset",
        "install",
        "Example",
        "--project",
        project.to_str().unwrap(),
        "--target",
        "universal",
    ];
    assert!(!run(&preset_args).status.success());
    assert!(destination.join("keep").is_file());
    let installed = run(&[preset_args.as_slice(), &["-y"]].concat());
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(
        run(&[
            "preset",
            "uninstall",
            "Example",
            "--project",
            project.to_str().unwrap()
        ])
        .status
        .success()
    );
    assert!(!destination.exists());
    assert!(sources.join("demo/SKILL.md").is_file());
}
