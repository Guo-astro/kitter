import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "copilot",
  name: "GitHub Copilot",
  icon: "builtin:copilot",
  reads: ["copilot", "claude-code", "universal"],
  install: {
    project: ".github/skills",
    global: "~/.copilot/skills",
  },
});
