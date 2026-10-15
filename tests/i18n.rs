use proc_macro2::{TokenStream, TokenTree};
use std::{collections::BTreeMap, fs, path::Path};
use syn::parse::Parser;
use syn::visit::{self, Visit};

fn parameters(text: &str) -> std::collections::BTreeSet<&str> {
    text.split("%{")
        .skip(1)
        .map(|part| part.split_once('}').expect("unclosed parameter").0)
        .collect()
}

fn catalogue(locale: &str) -> BTreeMap<String, String> {
    fn flatten(prefix: &str, value: &serde_yaml::Value, out: &mut BTreeMap<String, String>) {
        match value {
            serde_yaml::Value::Mapping(values) => {
                for (key, value) in values {
                    let key = key.as_str().expect("translation keys must be strings");
                    let path = if prefix.is_empty() {
                        key.into()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    flatten(&path, value, out);
                }
            }
            serde_yaml::Value::String(value) => {
                assert!(!value.trim().is_empty(), "empty translation: {prefix}");
                assert!(
                    out.insert(prefix.into(), value.clone()).is_none(),
                    "duplicate key: {prefix}"
                );
            }
            _ => panic!("invalid translation at {prefix}"),
        }
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("locales/{locale}.yml"));
    let value = serde_yaml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let mut out = BTreeMap::new();
    flatten("", &value, &mut out);
    out
}

#[test]
fn locales_have_identical_nonempty_keys() {
    let zh = catalogue("zh-CN");
    let en = catalogue("en");
    assert_eq!(zh.keys().collect::<Vec<_>>(), en.keys().collect::<Vec<_>>());
    for key in en.keys().filter(|key| key.ends_with(".one")) {
        assert!(en.contains_key(&format!("{}.other", key.trim_end_matches(".one"))));
    }
}

#[test]
fn translation_keys_and_parameters_have_semantic_names() {
    for locale in ["en", "zh-CN"] {
        for (key, value) in catalogue(locale) {
            assert!(!value.contains("%{arg"), "positional parameter: {key}");
            assert!(key.len() <= 45, "long key: {key}");
            assert!(key.split('.').count() <= 3, "deep key: {key}");
            for segment in key.split('.') {
                assert!(segment.starts_with(|c: char| c.is_ascii_lowercase()));
                assert!(segment.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
                assert!(!segment.ends_with('_'));
                assert!(!segment.contains("__"));
            }
        }
    }
}

#[test]
fn bilingual_parameters_match() {
    let zh = catalogue("zh-CN");
    for (key, en) in catalogue("en") {
        if key == "install.installed_single" {
            // The original Chinese single-install notice deliberately omits the count.
            assert_eq!(zh[&key], "已安装");
            continue;
        }
        assert_eq!(parameters(&en), parameters(&zh[&key]), "{key}");
    }
}

#[test]
fn identical_bilingual_messages_share_one_key() {
    let zh = catalogue("zh-CN");
    let mut seen = BTreeMap::new();
    for (key, en) in catalogue("en") {
        let pair = (zh[&key].clone(), en);
        assert!(
            seen.insert(pair, key.clone()).is_none(),
            "duplicate bilingual message: {key}"
        );
    }
}

fn test_only(attrs: &[syn::Attribute]) -> bool {
    fn enabled_without_tests(meta: &syn::Meta) -> bool {
        match meta {
            syn::Meta::Path(path) => !path.is_ident("test"),
            syn::Meta::List(list) => {
                let Ok(args) = list.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                ) else {
                    return true;
                };
                if list.path.is_ident("all") {
                    args.iter().all(enabled_without_tests)
                } else if list.path.is_ident("any") {
                    args.iter().any(enabled_without_tests)
                } else if list.path.is_ident("not") {
                    !args.iter().all(enabled_without_tests)
                } else {
                    true
                }
            }
            _ => true,
        }
    }
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || (attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Meta>()
                    .is_ok_and(|meta| !enabled_without_tests(&meta)))
    })
}

