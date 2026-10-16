import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "cursor",
  name: "Cursor",
  icon: "builtin:cursor",
  reads: ["cursor", "claude-code", "codex", "universal"],
  install: {
    project: ".cursor/skills",
    global: "~/.cursor/skills",
  },
  skills: standard.skills({
    project: [".cursor/skills", ".agents/skills", ".claude/skills", ".codex/skills"],
    user: ["~/.cursor/skills", "~/.agents/skills", "~/.claude/skills", "~/.codex/skills"],
  }),
});
