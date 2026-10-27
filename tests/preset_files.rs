use std::{
    fs,
    io::Write,
    process::{Command, Output, Stdio},
};

struct CliFixture {
    _temp: tempfile::TempDir,
    data: std::path::PathBuf,
    project: std::path::PathBuf,
}
impl CliFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("data");
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();
        Self {
            _temp: temp,
            data,
            project,
        }
    }
    fn run(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_kitter"))
            .env("KITTER_HOME", &self.data)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(bytes) = input {
            child.stdin.take().unwrap().write_all(bytes).unwrap();
        }
        drop(child.stdin.take());
        child.wait_with_output().unwrap()
    }
    fn ok(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let output = self.run(args, input);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

#[test]
fn cli_manages_preset_files_and_handles_apply_and_sync_conflicts() {
    let f = CliFixture::new();
    f.ok(&["preset", "create", "team"], None);
    let source = f._temp.path().join("source.md");
    fs::write(&source, "from source\n").unwrap();
    f.ok(
        &[
            "preset",
            "file",
            "write",
            "team",
            "docs/source.md",
            source.to_str().unwrap(),
        ],
        None,
    );
    f.ok(
        &["preset", "file", "write", "team", "docs/rules.md", "-"],
        Some(b"v1\n"),
    );
    assert_eq!(
        f.ok(&["preset", "file", "read", "team", "docs/rules.md"], None)
            .stdout,
        b"v1\n"
    );
    let files: Vec<String> = serde_json::from_slice(
        &f.ok(&["preset", "file", "list", "team", "--json"], None)
            .stdout,
    )
    .unwrap();
    assert_eq!(files, ["docs/rules.md", "docs/source.md"]);
    f.ok(
        &[
            "preset",
            "file",
            "rename",
            "team",
            "docs/source.md",
            "docs/renamed.md",
        ],
        None,
    );
    f.ok(
        &["preset", "file", "delete", "team", "docs/renamed.md"],
        None,
    );
    fs::create_dir(f.project.join("docs")).unwrap();
    fs::write(f.project.join("docs/rules.md"), "mine\n").unwrap();
    let args = [
        "preset",
        "apply",
        "team",
        "--project",
        f.project.to_str().unwrap(),
        "--target",
        "universal",
    ];
    let failed = f.run(&args, None);
    assert!(!failed.status.success());
    let error = String::from_utf8_lossy(&failed.stderr);
    assert!(error.contains("docs/rules.md"), "{error}");
    assert!(error.contains("--overwrite-conflicts"), "{error}");
    assert!(
        !f.run(
            &[
                args.as_slice(),
                &["--overwrite-conflicts", "--skip-conflicts"]
            ]
            .concat(),
            None
        )
        .status
        .success()
    );
    f.ok(
        &[args.as_slice(), &["--overwrite-conflicts"]].concat(),
        None,
    );
    assert_eq!(
        fs::read_to_string(f.project.join("docs/rules.md")).unwrap(),
        "v1\n"
    );

    f.ok(
        &["preset", "file", "write", "team", "new.md", "-"],
        Some(b"new preset"),
    );
    fs::write(f.project.join("new.md"), "keep").unwrap();
    let sync = [
        "preset",
        "sync",
        "team",
        "--project",
        f.project.to_str().unwrap(),
    ];
    let failed = f.run(&sync, None);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("new.md"));
    f.ok(&[sync.as_slice(), &["--skip-conflicts"]].concat(), None);
    f.ok(
        &["preset", "file", "write", "team", "docs/rules.md", "-"],
        Some(b"v2\n"),
    );
    fs::write(f.project.join("docs/rules.md"), "edited").unwrap();
    assert!(!f.run(&sync, None).status.success());
    f.ok(&[sync.as_slice(), &["--force"]].concat(), None);
    assert_eq!(
        fs::read_to_string(f.project.join("docs/rules.md")).unwrap(),
        "v2\n"
    );
    assert_eq!(
        fs::read_to_string(f.project.join("new.md")).unwrap(),
        "keep"
    );
    f.ok(
        &[
            "preset",
            "uninstall",
            "team",
            "--project",
            f.project.to_str().unwrap(),
        ],
        None,
    );
    assert!(!f.project.join("docs/rules.md").exists());
    assert!(f.project.join("docs").is_dir());
    assert!(f.project.join("new.md").exists());
}

#[test]
fn cli_rejects_binary_content_without_creating_a_file() {
    let f = CliFixture::new();
    f.ok(&["preset", "create", "team"], None);
    assert!(
        !f.run(
            &["preset", "file", "write", "team", "bad", "-"],
            Some(b"a\0b")
        )
        .status
        .success()
    );
    let files: Vec<String> = serde_json::from_slice(
        &f.ok(&["preset", "file", "list", "team", "--json"], None)
            .stdout,
    )
    .unwrap();
    assert!(files.is_empty());
}
