import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "amp",
  name: "Amp",
  icon: "builtin:amp",
  reads: ["claude-code", "universal"],
  skills: standard.skills({
    roots: ctx => {
      const roots = standard.roots(ctx, {
        user: ["~/.config/agents/skills", "~/.agents/skills", "~/.config/amp/skills"],
      });
      roots.push(...standard.roots(ctx, {
        project: [".agents/skills", ".claude/skills"],
        user: ["~/.claude/skills"],
      }));
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
    visibility: "automatic",
  }),
});
