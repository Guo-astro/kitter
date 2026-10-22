import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "trae",
  name: "Trae",
  icon: "builtin:trae",
  install: { project: ".trae/skills", global: "~/.trae-cn/skills" },
  skills: standard.skills({
    roots: ctx => [
      ...(!ctx.isGlobal ? [".trae/skills", ".agents/skills"]
        .map(dir => ({ path: ctx.path(dir, ctx.cwd), scope: "local" })) : []),
      { path: ctx.path("~/.trae-cn/skills"), scope: "user" },
    ],
  }),
});
