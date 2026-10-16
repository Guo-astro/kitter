import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "claude-code",
  name: "Claude Code",
  icon: "builtin:claude",
  reads: ["claude-code"],
  install: {
    project: ".claude/skills",
    global: "~/.claude/skills",
  },
});
