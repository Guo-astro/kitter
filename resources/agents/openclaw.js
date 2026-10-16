import { defineAgent, standard, catalog } from "kitter";

function stateDir(ctx) {
  return ctx.env("OPENCLAW_STATE_DIR") ?? ctx.env("CLAWDBOT_STATE_DIR") ?? `${ctx.home}/.openclaw`;
}

function config(ctx) {
  return ctx.readJsonc(ctx.env("OPENCLAW_CONFIG_PATH") ?? `${stateDir(ctx)}/openclaw.json`);
}

export default defineAgent({
  apiVersion: 1,
  id: "openclaw",
  name: "OpenClaw",
  icon: "builtin:openclaw",
  globalOnly: true,
  reads: [],
  skills: standard.skills({
    roots: ctx => {
      const state = stateDir(ctx);
      const settings = config(ctx);
      const roots = ctx.isGlobal ? [] : [
        { path: `${ctx.cwd}/skills`, scope: "local" },
        { path: `${ctx.cwd}/.agents/skills`, scope: "local" },
      ];
      roots.push(
        { path: `${ctx.home}/.agents/skills`, scope: "user" },
        { path: `${state}/skills`, scope: "user" },
      );
      const extra = settings?.skills?.load?.extraDirs;
      roots.push(...(Array.isArray(extra) ? extra : [])
        .filter(path => typeof path === "string")
        .map(path => ({ path: ctx.path(path, ctx.cwd), scope: "user" })));
      roots.push(...ctx.native.openclawPluginRoots({ config: settings, stateDir: state }));
      return roots;
    },
    validate: "none",
    filter: (entries, ctx) => {
      const disabled = config(ctx)?.skills?.entries;
      return entries.filter(entry => disabled?.[entry.name]?.enabled !== false);
    },
    catalog: {
      render: skills => {
        if (!skills.length) return "";
        const body = skills.map(skill => `  <skill>\n    <name>${catalog.xmlEscape(skill.name)}</name>\n    <description>${catalog.xmlEscape(skill.description)}</description>\n  </skill>`).join("\n");
        return catalog.truncate(`<available_skills>\n${body}\n</available_skills>`, 30_000);
      },
    },
  }),
});
