import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "claude-code",
  name: "Claude Code",
  icon: "builtin:claude",
  reads: ["claude-code"],
  skills: standard.skills({
    roots(ctx) {
      const roots = [];
      const add = (path, scope, layout = "default") => roots.push({ path, scope, layout });
      if (ctx.platform === "macos") {
        add("/Library/Application Support/ClaudeCode/.claude/skills", "system");
        add("/Library/Application Support/ClaudeCode/.claude/commands", "system", "flatMarkdown");
      } else if (ctx.platform !== "windows") {
        add("/etc/claude-code/.claude/skills", "system");
      }
      const home = ctx.env("CLAUDE_CONFIG_DIR") ?? `${ctx.home}/.claude`;
      add(`${home}/skills`, "user");
      add(`${home}/commands`, "user", "flatMarkdown");
      for (const dir of ctx.walkUp()) {
        add(`${dir}/.claude/skills`, "repository");
        add(`${dir}/.claude/commands`, "repository", "flatMarkdown");
      }
      return roots.concat(ctx.native.claudePluginRoots());
    },
    scan: "directChildren",
    metadata: "bodyFallback",
    validate: "none",
    collision: "keepAll",
    filter(entries, ctx) {
      const base = ctx.repoRoot ?? ctx.cwd;
      const files = [`${ctx.home}/.claude/settings.json`, `${base}/.claude/settings.json`,
        `${base}/.claude/settings.local.json`];
      if (ctx.platform === "macos") files.push("/Library/Application Support/ClaudeCode/managed-settings.json");
      else if (ctx.platform !== "windows") files.push("/etc/claude-code/managed-settings.json");
      const overrides = {};
      for (const file of files) {
        for (const [name, state] of Object.entries(ctx.readJson(file)?.skillOverrides ?? {})) {
          if (typeof state === "string") overrides[name] = state;
        }
      }
      return entries.flatMap(e => {
        const parts = e.dir.split(/[\\/]/);
        const index = parts.findIndex((p, i) => p === ".claude" && parts[i + 1] === "plugins" && parts[i + 2] === "cache" && i + 4 < parts.length);
        const cached = index >= 0 ? parts[index + 4] : null;
        const plugin = e.root.plugin?.displayName ?? cached;
        const local = /[\\/]SKILL\.md$/.test(e.file) ? parts[parts.length - 1] : e.name;
        const name = plugin === null ? local : `${plugin}:${local}`;
        if ((e.root.plugin || cached) && !e.hasExplicitDescription && e.whenToUse === null) return [];
        if (!e.root.plugin && !cached && overrides[name] === "off") return [];
        const states = { "name-only": "nameOnly", "user-invocable-only": "manualOnly", on: "automatic" };
        return [{ ...e, id: name, name, visibility: e.root.plugin || cached ? e.visibility : (states[overrides[name]] ?? e.visibility) }];
      });
    },
    catalog: {
      render(skills) {
        if (!skills.length) return "";
        const entries = skills.map(e => [e.name, e.visibility === "nameOnly" ? "" :
          catalog.truncate(e.description + (e.whenToUse ? ` - ${e.whenToUse}` : ""), 250)]);
        const render = (name, description) => description ? `- ${name}: ${description}` : `- ${name}`;
        let rows = entries.map(([name, description]) => render(name, description)).join("\n");
        if (catalog.byteLength(rows) > 8000) {
          const overhead = entries.reduce((n, [name]) => n + catalog.byteLength(name) + 4, 0) + entries.length - 1;
          const budget = Math.floor(Math.max(0, 8000 - overhead) / entries.length);
          rows = entries.map(([name, description]) => budget < 20 || !description ? `- ${name}` :
            render(name, catalog.truncate(description, budget))).join("\n");
        }
        return `<system-reminder>\n${rows}\n</system-reminder>`;
      },
    },
  }),
  install: {
    project: ".claude/skills",
    global: "~/.claude/skills",
  },
});