struct SourceCheck<'a> {
    path: &'a Path,
    keys: &'a BTreeMap<String, String>,
}
impl SourceCheck<'_> {
    fn literal(&self, literal: &syn::LitStr) {
        assert!(
            !literal.value().chars().any(|c| matches!(c as u32,
                0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x323af
            )),
            "hard-coded CJK string in {}: {:?}",
            self.path.display(),
            literal.value()
        );
    }
    fn tokens(&self, stream: TokenStream) {
        for token in stream {
            match token {
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Literal(literal) => {
                    if let Ok(literal) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        self.literal(&literal);
                    }
                }
                _ => {}
            }
        }
    }
}
impl<'ast> Visit<'ast> for SourceCheck<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !test_only(&item.attrs) {
            visit::visit_item_mod(self, item);
        }
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !test_only(&item.attrs) {
            visit::visit_item_fn(self, item);
        }
    }
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if !test_only(&item.attrs) {
            visit::visit_item_const(self, item);
        }
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if !test_only(&item.attrs) {
            visit::visit_impl_item_fn(self, item);
        }
    }
    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.literal(literal);
    }
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.tokens(item.tokens.clone());
        if item
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "t" || s.ident == "user_error" || s.ident == "user_bail")
        {
            if let Some(TokenTree::Literal(key)) = item.tokens.clone().into_iter().next() {
                let key: syn::LitStr = syn::parse_str(&key.to_string()).unwrap();
                assert!(
                    self.keys.contains_key(&key.value()),
                    "unknown key {} in {}",
                    key.value(),
                    self.path.display()
                );
                let expressions =
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                        .parse2(item.tokens.clone())
                        .expect("invalid translation arguments");
                let supplied: std::collections::BTreeSet<_> = expressions
                    .iter()
                    .skip(1)
                    .map(|expr| {
                        let syn::Expr::Assign(assign) = expr else {
                            panic!("expected named parameter")
                        };
                        let syn::Expr::Path(path) = &*assign.left else {
                            panic!("expected parameter name")
                        };
                        path.path.get_ident().unwrap().to_string()
                    })
                    .collect();
                for parameter in parameters(&self.keys[&key.value()]) {
                    assert!(
                        supplied.contains(parameter),
                        "missing parameter {parameter} for {} in {}",
                        key.value(),
                        self.path.display()
                    );
                }
            }
        }
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if path
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "tn" || s.ident == "finish_batch")
            {
                if let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(key),
                    ..
                })) = call.args.first()
                {
                    for suffix in ["one", "other"] {
                        assert!(
                            self.keys.contains_key(&format!("{}.{suffix}", key.value())),
                            "unknown plural key {} in {}",
                            key.value(),
                            self.path.display()
                        );
                    }
                }
            }
        }
        visit::visit_expr_call(self, call);
    }
}

#[test]
fn production_strings_are_localized_and_keys_exist() {
    let keys = catalogue("en");
    for entry in walkdir::WalkDir::new(Path::new(env!("CARGO_MANIFEST_DIR")).join("src")) {
        let entry = entry.unwrap();
        if entry.path().extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let source = fs::read_to_string(entry.path()).unwrap();
        let file = syn::parse_file(&source).unwrap();
        SourceCheck {
            path: entry.path(),
            keys: &keys,
        }
        .visit_file(&file);
    }
}

#[test]
fn user_error_survives_context_and_hides_diagnostics() {
    let message = "friendly message";
    let error = anyhow::Error::new(kitter::i18n::UserError::new(message).detail("/private/path"))
        .context("outer diagnostic");
    assert_eq!(kitter::i18n::error_message(&error), message);
    let diagnostic = kitter::i18n::error_diagnostic(&error);
    assert_eq!(diagnostic, "/private/path");
    let message_only =
        anyhow::Error::new(kitter::i18n::UserError::new(message)).context("outer diagnostic");
    assert_eq!(kitter::i18n::error_diagnostic(&message_only), message);
    let unknown = anyhow::anyhow!("unknown backend failure");
    assert_eq!(
        kitter::i18n::error_message(&unknown),
        kitter::_rust_i18n_translate(&rust_i18n::locale(), "errors.generic")
    );
    assert_eq!(
        kitter::i18n::error_diagnostic(&unknown.context("outer diagnostic")),
        "outer diagnostic: unknown backend failure"
    );
}

#[test]
fn error_macros_interpolate_and_preserve_the_error_type() {
    fn fail(name: &str) -> anyhow::Result<()> {
        kitter::user_bail!("errors.skill_not_found", name = name);
    }
    let expected = kitter::user_error!("errors.skill_not_found", name = "nope");
    assert!(!expected.message.contains("%{name}"));
    assert!(expected.message.ends_with("nope"));
    let error = fail("nope").unwrap_err().context("outer diagnostic");
    assert_eq!(
        error.downcast_ref::<kitter::i18n::UserError>(),
        Some(&expected)
    );
    assert_eq!(kitter::i18n::error_diagnostic(&error), expected.message);
    let error = anyhow::Error::new(expected.detail("specific diagnostic"));
    assert_eq!(
        kitter::i18n::error_diagnostic(&error),
        "specific diagnostic"
    );
}

