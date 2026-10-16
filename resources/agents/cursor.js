import { defineAgent } from "kitter";

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
});
