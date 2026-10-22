import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "grok",
  name: "Grok",
  icon: "builtin:grok",
  install: {
    project: ".grok/skills",
    global: ctx => ctx.path("skills", grokHome(ctx)),
  },
  skills: standard.skills({
    roots(ctx) {
      const roots = [];
      const add = (base, scope) => {
        roots.push({ path: ctx.path("skills", base), scope });
        roots.push({ path: ctx.path("commands", base), scope, layout: "flatMarkdown" });
      };
      for (const dir of ctx.walkUp()) {
        const scope = dir === ctx.cwd ? "local" : "repository";
        for (const config of [".grok", ".agents", ".claude", ".cursor"]) add(ctx.path(config, dir), scope);
      }
      for (const base of [grokHome(ctx), ctx.path("~/.agents"), ctx.path("~/.claude"), ctx.path("~/.cursor")]) add(base, "user");
      roots.push({ path: ctx.path("bundled", grokHome(ctx)), scope: "system" });
      for (const value of configValues(ctx, "paths")) {
        const path = ctx.path(value, grokHome(ctx));
        roots.push(ctx.isFile(path) ? { path, scope: "user", exactFile: path } : { path, scope: "user" });
      }
      roots.push(...pluginRoots(ctx));
      return roots;
    },
    scan: { recursive: { maxDepth: 5 } },
    metadata: "bodyFallback",
    validate: "none",
    promptPath: "file",
    collision: "keepAll",
    filter(entries, ctx) {
      const ignored = configValues(ctx, "ignore").map(value => ctx.canonical(ctx.path(value, grokHome(ctx))));
      const disabled = configValues(ctx, "disabled").map(normalizeName);
      return entries.filter(e => {
        const path = ctx.canonical(e.file).replace(/\\/g, "/");
        return !ignored.some(value => {
          const root = value.replace(/\\/g, "/").replace(/\/$/, "");
          return path === root || path.startsWith(`${root}/`);
        });
      }).map(e => ({ ...e, id: normalizeName(e.name), name: normalizeName(e.name),
        visibility: disabled.includes(normalizeName(e.name)) ? "manualOnly" : e.visibility,
      }));
    },
    resolve(entries) {
      const resolved = [], names = new Map();
      for (const e of entries) {
        if (e.root.plugin) {
          const qualified = `${e.root.plugin.displayName}:${pathIdentity(e)}`;
          if (!names.has(qualified)) {
            e.id = e.name = qualified;
            names.set(qualified, { scope: e.scope, index: resolved.length });
            resolved.push(e);
          }
          continue;
        }
        if (!names.has(e.id)) {
          names.set(e.id, { scope: e.scope, index: resolved.length });
          resolved.push(e);
          continue;
        }
        const winner = names.get(e.id);
        if (winner.scope !== e.scope) continue;
        const directory = pathIdentity(e);
        if (directory !== e.id && !names.has(directory)) {
          e.id = e.name = directory;
          names.set(directory, { scope: e.scope, index: resolved.length });
          resolved.push(e);
          continue;
        }
        if (directory === e.id) {
          const previous = resolved[winner.index], previousDirectory = pathIdentity(previous);
          if (previousDirectory !== e.id && !names.has(previousDirectory)) {
            const contested = e.id;
            previous.id = previous.name = previousDirectory;
            names.delete(contested);
            names.set(previousDirectory, winner);
            names.set(contested, { scope: e.scope, index: resolved.length });
            resolved.push(e);
          }
        }
      }
      return resolved;
    },
    catalog: { render(entries) {
      if (entries.length === 0) return "";
      const budget = 400000, header = "The following skills are available for use:\n\n";
      const wrap = text => `<system-reminder>\n${text}\n</system-reminder>`;
      const row = (e, combined) => {
        const when = e.whenToUse ?? "";
        const descriptionBudget = when === "" ? combined : Math.floor(combined * Math.max(1, catalog.byteLength(e.description))
          / (Math.max(1, catalog.byteLength(e.description)) + Math.max(1, catalog.byteLength(when))));
        const description = catalog.truncate(e.description, descriptionBudget);
        const path = e.promptPath ?? "";
        return when === "" ? `- ${e.name}: ${description}\n  Absolute path: ${path}`
          : `- ${e.name}: ${description}\n  Use when: ${catalog.truncate(when, Math.max(0, combined - descriptionBudget))}\n  Absolute path: ${path}`;
      };
      const full = entries.map(e => row(e, 400)).join("\n");
      if (catalog.byteLength(header) + catalog.byteLength(full) <= budget) return wrap(header + full);
      const perEntry = Math.floor(Math.max(0, budget - catalog.byteLength(header)) / entries.length);
      if (perEntry >= 20) return wrap(header + entries.map(e => row(e, Math.max(0, perEntry - 2))).join("\n"));
      const rows = [];
      let used = 0;
      for (const e of entries) {
        const text = `- ${e.name}`, next = catalog.byteLength(text) + (rows.length === 0 ? 0 : 1);
        if (used + next > Math.max(0, budget - catalog.byteLength(header))) break;
        used += next;
        rows.push(text);
      }
      return { text: wrap((header + rows.join("\n")).trimEnd()), included: rows.length };
    } },
  }),
});

