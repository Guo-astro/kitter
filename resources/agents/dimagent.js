import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "dimagent",
  name: "DimAgent",
  icon: "builtin:dimagent",
  reads: ["universal"],
  skills: standard.skills({
    roots(ctx) {
      const roots = [
        { path: ctx.path("skills", dimHome(ctx)), scope: "user" },
        { path: ctx.path("~/.agents/skills"), scope: "user" },
      ];
      if (!ctx.isGlobal) roots.push({ path: ctx.path(".agents/skills", ctx.cwd), scope: "local" });
      return roots;
    },
    scan: "dimRecursive",
    validate: "none",
    visibility: "automatic",
    filter: (entries, ctx) => entries.filter(e => typeof e.frontmatter.name === "string"
      && e.frontmatter.name.trim() !== "" && e.hasExplicitDescription && e.description.trim() !== ""
      && hasBody(ctx.readText(e.file) ?? "")),
    resolve(entries, ctx) {
      const disabled = ctx.readJson(ctx.path("skills.json", dimHome(ctx)))?.disabledSkillIds;
      return entries.filter(e => {
        const dir = e.dir.replace(/\\/g, "/"), root = e.root.path.replace(/\\/g, "/").replace(/\/$/, "");
        const relative = dir === root ? "" : dir.startsWith(`${root}/`) ? dir.slice(root.length + 1) : dir;
        const id = e.scope === "local" ? `workspace:${e.root.path}:${relative}` : `user:${relative}`;
        return !Array.isArray(disabled) || !disabled.includes(id);
      });
    },
    catalog: { render(entries) {
      const sorted = [...entries].sort((a, b) => compareNames(a.name, b.name));
      const costs = sorted.map(e => ({ entry: e, cost: Array.from(e.description).reduce(n => n + 1, 0) + 2 }));
      costs.sort((a, b) => a.cost - b.cost);
      let budget = 4000;
      const full = new Set();
      for (const { entry, cost } of costs) {
        if (cost <= budget) { budget -= cost; full.add(entry.name); }
      }
      return sorted.map(e => full.has(e.name) ? `- ${e.name}: ${e.description}` : `- ${e.name}`).join("\n");
    } },
  }),
});

function dimHome(ctx) {
  const value = ctx.env("DIMCODE_HOME");
  return value?.trim() ? ctx.path(value.trim(), ctx.cwd)
    : `${ctx.env("XDG_CONFIG_HOME") ?? ctx.home}/.dimcode/v2`;
}
function hasBody(text) {
  const lines = text.split(/\r?\n/);
  if (text.trimStart().startsWith("---\n") || text.trimStart().startsWith("---\r\n")) {
    let delimiters = 0;
    while (lines.length && delimiters < 2) if (lines.shift().trim() === "---") delimiters++;
  }
  return lines.some(line => line.trim() !== "");
}
function compareNames(a, b) {
  const left = Array.from(a), right = Array.from(b);
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    const order = left[i].codePointAt(0) - right[i].codePointAt(0);
    if (order) return order;
  }
  return left.length - right.length;
}
