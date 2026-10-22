use super::*;
use serde_json::{Value, json};
use std::{borrow::Cow, fs, path::Path, time::Instant};

fn source(name: &str, code: &str) -> PluginSource {
    PluginSource {
        name: name.into(),
        code: Cow::Owned(code.into()),
    }
}
fn context(home: &Path, cwd: &Path, repo: Option<&Path>, is_global: bool) -> HostContext {
    HostContext {
        home: home.into(),
        cwd: cwd.into(),
        repo_root: repo.map(Into::into),
        is_global,
    }
}
fn host() -> JsHost {
    JsHost::new(context(
        Path::new("/home/test"),
        Path::new("/repo/sub"),
        Some(Path::new("/repo")),
        false,
    ))
}
fn call(host: &mut JsHost, source: &PluginSource, path: &str, input: Option<Value>) -> Value {
    host.call(source, &HookPath(path.into()), input).unwrap()
}

#[test]
fn parses_declarations_and_hook_paths() {
    let plugin = source(
        "valid.js",
        r#"
import { defineAgent, standard } from 'kitter';
export default defineAgent({apiVersion: 1, id: 'test-agent', name: 'Test',
  install: {project: '.test/skills', global: ctx => ctx.path('~/.test/skills')},
  skills: standard.skills({project: ['.test/skills'], user: ['.test/skills'],
    filter: (entries, ctx) => entries,
    catalog: {render: (entries, ctx) => 'text'}
  })
});"#,
    );
    let spec = load_spec(&plugin).unwrap();
    assert!(!spec.global_only);
    assert!(
        matches!(spec.install.unwrap().global, PathTemplate::Hook(HookPath(ref p)) if p == "install.global")
    );
    let skills = spec.skills.unwrap();
    assert_eq!(skills.filter, Some(HookPath("skills.filter".into())));
    assert!(matches!(skills.roots, RootsValue::Decl(ref d) if d.project == [".test/skills"]));
    assert!(
        matches!(skills.catalog, Some(CatalogSpec::Custom(ref c)) if c.render.0 == "skills.catalog.render")
    );
}

