import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "pi",
  name: "Pi",
  icon: "builtin:pi",
  install: {
    project: ".pi/skills",
    global: "${env:PI_CODING_AGENT_DIR:-~/.pi/agent}/skills",
  },
  skills: standard.skills({
    roots(ctx) {
      const agent = ctx.path(ctx.env("PI_CODING_AGENT_DIR") ?? "~/.pi/agent");
      const trusted = !ctx.isGlobal && projectTrusted(ctx, agent);
      const roots = [];
      const configured = (settings, base, scope) => {
        const config = ctx.readJsonc(settings);
        const entries = Array.isArray(config?.skills) ? config.skills : config?.skills?.customDirectories;
        for (const value of Array.isArray(entries) ? entries : []) {
          if (typeof value !== "string" || value.startsWith("!")) continue;
          const path = ctx.path(value, base);
          roots.push(path.endsWith(".md")
            ? { path, scope, exactFile: path }
            : { path, scope, layout: "rootMarkdown" });
        }
      };
      if (trusted) {
        configured(ctx.path(".pi/settings.json", ctx.cwd), ctx.path(".pi", ctx.cwd), "repository");
        roots.push({ path: ctx.path(".pi/skills", ctx.cwd), scope: "local", layout: "rootMarkdown" });
        const user = ctx.canonical(ctx.path("~/.agents/skills"));
        for (const dir of ctx.walkUp()) {
          const path = ctx.path(".agents/skills", dir);
          if (ctx.canonical(path) !== user) roots.push({ path, scope: "repository" });
        }
      }
      configured(ctx.path("settings.json", agent), agent, "user");
      roots.push({ path: ctx.path("skills", agent), scope: "user", layout: "rootMarkdown" });
      roots.push({ path: ctx.path("~/.agents/skills"), scope: "user" });
      if (trusted) roots.push(...ctx.native.piPackageRoots({
        settings: ctx.path(".pi/settings.json", ctx.cwd),
        installRoot: ctx.path(".pi/npm/node_modules", ctx.cwd), scope: "repository",
      }));
      roots.push(...ctx.native.piPackageRoots({
        settings: ctx.path("settings.json", agent),
        installRoot: ctx.path("npm/node_modules", agent), scope: "user",
      }));
      return roots;
    },
    scan: "piIgnored",
    validate: "none",
    promptPath: "file",
    filter: entries => entries.filter(e => e.hasExplicitDescription && e.description.trim() !== "")
      .map(e => ({ ...e, visibility: e.visibility === "manualOnly" ? "manualOnly" : "automatic" })),
    catalog: { render(entries) {
      if (entries.length === 0) return "";
      const lines = ["", "",
        "The following skills provide specialized instructions for specific tasks.",
        "Use the read tool to load a skill's file when the task matches its description.",
        "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.",
        "", "<available_skills>"];
      for (const e of entries) lines.push("  <skill>",
        `    <name>${catalog.xmlEscape(e.name)}</name>`,
        `    <description>${catalog.xmlEscape(e.description)}</description>`,
        `    <location>${catalog.xmlEscape(e.promptPath ?? "")}</location>`, "  </skill>");
      lines.push("</available_skills>");
      return lines.join("\n");
    } },
  }),
});

function projectTrusted(ctx, agent) {
  const entries = ctx.readJson(ctx.path("trust.json", agent));
  if (!entries || Array.isArray(entries) || typeof entries !== "object") return true;
  let current = ctx.canonical(ctx.cwd);
  while (true) {
    if (Object.hasOwn(entries, current)) return typeof entries[current] === "boolean" ? entries[current] : true;
    let next = current.replace(/[\\/][^\\/]*[\\/]?$/, "") || (current.startsWith("/") ? "/" : current);
    if (/^[A-Za-z]:$/.test(next)) next += current.includes("\\") ? "\\" : "/";
    if (next === current) return true;
    current = next;
  }
}
