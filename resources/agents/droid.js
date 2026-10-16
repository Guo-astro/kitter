import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "droid",
  name: "Droid",
  icon: "builtin:droid",
  reads: ["droid", "antigravity", "universal"],
  install: {
    project: ".factory/skills",
    global: "~/.factory/skills",
  },
  skills: standard.skills({
    project: [".factory/skills", ".agents/skills", ".agent/skills"],
    user: ["~/.factory/skills", "~/.agents/skills", "~/.agent/skills"],
  }),
});
