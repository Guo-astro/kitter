import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "hermes",
  name: "Hermes",
  icon: "builtin:hermes",
  globalOnly: true,
  reads: [],
  skills: standard.skills({
    roots: ctx => {
      const home = ctx.env("HERMES_HOME") ?? `${ctx.home}/.hermes`;
      const config = ctx.readYaml(`${home}/config.yaml`);
      const external = config?.skills?.external_dirs;
      return [
        { path: `${home}/skills`, scope: "user" },
        ...(Array.isArray(external) ? external : [])
          .filter(path => typeof path === "string")
          .map(path => ({ path: ctx.path(path, home), scope: "user" })),
      ];
    },
    validate: "none",
    visibility: "automatic",
    catalog: {
      render: skills => {
        if (!skills.length) return "";
        const body = skills.map(skill => `- ${skill.name}: ${catalog.truncate(skill.description, 57)}`).join("\n");
        return `<available_skills>\n${body}\n</available_skills>`;
      },
    },
  }),
});