#[test]
fn detailed_errors_keep_ui_and_cli_messages_separate_in_both_languages() {
    let pairs = [
        ("skill_missing_notice", "skill_not_found"),
        ("skill_added_notice", "skill_added_detail"),
        ("skill_changed_notice", "skill_changed_detail"),
        ("skill_changed_notice", "reference_changed_detail"),
        ("skill_changed_notice", "reference_verify_failed"),
        ("skill_name_conflict_notice", "skill_name_conflict"),
        ("source_empty", "source_empty_detail"),
        ("update_check_failed", "update_check_detail"),
        ("tool_unavailable", "tool_start_failed"),
        ("fetch_failed", "command_failed"),
    ];
    for locale in ["en", "zh-CN"] {
        let translations = catalogue(locale);
        for (message_key, detail_key) in pairs {
            let render = |key| {
                let mut text = translations[&format!("errors.{key}")].clone();
                for name in ["name", "path", "source", "failures", "program", "detail"] {
                    text = text.replace(&format!("%{{{name}}}"), "diagnostic-value");
                }
                assert!(!text.contains("%{"));
                text
            };
            let message = render(message_key);
            let detail = render(detail_key);
            let error = anyhow::Error::new(kitter::i18n::UserError::new(&message).detail(&detail))
                .context("outer diagnostic");
            assert_eq!(kitter::i18n::error_message(&error), message);
            assert_eq!(kitter::i18n::error_diagnostic(&error), detail);
        }
    }
}

#[test]
fn counts_preserve_zero_singular_plural_and_special_chinese_copy() {
    for (count, english, chinese) in [
        (0, "Installed 0 skills", "已安装 0 个技能"),
        (1, "Installed 1 skill", "已安装 1 个技能"),
        (2, "Installed 2 skills", "已安装 2 个技能"),
    ] {
        let args = [("count", count.to_string())];
        assert_eq!(
            kitter::i18n::tn_for_locale("common.installed_skills", count, &args, "en"),
            english
        );
        assert_eq!(
            kitter::i18n::tn_for_locale("common.installed_skills", count, &args, "zh-CN"),
            chinese
        );
    }
    for (locale, expected) in [("en", "Installed 1 skill"), ("zh-CN", "已安装")] {
        let template = kitter::_rust_i18n_translate(locale, "install.installed_single");
        assert_eq!(template.replace("%{count}", "1"), expected);
    }
    assert_eq!(kitter::i18n::locale_for(kitter::config::Language::En), "en");
    assert_eq!(
        kitter::i18n::locale_for(kitter::config::Language::ZhCn),
        "zh-CN"
    );
    assert_eq!(
        kitter::i18n::locale_for(kitter::config::Language::System),
        kitter::i18n::locale_for(kitter::config::Language::system())
    );
}

#[test]
fn cli_languages_share_json_and_keep_exit_codes() {
    let temp = tempfile::tempdir().unwrap();
    let mut json = Vec::new();
    for (locale, expected, error) in [
        ("en", "No groups\n", "error: Enter a group name\n"),
        ("zh_cn", "没有分组\n", "错误：请输入分组名称\n"),
    ] {
        let home = temp.path().join(locale);
        fs::create_dir_all(&home).unwrap();
        let config = serde_json::json!({ "language": locale, "library_dir": home.join("skills") });
        fs::write(
            home.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let run = |args: &[&str]| {
            std::process::Command::new(env!("CARGO_BIN_EXE_kitter"))
                .args(args)
                .env("KITTER_HOME", &home)
                .output()
                .unwrap()
        };
        let output = run(&["group", "list"]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        let output = run(&["group", "create", ""]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(String::from_utf8(output.stderr).unwrap(), error);
        for (args, expected) in [
            (
                vec!["show", "nope"],
                if locale == "en" {
                    "error: Skill not found: nope\n"
                } else {
                    "错误：没有找到技能：nope\n"
                },
            ),
            (
                vec![
                    "install",
                    "--project",
                    "/nonexist",
                    "--target",
                    "universal",
                    "x",
                ],
                if locale == "en" {
                    "error: Project folder does not exist: /nonexist\n"
                } else {
                    "错误：项目文件夹不存在：/nonexist\n"
                },
            ),
        ] {
            let output = run(&args);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(String::from_utf8(output.stderr).unwrap(), expected);
        }
        let output = run(&["group", "list", "--json"]);
        assert!(output.status.success());
        json.push(output.stdout);
    }
    assert_eq!(json[0], b"[]\n");
    assert_eq!(json[0], json[1]);
}
