import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "grok",
  name: "Grok",
  icon: "builtin:grok",
  reads: ["grok", "claude-code", "cursor", "universal"],
  install: {
    project: ".grok/skills",
    global: "~/.grok/skills",
  },
});
