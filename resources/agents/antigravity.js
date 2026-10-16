import { defineAgent } from "kitter";

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
});
