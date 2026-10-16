use rquickjs::{Ctx, Function, Object, Value, function::Opt, object::Property};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

use super::{
    expand_path,
    host::{from_json, to_json},
};
use crate::effective_skills::{cwd_to_boundary, normalize_jsonc, wildcard_match};

#[derive(Clone, Debug)]
pub struct HostContext {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub repo_root: Option<PathBuf>,
    pub is_global: bool,
}

pub type NativeFn = fn(&HostContext, serde_json::Value) -> serde_json::Value;
const NATIVES: &[(&str, NativeFn)] = &[
    ("openclawPluginRoots", |ctx, input| {
        plugin_roots(crate::effective_skills::openclaw_plugin_roots(
            &input["config"],
            Path::new(input["stateDir"].as_str().unwrap_or_default()),
            &discovery_context(ctx),
        ))
    }),
    ("piPackageRoots", |_, input| {
        let scope = match input["scope"].as_str() {
            Some("repository") => crate::effective_skills::SkillScope::Repository,
            _ => crate::effective_skills::SkillScope::User,
        };
        plugin_roots(crate::effective_skills::pi_package_skill_roots(
            Path::new(input["settings"].as_str().unwrap_or_default()),
            Path::new(input["installRoot"].as_str().unwrap_or_default()),
            scope,
        ))
    }),
    ("codexPluginRoots", |ctx, _| {
        plugin_roots(crate::effective_skills::codex_plugin_roots(
            &crate::effective_skills::codex_home(&discovery_context(ctx)),
        ))
    }),
    ("claudePluginRoots", |ctx, _| {
        plugin_roots(crate::effective_skills::claude_plugin_roots(
            &discovery_context(ctx),
        ))
    }),
    ("opencodeConfig", |ctx, _| {
        serde_json::to_value(crate::effective_skills::opencode_config(
            &discovery_context(ctx),
        ))
        .unwrap()
    }),
    ("ompAgentDir", |ctx, _| {
        serde_json::json!(crate::agents::omp_agent_dir(&ctx.home))
    }),
    #[cfg(test)]
    (
        "testEcho",
        |ctx, input| serde_json::json!({"input": input, "cwd": ctx.cwd}),
    ),
];

fn discovery_context(ctx: &HostContext) -> crate::effective_skills::DiscoveryContext {
    crate::effective_skills::DiscoveryContext {
        home: ctx.home.clone(),
        cwd: ctx.cwd.clone(),
        repository_root: ctx.repo_root.clone(),
    }
}

fn plugin_roots(roots: Vec<crate::effective_skills::SkillRoot>) -> serde_json::Value {
    serde_json::Value::Array(
        roots
            .into_iter()
            .map(|root| {
                let plugin = match root.source {
                    crate::effective_skills::SkillSource::Plugin { id, display_name } => {
                        serde_json::json!({ "id": id, "displayName": display_name })
                    }
                    _ => serde_json::Value::Null,
                };
                serde_json::json!({
                    "path": root.path, "scope": root.scope, "exactFile": root.exact_skill_file,
                    "plugin": plugin,
                    "layout": if root.flat_markdown_only { "flatMarkdown" }
                        else if root.direct_children_only { "directChildren" }
                        else if root.include_root_markdown { "rootMarkdown" }
                        else { "default" },
                    "followSymlinks": root.follow_directory_symlinks,
                })
            })
            .collect(),
    )
}

