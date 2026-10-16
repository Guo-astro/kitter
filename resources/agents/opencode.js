import { defineAgent, standard, catalog } from "kitter";

function permission(ctx, name) {
  let effect = "allow";
  for (const [pattern, candidate] of ctx.native.opencodeConfig().permissionRules) {
    if (ctx.wildcard(pattern, name)) effect = candidate;
  }
  return effect;
}

export default defineAgent({
  apiVersion: 1,
  id: "opencode",
  name: "OpenCode",
  icon: "builtin:opencode",
  reads: ["opencode", "claude-code", "universal"],
  skills: standard.skills({
    roots(ctx) {
      const roots = [];
      const add = (path, scope) => roots.push({ path, scope, layout: "rootMarkdown" });
      const dirs = ctx.walkUp().reverse();
      const truthy = name => ["1", "true", "yes", "on"].includes((ctx.env(name) ?? "").trim().toLowerCase());
      if (!truthy("OPENCODE_DISABLE_EXTERNAL_SKILLS")) {
        if (!truthy("OPENCODE_DISABLE_CLAUDE_CODE") && !truthy("OPENCODE_DISABLE_CLAUDE_CODE_SKILLS")) {
          add(`${ctx.home}/.claude/skills`, "user");
          for (const dir of dirs) add(`${dir}/.claude/skills`, "repository");
        }
        add(`${ctx.home}/.agents/skills`, "user");
        for (const dir of dirs) add(`${dir}/.agents/skills`, "repository");
      }
      const home = `${ctx.env("XDG_CONFIG_HOME") ?? `${ctx.home}/.config`}/opencode`;
      for (const relative of ["skills", "skill"]) add(`${home}/${relative}`, "user");
      for (const dir of dirs) {
        for (const relative of ["skills", "skill"]) add(`${dir}/.opencode/${relative}`, "repository");
      }
      const custom = ctx.env("OPENCODE_CONFIG_DIR");
      if (custom !== null) {
        for (const relative of ["skill", "skills"]) add(`${custom}/${relative}`, "user");
      }
      if (ctx.platform === "macos") {
        for (const relative of ["skill", "skills"]) add(`/Library/Application Support/opencode/${relative}`, "system");
      }
      for (const [path, scope] of ctx.native.opencodeConfig().skillPaths) add(path, scope);
      return roots;
    },
    validate: "none",
    id: "path",
    visibility: "automatic",
    collision: "lastWins",
    builtins(ctx) {
      if (permission(ctx, "customize-opencode") === "deny") return [];
      return [{ id: "customize-opencode", name: "customize-opencode", description: "Use ONLY when the user is editing or creating opencode's own configuration: opencode.json, opencode.jsonc, files under .opencode/, or files under ~/.config/opencode/. Also use when creating or fixing opencode agents, subagents, skills, plugins, MCP servers, or permission rules. Do not use for the user's own application code, or for any project that is not configuring opencode itself." }];
    },
    filter(entries, ctx) {
      const rules = ctx.native.opencodeConfig().permissionRules;
      return entries.filter(e => {
        let effect = "allow";
        for (const [pattern, candidate] of rules) {
          if (ctx.wildcard(pattern, e.id)) effect = candidate;
        }
        return effect !== "deny";
      }).map(e => {
        const meta = e.frontmatter.metadata;
        const disabled = value => value === false || value === "false";
        return { ...e, visibility: !e.hasExplicitDescription || !e.description.trim() ||
          disabled(meta?.["opencode/autoinvoke"]) || disabled(meta?.opencode?.autoinvoke) ? "manualOnly" : "automatic" };
      });
    },
    catalog: {
      render(skills) {
        if (!skills.length) return "";
        const body = skills.map(e => `  <skill>\n    <id>${catalog.xmlEscape(e.id)}</id>\n    <name>${catalog.xmlEscape(e.name)}</name>\n    <description>${catalog.xmlEscape(e.description)}</description>\n  </skill>`).join("\n");
        return `Skills provide specialized instructions and workflows for specific tasks.\nUse the skill tool to load a skill when a task matches its description.\n<available_skills>\n${body}\n</available_skills>`;
      },
    },
  }),
  install: {
    project: ".opencode/skills",
    global: "${env:XDG_CONFIG_HOME:-~/.config}/opencode/skills",
  },
});
