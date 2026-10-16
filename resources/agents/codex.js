import { defineAgent, standard, catalog } from "kitter";

const INTRO = "A skill is a set of instructions provided through a `SKILL.md` source. Below is the list of skills that can be used. Each entry includes a name, description, and source locator. `file` locators are on the host filesystem, `executor package` locators are owned by their execution environment, `orchestrator package` locators are opaque package identifiers, and `custom resource` locators use their provider's access mechanism.";
const USAGE = "- Discovery: The list above is the skills available in this session (name + description + source locator). `file` entries live on the host filesystem, `executor package` and `orchestrator package` entries are accessed directly through `skills.read`, and `custom resource` entries use their provider's access mechanism.\n- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.\n- Missing/blocked: If a named skill isn't in the list or its source can't be read, say so briefly and continue with the best fallback.\n- How to use a skill (progressive disclosure):\n  1) After deciding to use a skill, the main agent must read its `SKILL.md` completely before taking task actions. For a `file` entry, open the listed path. For an `executor package` or `orchestrator package`, pass the listed locator directly to `skills.read` as `package`; root aliases are resolved automatically. Omit `resource` to read `SKILL.md` directly without calling `skills.list`. If a read is paginated, follow `next_cursor` until EOF.\n  2) When `SKILL.md` references another resource, use the same access mechanism. For executor and orchestrator skills, pass the complete package-contained resource identifier with the same package to `skills.read`; do not treat `skill://` identifiers as filesystem paths.\n  3) If `SKILL.md` points to extra folders such as `references/`, use its routing instructions to identify the resources required for the task. The main agent must read each required instruction or reference file itself before acting on it. Do not delegate reading, summarizing, or interpreting skill instructions to a subagent. Subagents may still perform task work when the selected skill allows it.\n  4) For filesystem-backed skills, prefer running or patching provided scripts instead of retyping large code blocks. For executor and orchestrator skills, use `skills.read` and the available tools; do not invent a local path.\n  5) Reuse provided assets or templates through the same source access mechanism instead of recreating them.\n- Coordination and sequencing:\n  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.\n  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.\n- Context hygiene:\n  - Progressive disclosure applies to selecting relevant files, not partially reading a selected instruction file. Do not load unrelated references, scripts, or assets.\n  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.\n  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.\n- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.";

export default defineAgent({
  apiVersion: 1,
  id: "codex",
  name: "Codex",
  icon: "builtin:codex",
  reads: ["codex", "universal"],
  skills: standard.skills({
    roots(ctx) {
      const roots = [];
      const add = (path, scope, extra = {}) => roots.push({ path, scope, ...extra });
      if (!ctx.isGlobal) {
        const base = ctx.repoRoot ?? ctx.cwd;
        add(`${base}/.codex/skills`, "repository");
        if (base !== ctx.cwd) add(`${ctx.cwd}/.codex/skills`, "local");
        roots.push(...standard.roots(ctx, { project: [".agents/skills"] }));
      }
      const home = ctx.env("CODEX_HOME") ?? `${ctx.home}/.codex`;
      add(`${home}/skills`, "user");
      add(`${ctx.home}/.agents/skills`, "user");
      add(`${home}/skills/.system`, "system", { followSymlinks: false });
      if (ctx.platform !== "windows") add("/etc/codex/skills", "system");
      return roots.concat(ctx.native.codexPluginRoots());
    },
    scan: { recursive: { maxDepth: 6, maxDirectories: 2000, maxEntries: 20000 } },
    validate: "none",
    visibility: "automatic",
    promptPath: "file",
    collision: "keepAll",
    filter(entries, ctx) {
      const home = ctx.env("CODEX_HOME") ?? `${ctx.home}/.codex`;
      const config = ctx.readToml(`${home}/config.toml`);
      const disabled = new Set((config?.skills?.config ?? [])
        .filter(e => typeof e.path === "string" && e.enabled === false)
        .map(e => ctx.canonical(ctx.path(e.path, home))));
      return entries.filter(e => e.hasExplicitDescription &&
        !disabled.has(ctx.canonical(e.file)) && !disabled.has(ctx.canonical(e.dir)))
        .map(e => ({ ...e, visibility: ctx.readYaml(`${e.dir}/agents/openai.yaml`)
          ?.policy?.allow_implicit_invocation === false ? "manualOnly" : "automatic" }));
    },
    catalog: {
      render(entries) {
        if (!entries.length) return { text: "", included: 0 };
        const order = { system: 0, repository: 1, local: 1, user: 2 };
        const cmp = (a, b) => {
          const left = Array.from(a, c => c.codePointAt(0));
          const right = Array.from(b, c => c.codePointAt(0));
          for (let i = 0; i < Math.min(left.length, right.length); i++) {
            if (left[i] !== right[i]) return left[i] - right[i];
          }
          return left.length - right.length;
        };
        const skills = [...entries].sort((a, b) => order[a.scope] - order[b.scope] ||
          cmp(a.name, b.name) || cmp(a.file, b.file));
        const render = (e, description) => `- ${e.name}: ${description} (file: ${e.promptPath ?? ""})`;
        let rows = skills.map(e => render(e, catalog.truncate(e.description, 1024))).join("\n");
        let included = skills.length;
        if (catalog.byteLength(rows) > 8000) {
          const overhead = skills.reduce((n, e) => n + catalog.byteLength(render(e, "")) + 1, 0);
          const budget = Math.floor(Math.max(0, 8000 - overhead) / skills.length);
          if (budget >= 20) {
            rows = skills.map(e => render(e, catalog.truncate(e.description, budget))).join("\n");
          } else {
            const names = [];
            let used = 0;
            for (const e of skills) {
              const row = `- ${e.name}`;
              const next = catalog.byteLength(row) + (names.length ? 1 : 0);
              if (used + next > 8000) break;
              used += next;
              names.push(row);
            }
            rows = names.join("\n");
            included = names.length;
          }
        }
        return { text: `<skills_instructions>\n## Skills\n${INTRO}\n### Available skills\n${rows}\n### How to use skills\n${USAGE}\n</skills_instructions>`, included };
      },
    },
  }),
  install: {
    project: ".codex/skills",
    global: "${env:CODEX_HOME:-~/.codex}/skills",
  },
});