#[test]
fn grok_plugin_containment_accepts_windows_paths_and_rejects_outside_folders() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let plugin = home.join(".grok/plugins/demo");
    for dir in ["skills", "commands"] {
        fs::create_dir_all(plugin.join(dir)).unwrap();
    }
    fs::create_dir_all(plugin.parent().unwrap().join("escape")).unwrap();
    fs::write(
        plugin.join("plugin.json"),
        r#"{"name":"demo","skills":["skills","../escape"]}"#,
    )
    .unwrap();
    fs::write(
        home.join(".grok/config.toml"),
        "[plugins]\nenabled = [\"demo\"]\n",
    )
    .unwrap();
    let builtin = AgentRegistry::global().agent("grok").unwrap();
    // Exercise the actual roots hook with Windows-style canonical paths on
    // every CI host. Filesystem reads still use native fixture paths.
    let windows_code = builtin.source.code.replacen(
        "roots(ctx) {",
        r#"roots(ctx) {
      const canonical = ctx.canonical;
      ctx = { ...ctx, canonical: value => canonical(value).replace(/\//g, "\\") };
    "#,
        1,
    );
    for code in [builtin.source.code.as_ref(), windows_code.as_str()] {
        let source = source("grok-paths.js", code);
        let mut host = JsHost::new(context(&home, &home, None, true));
        let roots = call(&mut host, &source, "skills.roots", None);
        let plugin_roots = roots
            .as_array()
            .unwrap()
            .iter()
            .filter(|root| root.get("plugin").is_some())
            .collect::<Vec<_>>();
        assert_eq!(plugin_roots.len(), 2);
        assert!(plugin_roots.iter().all(|root| {
            let path = std::path::PathBuf::from(root["path"].as_str().unwrap());
            path == plugin.join("skills") || path == plugin.join("commands")
        }));
    }
}

#[test]
fn rejects_invalid_specs_and_reports_syntax_locations() {
    for body in [
        "apiVersion:1,id:'x',name:'X',typo: true",
        "apiVersion:1,id:'Bad_id',name:'X'",
        "apiVersion:1,id:'universal',name:'X'",
        "apiVersion:2,id:'x',name:'X'",
        "apiVersion:1,id:'x',name:'X',skills:{roots:{oops:[]}}",
        "apiVersion:1,id:'x',name:'X',install:{global:'~/.test'}",
        "apiVersion:1,id:'x',name:'X',install:{project:'.test'}",
        "apiVersion:1,id:'x',name:'X',install:{project:'/absolute',global:'~/.test'}",
    ] {
        assert!(
            load_spec(&source("invalid.js", &format!("export default {{{body}}}"))).is_err(),
            "{body}"
        );
    }
    for code in ["export default null", "export default 1"] {
        assert!(load_spec(&source("shape.js", code)).is_err());
    }
    let error = load_spec(&source("syntax.js", "export default {\n broken: ;\n};")).unwrap_err();
    assert!(!error.message.is_empty());
    assert!(error.stack.unwrap().contains("syntax.js:2:"));
}

#[test]
fn icons_accept_strings_and_theme_pairs_and_validate_each_svg() {
    for icon in [
        json!("builtin:cursor"),
        json!("  <svg/> \n"),
        json!({"light":"<svg fill='black'/>","dark":"<svg fill='white'/>"}),
    ] {
        let plugin = source(
            "icon.js",
            &format!("export default {{apiVersion:1,id:'x',name:'X',icon:{icon}}}"),
        );
        assert!(load_spec(&plugin).is_ok(), "{icon}");
    }
    let oversized = format!("<svg>{}</svg>", "x".repeat(64 * 1024));
    for icon in [
        json!("cursor"),
        json!(oversized),
        json!({"light":"<svg/>"}),
        json!({"light":"<svg/>","dark":"<svg/>","tint":true}),
        json!({"light":"bad","dark":"<svg/>"}),
        json!({"light":"<svg/>","dark":"bad"}),
        json!({"light":oversized,"dark":"<svg/>"}),
        json!({"light":"<svg/>","dark":oversized}),
    ] {
        assert!(
            load_spec(&source(
                "icon.js",
                &format!("export default {{apiVersion:1,id:'x',name:'X',icon:{icon}}}")
            ))
            .is_err()
        );
    }
}

#[test]
fn hooks_receive_input_and_one_shared_context_and_modules_execute_once() {
    let plugin = source(
        "hooks.js",
        r#"
const count = globalThis.executions = (globalThis.executions ?? 0) + 1;
let saved;
export default {skills: {
  filter(entries, ctx) {
    saved ??= ctx;
    return entries.map(e => ({...e, name: e.name + ctx.cwd, count, same: saved === ctx}));
  },
  render(ctx) { return {cwd: ctx.cwd, same: saved === ctx}; }
}};"#,
    );
    let mut host = host();
    for _ in 0..2 {
        let output = call(
            &mut host,
            &plugin,
            "skills.filter",
            Some(json!([{"key":0,"name":"A"}])),
        );
        assert_eq!(
            output,
            json!([{"key":0,"name":"A/repo/sub","count":1,"same":true}])
        );
    }
    assert_eq!(
        call(&mut host, &plugin, "skills.render", None),
        json!({"cwd":"/repo/sub","same":true})
    );
}

#[test]
fn ctx_reads_formats_and_returns_null_for_missing_or_invalid_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "answer = 42\n").unwrap();
    fs::write(
        dir.path().join("config.jsonc"),
        "{\n// comment\n\"answer\":42,\n}\n",
    )
    .unwrap();
    fs::write(dir.path().join("config.json"), "{\"answer\":42}").unwrap();
    fs::write(dir.path().join("config.yaml"), "answer: 42\n").unwrap();
    fs::write(dir.path().join("bad"), "[\"unterminated").unwrap();
    let plugin = source(
        "reads.js",
        r#"export default {read(p, ctx) {
      const methods = ['readJson','readJsonc','readToml','readYaml'];
      return {
        values: [ctx.readJson(p+'/config.json'), ctx.readJsonc(p+'/config.jsonc'), ctx.readToml(p+'/config.toml'), ctx.readYaml(p+'/config.yaml')],
        missing: [...methods.map(m => ctx[m](p+'/missing')), ctx.readText(p+'/missing')],
        invalid: methods.map(m => ctx[m](p+'/bad')),
        text: ctx.readText(p+'/config.yaml'), names: ctx.listDir(p), noNames: ctx.listDir(p+'/missing'),
        exists: ctx.exists(p), file: ctx.isFile(p+'/config.toml'), dir: ctx.isDir(p),
        canonical: ctx.canonical(p), missingCanonical: ctx.canonical(p+'/missing'),
        wildcard: ctx.wildcard('config.*', 'config.toml'),
        native: ctx.native.testEcho({value:42}),
      };
    }};"#,
    );
    let mut host = JsHost::new(context(dir.path(), dir.path(), None, true));
    let result = call(&mut host, &plugin, "read", Some(json!(dir.path())));
    assert_eq!(
        result["values"],
        json!([{"answer":42},{"answer":42},{"answer":42},{"answer":42}])
    );
    assert_eq!(result["missing"], json!([null, null, null, null, null]));
    assert_eq!(result["invalid"], json!([null, null, null, null]));
    assert_eq!(result["text"], "answer: 42\n");
    assert_eq!(
        result["names"],
        json!([
            "bad",
            "config.json",
            "config.jsonc",
            "config.toml",
            "config.yaml"
        ])
    );
    assert_eq!(result["noNames"], json!([]));
    for key in ["exists", "file", "dir", "wildcard"] {
        assert_eq!(result[key], true);
    }
    assert_eq!(
        result["canonical"],
        json!(fs::canonicalize(dir.path()).unwrap())
    );
    assert_eq!(
        result["missingCanonical"],
        json!(format!("{}/missing", dir.path().display()))
    );
    assert_eq!(
        result["native"],
        json!({"input":{"value":42},"cwd":dir.path()})
    );
}

