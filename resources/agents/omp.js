import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "omp",
  name: "omp",
  icon: "builtin:omp",
  reads: ["omp", "claude-code", "codex", "opencode", "antigravity", "copilot", "universal"],
  install: {
    project: ".omp/skills",
    global: (ctx) => ctx.path("skills", ctx.native.ompAgentDir()),
  },
  skills: standard.skills({
    roots(ctx) {
      const settings = readSettings(ctx);
      if (settings.skills?.enabled === false) return [];
      const dirs = ctx.walkUp();
      const homeIndex = dirs.indexOf(ctx.home);
      const project = (ctx.repoRoot || homeIndex < 0 ? dirs : dirs.slice(0, homeIndex)).filter(dir => dir !== ctx.home);
      const roots = [];
      const add = (path, scope) => roots.push({ path, scope, layout: "directChildren" });
      const custom = settings.skills?.customDirectories;
      for (const value of Array.isArray(custom) ? custom : []) {
        if (typeof value === "string") add(ctx.path(value, ctx.cwd), "local");
      }
      for (const [suffix, user, projectToggle, userToggle] of [
        [".omp/skills", ctx.path("skills", ctx.native.ompAgentDir()), "enablePiProject", "enablePiUser"],
        [".claude/skills", ctx.path("~/.claude/skills"), "enableClaudeProject", "enableClaudeUser"],
        [".agent/skills", ctx.path("~/.agent/skills"), "enableAgentsProject", "enableAgentsUser"],
        [".agents/skills", ctx.path("~/.agents/skills"), "enableAgentsProject", "enableAgentsUser"],
        [".codex/skills", `${ctx.env("CODEX_HOME") ?? ctx.path("~/.codex")}/skills`, "enableCodexProject", "enableCodexUser"],
        [".opencode/skills", ctx.path("~/.config/opencode/skills"), "enableOpenCodeProject", "enableOpenCodeUser"],
        [".github/skills", null, "enableGithubProject", "enableGithubUser"],
      ]) {
        if (settings.skills?.[projectToggle] !== false) {
          for (const dir of project) add(ctx.path(suffix, dir), "repository");
        }
        const provider = suffix.split("/")[0].replace(/^\./, "");
        const optedIn = ["omp", "agent", "agents"].includes(provider)
          || (Array.isArray(settings.enabledProviders) && settings.enabledProviders.includes(provider))
          || settings.skills?.[userToggle] === true;
        if (user !== null && optedIn && settings.skills?.[userToggle] !== false) add(user, "user");
      }
      return roots;
    },
    scan: "providerChildren",
    metadata: "provider",
    validate: "none",
    visibility: "automatic",
    collision: "keepAll",
    filter(entries, ctx) {
      const settings = readSettings(ctx);
      const agent = ctx.path("skills", ctx.native.ompAgentDir());
      return entries.filter(e => {
        const root = e.root.path.replace(/\\/g, "/");
        const requiresDescription = root.endsWith(".omp/skills") || e.root.path === agent
          || root.endsWith(".github/skills") || ![".claude/skills", ".agent/skills", ".agents/skills", ".codex/skills", ".opencode/skills"].some(s => root.endsWith(s));
        return e.frontmatter.enabled !== false && !/[\\/]/.test(e.name)
          && (!requiresDescription || (e.hasExplicitDescription && e.description.trim() !== ""))
          && !matches(settings.disabledExtensions, `skill:${e.name}`, ctx)
          && !matches(settings.skills?.ignoredSkills, e.name, ctx);
      }).map(e => ({ ...e,
        visibility: ["hide", "disable-model-invocation", "disableModelInvocation"].some(k => e.frontmatter[k] === true) ? "manualOnly" : "automatic",
        promptPath: `skill://${e.name}`,
      }));
    },
    resolve(entries, ctx) {
      const result = [], paths = new Set(), names = new Set(), contents = new Map();
      for (const e of entries) {
        const path = ctx.canonical(e.file);
        if (paths.has(path)) continue;
        paths.add(path);
        const content = ctx.readText(e.file) ?? "";
        if (contents.has(e.name)) {
          if (contents.get(e.name) === content) continue;
          const segments = e.root.path.replace(/\\/g, "/").split("/").filter(Boolean);
          const namespace = (segments.at(-2) ?? "omp").replace(/^\.+/, "");
          const base = `${namespace}/${e.name}`;
          let name = base, suffix = 2;
          while (names.has(name)) name = `${base}~${suffix++}`;
          e.id = e.name = name;
          e.promptPath = `skill://${name}`;
        } else contents.set(e.name, content);
        names.add(e.name);
        result.push(e);
      }
      const settings = readSettings(ctx);
      return result.filter(e => !matches(settings.disabledExtensions, `skill:${e.name}`, ctx)
        && !matches(settings.skills?.ignoredSkills, e.name, ctx)
        && (!Array.isArray(settings.skills?.includeSkills) || settings.skills.includeSkills.length === 0
          || matches(settings.skills.includeSkills, e.name, ctx)));
    },
    catalog: { render: entries => entries.map(e => `- ${e.name}: ${e.description}`).join("\n") },
  }),
});

function readSettings(ctx) {
  const merged = {};
  const dirs = [ctx.native.ompAgentDir()];
  const nearest = ctx.walkUp().find(dir => ctx.isDir(ctx.path(".omp", dir)));
  if (nearest) dirs.push(ctx.path(".omp", nearest));
  function merge(dest, source) {
    if (!dest || typeof dest !== "object" || Array.isArray(dest)
      || !source || typeof source !== "object" || Array.isArray(source)) return;
    for (const [key, value] of Object.entries(source)) {
      if (value && typeof value === "object" && !Array.isArray(value)) {
        if (!Object.hasOwn(dest, key)) dest[key] = {};
        merge(dest[key], value);
      } else dest[key] = value;
    }
  }
  for (const dir of dirs) {
    for (const file of ["config.yaml", "config.yml", "settings.json"]) merge(merged, ctx.readYaml(ctx.path(file, dir)));
  }
  return merged;
}
function matches(values, value, ctx) {
  return Array.isArray(values) && values.some(pattern => typeof pattern === "string" && ctx.wildcard(pattern, value));
}