impl HostContext {
    pub(crate) fn walk_up(&self) -> Vec<PathBuf> {
        if self.is_global {
            Vec::new()
        } else {
            cwd_to_boundary(&self.cwd, self.repo_root.as_deref())
        }
    }
    pub(crate) fn env_paths(&self, name: &str) -> Vec<PathBuf> {
        env::var_os(name)
            .map(|value| {
                env::split_paths(&value)
                    .map(|path| {
                        if path.is_absolute() {
                            path
                        } else {
                            self.cwd.join(path)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn object<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        let obj = Object::new(ctx.clone())?;
        obj.prop(
            "home",
            Property::from(self.home.to_string_lossy().into_owned()).enumerable(),
        )?;
        obj.prop(
            "cwd",
            Property::from(self.cwd.to_string_lossy().into_owned()).enumerable(),
        )?;
        obj.prop(
            "repoRoot",
            Property::from(from_json(ctx, &serde_json::json!(self.repo_root))?).enumerable(),
        )?;
        obj.prop("isGlobal", Property::from(self.is_global).enumerable())?;
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        };
        obj.prop("platform", Property::from(platform).enumerable())?;
        obj.set(
            "env",
            Function::new(ctx.clone(), |c: Ctx<'js>, name: String| {
                from_json(
                    &c,
                    &serde_json::json!(env::var(name).ok().filter(|s| !s.is_empty())),
                )
            })?,
        )?;
        let host = self.clone();
        obj.set(
            "path",
            Function::new(ctx.clone(), move |template: String, base: Opt<String>| {
                let base = base
                    .0
                    .map(PathBuf::from)
                    .unwrap_or_else(|| host.home.clone());
                expand_path(&template, &host.home, &base)
                    .to_string_lossy()
                    .into_owned()
            })?,
        )?;
        let host = self.clone();
        obj.set(
            "walkUp",
            Function::new(ctx.clone(), move |c: Ctx<'js>| {
                from_json(&c, &serde_json::json!(host.walk_up()))
            })?,
        )?;
        let host = self.clone();
        obj.set(
            "envPaths",
            Function::new(ctx.clone(), move |c: Ctx<'js>, name: String| {
                from_json(&c, &serde_json::json!(host.env_paths(&name)))
            })?,
        )?;
        obj.set(
            "exists",
            Function::new(ctx.clone(), |p: String| PathBuf::from(p).exists())?,
        )?;
        obj.set(
            "isFile",
            Function::new(ctx.clone(), |p: String| PathBuf::from(p).is_file())?,
        )?;
        obj.set(
            "isDir",
            Function::new(ctx.clone(), |p: String| PathBuf::from(p).is_dir())?,
        )?;
        obj.set(
            "canonical",
            Function::new(ctx.clone(), |p: String| {
                fs::canonicalize(&p)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or(p)
            })?,
        )?;
        obj.set(
            "listDir",
            Function::new(ctx.clone(), |p: String| {
                let mut names: Vec<_> = fs::read_dir(p)
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                names
            })?,
        )?;
        obj.set(
            "readText",
            Function::new(ctx.clone(), |c: Ctx<'js>, p: String| {
                from_json(&c, &serde_json::json!(fs::read_to_string(p).ok()))
            })?,
        )?;
        for (name, format) in [
            ("readJson", "json"),
            ("readJsonc", "jsonc"),
            ("readToml", "toml"),
            ("readYaml", "yaml"),
        ] {
            obj.set(
                name,
                Function::new(ctx.clone(), move |c: Ctx<'js>, p: String| {
                    let value = fs::read_to_string(p)
                        .ok()
                        .and_then(|text| match format {
                            "json" => serde_json::from_str(&text).ok(),
                            "jsonc" => serde_json::from_str(&normalize_jsonc(&text)).ok(),
                            "toml" => toml::from_str::<toml::Value>(&text)
                                .ok()
                                .and_then(|v| serde_json::to_value(v).ok()),
                            "yaml" => serde_yaml::from_str::<serde_json::Value>(&text).ok(),
                            _ => unreachable!(),
                        })
                        .unwrap_or(serde_json::Value::Null);
                    from_json(&c, &value)
                })?,
            )?;
        }
        obj.set(
            "wildcard",
            Function::new(ctx.clone(), |pattern: String, value: String| {
                wildcard_match(&pattern, &value)
            })?,
        )?;
        let native = Object::new(ctx.clone())?;
        for &(name, function) in NATIVES {
            let host = self.clone();
            native.set(
                name,
                Function::new(ctx.clone(), move |c: Ctx<'js>, input: Opt<Value<'js>>| {
                    let input = match input.0 {
                        Some(v) => to_json(&c, v)?,
                        None => serde_json::Value::Null,
                    };
                    from_json(&c, &function(&host, input))
                })?,
            )?;
        }
        obj.set("native", native)?;
        Ok(obj)
    }
}
