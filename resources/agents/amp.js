import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "amp",
  name: "Amp",
  icon: "builtin:amp",
  reads: ["claude-code", "universal"],
});
