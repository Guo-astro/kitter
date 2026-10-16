import { defineAgent, standard } from "kitter";

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
  skills: standard.skills({
    project: [".github/skills", ".agents/skills", ".claude/skills"],
    user: ["~/.copilot/skills", "~/.agents/skills", "~/.claude/skills"],
    envPaths: "COPILOT_SKILLS_DIRS",
  }),
});
