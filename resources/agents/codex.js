import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "codex",
  name: "Codex",
  icon: "builtin:codex",
  reads: ["codex", "universal"],
  install: {
    project: ".codex/skills",
    global: "${env:CODEX_HOME:-~/.codex}/skills",
  },
});
