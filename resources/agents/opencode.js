import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "opencode",
  name: "OpenCode",
  icon: "builtin:opencode",
  reads: ["opencode", "claude-code", "universal"],
  install: {
    project: ".opencode/skills",
    global: "${env:XDG_CONFIG_HOME:-~/.config}/opencode/skills",
  },
});
