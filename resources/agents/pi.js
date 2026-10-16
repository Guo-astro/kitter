import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "pi",
  name: "Pi",
  icon: "builtin:pi",
  reads: ["pi", "universal"],
  install: {
    project: ".pi/skills",
    global: "${env:PI_CODING_AGENT_DIR:-~/.pi/agent}/skills",
  },
});
