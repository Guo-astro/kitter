use std::path::{Path, PathBuf};

/// Expands an Agent plugin path template.
///
/// `${env:NAME}` and `${env:NAME:-default}` are substituted first (an unset or
/// empty variable uses the default), then a leading `~` resolves to `home`, and
/// a path that is still relative is joined onto `base`.
pub fn expand_path(template: &str, home: &Path, base: &Path) -> PathBuf {
    expand_path_with(template, home, base, |name| {
        std::env::var(name).ok().filter(|value| !value.is_empty())
    })
}

fn expand_path_with(
    template: &str,
    home: &Path,
    base: &Path,
    env: impl Fn(&str) -> Option<String>,
) -> PathBuf {
    let mut expanded = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${env:") {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        expanded.push_str(&rest[..start]);
        let body = &rest[start + "${env:".len()..start + end];
        let (name, default) = body.split_once(":-").unwrap_or((body, ""));
        expanded.push_str(&env(name).unwrap_or_else(|| default.to_string()));
        rest = &rest[start + end + 1..];
    }
    expanded.push_str(rest);

    let path = if expanded == "~" {
        home.to_path_buf()
    } else if let Some(relative) = expanded.strip_prefix("~/") {
        home.join(relative)
    } else {
        PathBuf::from(expanded)
    };
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(template: &str, env: &[(&str, &str)]) -> PathBuf {
        expand_path_with(template, Path::new("/home/u"), Path::new("/base"), |name| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        })
    }

    #[test]
    fn expands_env_defaults_home_and_relative_paths() {
        assert_eq!(
            expand("~/.cursor/skills", &[]),
            Path::new("/home/u/.cursor/skills")
        );
        assert_eq!(expand("~", &[]), Path::new("/home/u"));
        assert_eq!(expand(".pi/skills", &[]), Path::new("/base/.pi/skills"));
        assert_eq!(
            expand("${env:CODEX_HOME:-~/.codex}/skills", &[]),
            Path::new("/home/u/.codex/skills")
        );
        assert_eq!(
            expand(
                "${env:CODEX_HOME:-~/.codex}/skills",
                &[("CODEX_HOME", "/opt/codex")]
            ),
            Path::new("/opt/codex/skills")
        );
        assert_eq!(
            expand("${env:PI_DIR:-~/.pi/agent}", &[("PI_DIR", "~/custom")]),
            Path::new("/home/u/custom")
        );
        assert_eq!(expand("${env:MISSING}/x", &[]), Path::new("/x"));
    }
}