#[test]
fn ctx_walk_up_paths_env_and_readonly_properties() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let repo = dir.path().join("repo");
    let cwd = repo.join("sub");
    let plugin = source(
        "paths.js",
        r#"export default {inspect(ctx) {
      let readonly = false;
      try { ctx.cwd = 'changed'; } catch { readonly = true; }
      return {walk: ctx.walkUp(), path: ctx.path('${env:KITTER_T3_UNSET_VARIABLE:-~/.test}/skills'),
        base: ctx.path('x', ctx.cwd), missing: ctx.env('KITTER_T3_UNSET_VARIABLE'),
        paths: ctx.envPaths('PATH'), platform: ctx.platform, readonly, repo:ctx.repoRoot, global:ctx.isGlobal};
    }};"#,
    );
    let ctx = context(&home, &cwd, Some(&repo), false);
    let mut host = JsHost::new(ctx.clone());
    let result = call(&mut host, &plugin, "inspect", None);
    assert_eq!(result["walk"], json!([cwd, repo]));
    assert_eq!(result["path"], json!(home.join(".test/skills")));
    assert_eq!(result["base"], json!(cwd.join("x")));
    assert_eq!(result["missing"], Value::Null);
    assert_eq!(result["paths"], json!(ctx.env_paths("PATH")));
    assert_eq!(result["readonly"], true);
    assert_eq!(result["repo"], json!(repo));
    assert_eq!(result["global"], false);
    assert!(["macos", "linux", "windows"].contains(&result["platform"].as_str().unwrap()));
    let mut host = JsHost::new(context(&home, &home, None, true));
    let result = call(&mut host, &plugin, "inspect", None);
    assert_eq!(result["walk"], json!([]));
    assert_eq!(result["repo"], Value::Null);
    assert_eq!(result["global"], true);
}

