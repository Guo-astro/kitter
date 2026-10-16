import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "antigravity",
  name: "Antigravity",
  icon: "builtin:antigravity",
  reads: ["antigravity", "universal"],
  install: {
    project: ".agent/skills",
    global: "~/.gemini/config/skills",
  },
  skills: standard.skills({
    project: [".agents/skills", ".agent/skills"],
    user: ["~/.gemini/config/skills", "~/.agents/skills", "~/.agent/skills"],
    visibility: "automatic",
  }),
});
