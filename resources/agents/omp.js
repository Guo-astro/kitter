import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "omp",
  name: "omp",
  icon: "builtin:omp",
  reads: ["omp", "claude-code", "codex", "opencode", "antigravity", "copilot", "universal"],
  install: {
    project: ".omp/skills",
    global: (ctx) => ctx.path("skills", ctx.native.ompAgentDir()),
  },
});
