import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "hermes",
  name: "Hermes",
  icon: "builtin:hermes",
  install: { project: ctx => {
      const root = trustedProjectRoot(ctx);
      return root ? ctx.path(".hermes/skills", root) : null;
    },
    global: "${env:HERMES_HOME:-~/.hermes}/skills" },
  skills: standard.skills({
    roots: ctx => {
      const home = ctx.env("HERMES_HOME") ?? `${ctx.home}/.hermes`;
      const config = ctx.readYaml(`${home}/config.yaml`);
      const external = config?.skills?.external_dirs;
      const root = trustedProjectRoot(ctx, config);
      const project = root ? [".hermes/skills", ".agents/skills"]
        .map(dir => ({ path: ctx.path(dir, root), scope: "repository" })) : [];
      return [
        ...project,
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

function trustedProjectRoot(ctx, config) {
  if (ctx.isGlobal || !ctx.repoRoot || ctx.canonical(ctx.repoRoot) === ctx.canonical(ctx.home)) return null;
  config ??= ctx.readYaml(ctx.path("config.yaml", ctx.env("HERMES_HOME") ?? ctx.path("~/.hermes")));
  const trusted = config?.skills?.trusted_project_dirs;
  return config?.skills?.project_discovery !== false
    && Array.isArray(trusted) && trusted.some(dir =>
      typeof dir === "string" && ctx.canonical(ctx.path(dir)) === ctx.canonical(ctx.repoRoot))
    ? ctx.repoRoot : null;
}
