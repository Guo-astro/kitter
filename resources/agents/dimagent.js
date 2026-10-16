import { defineAgent } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "dimagent",
  name: "DimAgent",
  icon: "builtin:dimagent",
  reads: ["universal"],
});
