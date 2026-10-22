import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "droid",
  name: "Droid",
  icon: "builtin:droid",
  install: {
    project: ".factory/skills",
    global: "~/.factory/skills",
  },
  skills: standard.skills({
    project: [".factory/skills", ".agents/skills", ".agent/skills"],
    user: ["~/.factory/skills", "~/.agents/skills", "~/.agent/skills"],
    filter(entries, ctx) {
      const files = [ctx.path("~/.factory/settings.json"),
        ...ctx.walkUp().map(dir => ctx.path(".factory/settings.json", dir))];
      const disabled = new Set(files.flatMap(file => {
        const names = ctx.readJsonc(file)?.disabledSkills;
        return Array.isArray(names) ? names : [];
      }));
      return entries.filter(e => e.frontmatter.enabled !== false && !disabled.has(e.name));
    },
  }),
});
