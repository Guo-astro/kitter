import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "droid",
  name: "Droid",
  icon: "builtin:droid",
  reads: ["droid", "antigravity", "universal"],
  install: {
    project: ".factory/skills",
    global: "~/.factory/skills",
  },
});