#[test]
fn standard_roots_match_the_existing_rust_roots_and_native_catalog_helpers() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let repo = dir.path().join("repo");
    let cwd = repo.join("sub");
    let decl = RootsDecl {
        project: vec![".cursor/skills".into(), ".agents/skills".into()],
        user: vec![".cursor/skills".into()],
        env_paths: Some("PATH".into()),
    };
    let plugin = source(
        "roots.js",
        r#"import {standard, catalog} from 'kitter';
      export default {roots(decl,ctx) {return standard.roots(ctx,decl);},
        catalog(ctx) {return [catalog.xmlEscape('<&"\'你>'),catalog.truncate('😀你好abc',4),catalog.byteLength('😀你好')];}};"#,
    );
    for global in [false, true] {
        let ctx = context(
            &home,
            if global { &home } else { &cwd },
            Some(&repo),
            global,
        );
        let old = crate::effective_skills::compatible_roots_for_test(
            &ctx.cwd,
            &ctx.home,
            ctx.repo_root.as_deref(),
            &[".cursor/skills", ".agents/skills"],
            &[".cursor/skills"],
            Some("PATH"),
        );
        let roots = expand_roots(&decl, &ctx);
        let comparable: Vec<_> = roots
            .iter()
            .map(|r| {
                (
                    r.path.clone(),
                    match r.scope {
                        Scope::Repository => crate::effective_skills::SkillScope::Repository,
                        Scope::User => crate::effective_skills::SkillScope::User,
                        _ => panic!("unexpected scope"),
                    },
                    r.layout == Some(Layout::RootMarkdown),
                )
            })
            .collect();
        assert_eq!(comparable, old);
        let mut host = JsHost::new(ctx);
        assert_eq!(
            call(
                &mut host,
                &plugin,
                "roots",
                Some(serde_json::to_value(&decl).unwrap())
            ),
            serde_json::to_value(roots).unwrap()
        );
        assert_eq!(
            call(&mut host, &plugin, "catalog", None),
            json!([
                crate::effective_skills::xml_escape("<&\"'你>"),
                crate::effective_skills::truncate_chars("😀你好abc", 4),
                "😀你好".len()
            ])
        );
    }
    let ctx = context(&home, &cwd, Some(&repo), false);
    let templated = RootsDecl {
        user: vec![
            "~/.test/skills".into(),
            "${env:KITTER_T3_UNSET_VARIABLE:-~/.fallback}".into(),
        ],
        ..Default::default()
    };
    let roots = expand_roots(&templated, &ctx);
    assert_eq!(roots[0].path, home.join(".test/skills"));
    assert_eq!(roots[1].path, home.join(".fallback"));
}

#[test]
fn exceptions_and_infinite_loops_are_plugin_errors_and_host_recovers() {
    let plugin = source(
        "throw.js",
        "export default {\n boom(ctx) { throw new Error('hook failed'); },\n loop(ctx) { while(true) {} },\n ok(ctx) { return 42; }\n};",
    );
    let mut host = host();
    let error = host
        .call(&plugin, &HookPath("boom".into()), None)
        .unwrap_err();
    assert_eq!(error.message, "hook failed");
    assert!(error.stack.unwrap().contains("throw.js:2:"));
    let start = Instant::now();
    let error = host
        .call(&plugin, &HookPath("loop".into()), None)
        .unwrap_err();
    assert!(error.message.contains("interrupted"), "{error:?}");
    assert!(start.elapsed().as_secs_f64() < 5.0);
    assert_eq!(call(&mut host, &plugin, "ok", None), 42);
}

#[test]
fn top_level_infinite_loops_are_interrupted_while_loading() {
    let start = Instant::now();
    let error = load_spec(&source("loop.js", "while (true) {}\nexport default {};")).unwrap_err();
    assert!(error.message.contains("interrupted"), "{error:?}");
    assert!(start.elapsed().as_secs_f64() < 5.0);
}
