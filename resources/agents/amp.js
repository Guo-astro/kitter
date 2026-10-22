import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "amp",
  name: "Amp",
  icon: "builtin:amp",
  install: { project: ".agents/skills", global: "~/.config/agents/skills" },
  skills: standard.skills({
    roots: ctx => {
      const settings = Object.assign({}, ctx.readJsonc(`${ctx.home}/.config/amp/settings.json`),
        ...ctx.walkUp().reverse().map(dir => ctx.readJsonc(`${dir}/.amp/settings.json`)));
      const claude = settings["amp.skills.disableClaudeCodeSkills"] !== true;
      const roots = standard.roots(ctx, {
        user: ["~/.config/agents/skills",
          ...(settings["amp.skills.disableGlobalAgentsSkills"] !== true ? ["~/.agents/skills"] : []),
          "~/.config/amp/skills"],
      });
      roots.push(...standard.roots(ctx, {
        project: [".agents/skills", ...(claude ? [".claude/skills"] : [])],
        user: claude ? ["~/.claude/skills"] : [],
      }));
      if (claude) roots.push({ path: ctx.path("~/.claude/plugins/cache"), scope: "user",
        plugin: { id: "claude-cache", displayName: "Claude" } });
      const paths = ctx.envPaths("AMP_SKILL_PATHS");
      const files = [
        `${ctx.home}/.config/amp/settings.json`,
        `${ctx.home}/.config/amp/config.json`,
      ];
      if (!ctx.isGlobal) {
        files.push(`${ctx.cwd}/.amp/settings.json`, `${ctx.cwd}/.amp/config.json`);
      }
      for (const file of files) {
        const config = ctx.readJsonc(file);
        const value = config?.skills?.path !== undefined ? config.skills.path
          : config?.skills?.paths !== undefined ? config.skills.paths : config?.skillPath;
        const values = Array.isArray(value) ? value : [value];
        const base = file.slice(0, file.lastIndexOf("/"));
        paths.push(...values.filter(path => typeof path === "string").map(path => ctx.path(path, base)));
      }
      roots.push(...paths.map(path => ({ path, scope: "user", layout: "rootMarkdown" })));
      return roots;
    },
    scan: { recursive: { maxDepth: 5 } },
    visibility: "automatic",
  }),
});
