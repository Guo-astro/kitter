import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "hermes",
  name: "Hermes",
  icon: "builtin:hermes",
  globalOnly: true,
  reads: [],
});