function grokHome(ctx) { return ctx.env("GROK_HOME") ?? ctx.path("~/.grok"); }
function configValues(ctx, key) {
  const values = ctx.readToml(ctx.path("config.toml", grokHome(ctx)))?.skills?.[key];
  return Array.isArray(values) ? values.filter(value => typeof value === "string") : [];
}

function pluginRoots(ctx) {
  const settings = ctx.readToml(ctx.path("config.toml", grokHome(ctx)))?.plugins ?? {};
  const enabled = new Set(settings.enabled ?? []), disabled = new Set(settings.disabled ?? []);
  const seenPaths = new Set(), names = new Set(), roots = [];
  const add = (dir, scope, configured = false, installedName = null) => {
    if (!ctx.isDir(dir)) return;
    const canonical = ctx.canonical(dir);
    if (seenPaths.has(canonical)) return;
    seenPaths.add(canonical);
    const manifest = ["plugin.json", ".grok-plugin/plugin.json", ".claude-plugin/plugin.json"]
      .map(file => ctx.readJson(ctx.path(file, dir))).find(value => value !== null) ?? {};
    const name = installedName ?? manifest.name ?? dir.replace(/\\/g, "/").split("/").at(-1);
    if (names.has(name)) return;
    names.add(name);
    const id = ctx.native.grokPluginId({ path: canonical, scope: configured ? "config" : scope === "repository" ? "project" : "user", name });
    if (disabled.has(name) || disabled.has(id) || (!configured && !enabled.has(name) && !enabled.has(id))) return;
    const component = (key, layout) => {
      const values = manifest[key] ?? [key];
      for (const value of Array.isArray(values) ? values : [values]) {
        if (typeof value !== "string") continue;
        const path = ctx.path(value, dir), target = ctx.canonical(path);
        const rootPath = canonical.replace(/\\/g, "/").replace(/\/$/, "");
        const targetPath = target.replace(/\\/g, "/");
        if (targetPath !== rootPath && !targetPath.startsWith(`${rootPath}/`)) continue;
        roots.push({ path, scope, ...(layout ? { layout } : {}),
          ...(ctx.isFile(path) ? { exactFile: path } : {}), plugin: { id, displayName: name } });
      }
    };
    component("skills");
    component("commands", "flatMarkdown");
  };
  const children = (base, scope) => ctx.listDir(base).forEach(name => add(ctx.path(name, base), scope));
  for (const dir of ctx.walkUp()) {
    children(ctx.path(".grok/plugins", dir), "repository");
    children(ctx.path(".claude/plugins", dir), "repository");
  }
  children(ctx.path("plugins", grokHome(ctx)), "user");
  children(ctx.path("~/.claude/plugins"), "user");
  const registry = ctx.readJson(ctx.path("installed-plugins/registry.json", grokHome(ctx)));
  for (const repo of Object.values(registry?.repos ?? {})) {
    if (typeof repo.path !== "string") continue;
    for (const [name, entry] of Object.entries(repo.plugins ?? {})) {
      const subdir = entry.subdir ?? ".";
      if (typeof subdir === "string" && !subdir.split(/[\\/]/).includes("..") && !/^(?:[\\/]|[A-Za-z]:)/.test(subdir))
        add(ctx.path(subdir, repo.path), "user", false, name);
    }
  }
  const installed = ctx.readJson(ctx.path("~/.claude/plugins/installed_plugins.json"))?.plugins;
  for (const [key, entries] of Object.entries(installed ?? {})) {
    for (const entry of Array.isArray(entries) ? entries : [entries]) {
      if (typeof entry.installPath !== "string" || (entry.scope !== "user" && (ctx.isGlobal || ctx.canonical(entry.projectPath ?? "") !== ctx.canonical(ctx.cwd)))) continue;
      add(entry.installPath, "user", false, key.split("@")[0]);
    }
  }
  for (const value of Array.isArray(settings.paths) ? settings.paths : []) {
    if (typeof value === "string") add(ctx.path(value, grokHome(ctx)), "user", true);
  }
  return roots;
}
function normalizeName(name) {
  return name.trim().replace(/[A-Z]/g, c => c.toLowerCase())
    .replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 64);
}
function pathIdentity(e) {
  const parts = e.file.replace(/\\/g, "/").split("/");
  const file = parts.at(-1);
  const raw = file === "SKILL.md" ? parts.at(-2) : file.replace(/(?!^)\.[^.]*$/, "");
  return normalizeName(raw ?? "") || e.id;
}
