import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "antigravity",
  name: "Antigravity",
  icon: "builtin:antigravity",
  install: {
    project: ".agent/skills",
    global: "~/.gemini/config/skills",
  },
  skills: standard.skills({
    roots(ctx) {
      const roots = [];
      const plugins = (base, scope) => {
        for (const name of ctx.listDir(base)) {
          const dir = ctx.path(name, base);
          if (ctx.isDir(dir)) roots.push({ path: ctx.path("skills", dir), scope,
            plugin: { id: ctx.canonical(dir), displayName: name } });
        }
      };
      for (const dir of ctx.walkUp()) {
        const scope = dir === ctx.cwd ? "local" : "repository";
        for (const suffix of [".agents/skills", ".agent/skills", "_agents/skills", "_agent/skills"])
          roots.push({ path: ctx.path(suffix, dir), scope });
        plugins(ctx.path(".agents/plugins", dir), scope);
      }
      roots.push(...standard.roots(ctx, { user: [
        "~/.gemini/config/skills", "~/.gemini/antigravity-cli/skills",
        "~/.gemini/skills", "~/.gemini/antigravity/skills",
      ] }));
      plugins(ctx.path("~/.gemini/config/plugins"), "user");
      roots.push({ path: ctx.path("~/.gemini/antigravity-cli/builtin/skills"), scope: "system" });
      return roots;
    },
    visibility: "automatic",
  }),
});
