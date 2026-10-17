# Agent plugins

[简体中文](agent-plugins.zh-CN.md)

## Location and loading

Run `kitter agents` to see available Agents, plugin errors, and the plugin directory (`<Kitter data directory>/agents`, or `$KITTER_HOME/agents` when set). Put ES modules directly in that directory; subdirectories are not scanned. The file must be named `<id>.js`.

A user plugin with a built-in id replaces the whole built-in definition. If that override fails to load, Kitter disables that Agent; it does not fall back to the built-in. Restart the desktop app after manually editing plugin files. Changes made through Agent management take effect immediately. Each CLI command loads plugins again.

## Manage Agents

Use the settings page or these commands to add, remove, update, enable, disable, and reorder Agents:

```bash
kitter agents [--json]
kitter agents add https://github.com/owner/repo [--only maki]
kitter agents add /path/to/plugins
kitter agents add /path/to/maki.js
kitter agents remove maki
kitter agents update [maki]
kitter agents enable maki
kitter agents disable maki
kitter agents move maki --before codex
kitter agents move maki --after codex
kitter agents check
```

Repeat `--only <id>` to select several Agents. GitHub URLs must point to a repository; `owner/repo` shorthand and subdirectory URLs are not supported. If the source has an `agents/` directory, Kitter scans `agents/*.js`; otherwise it scans `*.js` at the source root. Neither scan includes nested directories. Source filenames can differ from Agent ids; added files are saved as `<id>.js`.

`update` without an id checks GitHub sources and updates those with new commits. It also refreshes all local sources. With an id, it refreshes that Agent directly. Only Agents added through management can be removed or updated. Removing an override restores the built-in Agent.

Disabled Agents stay in the management list but are excluded from Skill discovery and installation targets. Enable them to use them again. Settings offers the same operations, and changes take effect immediately.

## Minimal plugin

Save this as `maki.js`. It describes Maki's paths; the Agent itself must support these paths.

```js
import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "maki",
  name: "Maki",
  reads: ["maki", "universal"],
  install: {
    project: ".maki/skills",
    global: "~/.maki/skills",
  },
  skills: standard.skills({
    project: [".maki/skills", ".agents/skills"],
    user: ["~/.maki/skills", "~/.agents/skills"],
  }),
});
```

## Agent fields

`defineAgent(definition)` returns the definition. Export it as the module's default. Unknown definition fields are rejected.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `apiVersion` | number | required | Must be `1`. |
| `id` | string | required | 1–32 ASCII characters; starts with a lowercase letter, then lowercase letters, digits, or `-`. `universal` is reserved. Must match the filename stem. Use canonical ids such as `claude-code`. |
| `name` | string | required | Display name. |
| `icon` | string or `{ light, dark }` | monogram | Inline SVG, theme-specific inline SVG strings, or `builtin:<name>`. Each SVG must start with `<svg` after trimming and be at most 64 KiB. |
| `globalOnly` | boolean | `false` | Agent is available only at user scope. |
| `reads` | string[] | `[]` | Install target ids the Agent can read, including `universal` for `.agents/skills`. Does not set discovery roots. |
| `install` | `{ project, global }` | absent | Enables an install target. `project` is a relative directory; `global` is a path template or `(ctx) => string`. Return an absolute path from the hook. |
| `skills` | object | absent | Enables effective-skill discovery and catalog estimates. Requires `roots`; see below. |

Built-in icon names: `amp`, `antigravity`, `claude`, `codex`, `copilot`, `cursor`, `dimagent`, `droid`, `gemini`, `grok`, `hermes`, `omp`, `openclaw`, `opencode`, `pi`, `trae`. Icon names are separate from Agent ids.

### Paths

Path templates support `~`, `~/…`, `${env:NAME}`, and `${env:NAME:-default}`. An unset or empty variable uses the default (or an empty string). For example, `"${env:MAKI_HOME:-~/.maki}/skills"`.

`install.global`, `roots.user`, and `ctx.path(template)` resolve relative paths against the user home. `ctx.path(template, base)` uses the given base. `install.project` is joined to the target project. Declarative `roots.project` entries are relative paths joined to each directory from the working directory up to the repository root (or filesystem root if none); they are not expanded as templates. Global inspection skips project roots. `envPaths` names a variable containing an OS-separated path list; relative entries use `ctx.cwd`.

## Skill settings

`standard.skills({ project: [], user: [], envPaths, ...overrides })` creates declarative `roots` and sets `validate: "agentskills"`, `visibility: "frontmatter"`, `collision: "firstWins"`, and `catalog: catalog.xml()`. Overrides can replace these fields, including `roots`. The defaults below also apply to a plain `skills` object unless stated otherwise.

| Field | Values / shape | Default and behavior |
| --- | --- | --- |
| `roots` | `{ project?: string[], user?: string[], envPaths?: string }` or hook | Required in plain `skills`; empty arrays from `standard.skills`. Project roots first, then user roots, then environment paths. |
| `scan` | `"recursive"`, `"directChildren"`, `"providerChildren"`, `"dimRecursive"`, `"piIgnored"`, or `{ recursive: { maxDepth?, maxDirectories?, maxEntries? } }` | `"recursive"`; unspecified numeric limits are unbounded. Recursive scanning stops below a found `SKILL.md` and skips hidden directories and `node_modules`. `directChildren` includes the root manifest and child manifests; `providerChildren` only non-hidden child manifests. `dimRecursive` includes hidden directories, excludes the root manifest, and caps depth/directories/entries at 11/2,000/20,000. `piIgnored` follows ignore rules, including `.gitignore` and `.ignore`. |
| `metadata` | `"strict"`, `"bodyFallback"`, `"provider"` | `"strict"` requires parseable frontmatter. Missing names use the directory/file name. Missing descriptions use the first body paragraph; `bodyFallback` also allows missing/invalid frontmatter. `provider` allows missing/invalid frontmatter and uses an empty description if absent. |
| `validate` | `"agentskills"`, `"none"` | `"agentskills"` checks skill name and description validity; `"none"` skips that check. |
| `id` | `"name"`, `"path"` | `"name"`; `"path"` derives the id from the path within its root. |
| `visibility` | `"frontmatter"`, `"automatic"` | `"frontmatter"` uses invocation flags; `"automatic"` makes every scanned entry model-visible. |
| `collision` | `"firstWins"`, `"lastWins"`, `"keepAll"` | `"firstWins"`; resolve duplicate ids after `filter`, with builtins preceding filesystem entries. |
| `promptPath` | `"none"`, `"file"` | `"none"`; `"file"` sets the absolute skill file path. |
| `catalog` | `catalog.xml()` / `{ format: "xml" }` or `{ render }` | XML name/description catalog. |
| `builtins`, `filter`, `resolve` | functions | Absent; see hooks. |

Frontmatter visibility: `disable-model-invocation: true` gives `manualOnly`; otherwise a configured `paths` value gives `conditional`; otherwise `automatic`. Set `nameOnly` through a hook when needed.

## Hooks

Hooks are synchronous and return JSON-compatible values. The pipeline is: `roots` → `builtins` → scan/metadata/validation → `filter` → collisions → `resolve` → visible entries → `catalog.render`. Canonical filesystem paths are deduplicated before `filter`. Only `automatic` and `nameOnly` entries reach the renderer.

| Hook | Input | Return |
| --- | --- | --- |
| `roots(ctx)` | Context | `RootSpec[]`. |
| `builtins(ctx)` | Context | `{ id, name, description, whenToUse?, visibility?, scope? }[]`. Default visibility `automatic`, scope `system`; no file scan or validation. |
| `filter(entries, ctx)` | Scanned filesystem/plugin `SkillEntry[]`, before collisions; excludes builtins | Entry updates in the desired order. Omit entries to remove them. |
| `resolve(entries, ctx)` | All remaining `SkillEntry[]`, including builtins, after collisions | Same update format as `filter`. |
| `catalog.render(entries, ctx)` | Visible `SkillEntry[]` | A string, or `{ text: string, included: number }` to report a different included count. A string counts every input entry. |

`RootSpec` requires `path` (use an absolute path) and `scope` (`local`, `repository`, `user`, `system`). Optional fields: `layout` (`default`, `rootMarkdown`, `flatMarkdown`, `directChildren`), `exactFile` (scan only this file), `followSymlinks` (default `true`), and `plugin: { id, displayName }`. `rootMarkdown` also scans Markdown files at the root; `flatMarkdown` scans only root `.md` files; `directChildren` selects root and immediate child manifests. `standard.roots(ctx, { project, user, envPaths })` returns roots using the declarative path rules.

### SkillEntry

| Fields | Type / meaning |
| --- | --- |
| `key` | Number; stable identity for updates. |
| `id`, `name`, `description` | Strings; skill id, display name, description. |
| `whenToUse` | String or `null`. |
| `hasExplicitDescription` | Boolean; description was supplied in frontmatter (always `true` for builtins). |
| `file`, `dir` | Strings; file and directory paths. Builtins use `"<built-in>"` and `""`. |
| `root` | `{ path, scope, plugin }` or `null` for builtins. `plugin` is `{ id, displayName }` or `null`. |
| `scope` | `local`, `repository`, `user`, or `system`. |
| `frontmatter` | Object with parsed frontmatter; empty for builtins. |
| `visibility` | `automatic`, `nameOnly`, `manualOnly`, or `conditional`. |
| `promptPath` | String or `null`. |
| `source` | `filesystem`, `builtin`, or `plugin`. |

For `filter` and `resolve`, return objects with `key`, `id`, `name`, `visibility`, and optional `promptPath` (omission clears it). Only those fields are applied; other returned fields are ignored. Preserve the full entry with `{ ...entry, ...changes }`. Unknown keys cause an error; hooks cannot create entries.

### Hook example

Save this complete example as `maki-hooks.js`. Each hook is short; its roots use the same helper as the built-in plugins.

```js
import { defineAgent, standard, catalog } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "maki-hooks",
  name: "Maki Hooks",
  skills: standard.skills({
    roots: ctx => standard.roots(ctx, {
      project: [".maki/skills"], user: ["~/.maki/skills"],
    }),
    builtins: ctx => [{ id: "help", name: "help", description: "Show help." }],
    filter: (entries, ctx) => entries.filter(e => e.name !== "private"),
    resolve: (entries, ctx) => entries.map(e => ({
      ...e, promptPath: e.source === "builtin" ? null : e.file,
    })),
    catalog: { render: (entries, ctx) => ({
      text: entries.map(e => `<skill>${catalog.xmlEscape(e.name)}</skill>`).join("\n"),
      included: entries.length,
    }) },
  }),
});
```

## Public helpers

| API | Result |
| --- | --- |
| `ctx.home`, `ctx.cwd`, `ctx.repoRoot` | Absolute home/working/repository paths; repository root can be `null`. |
| `ctx.isGlobal`, `ctx.platform` | Boolean; `"macos"`, `"windows"`, or `"linux"`. |
| `ctx.env(name)` | Non-empty environment string or `null`. |
| `ctx.path(template, base?)` | Expanded path; base defaults to home. |
| `ctx.walkUp()` | Working directory through repository/filesystem root, inclusive; `[]` in global context. |
| `ctx.envPaths(name)` | OS-separated environment paths, made absolute against `cwd`. |
| `ctx.exists(path)`, `ctx.isFile(path)`, `ctx.isDir(path)` | Boolean. |
| `ctx.canonical(path)` | Canonical path, or original path if resolution fails. |
| `ctx.listDir(path)` | Sorted entry names; `[]` if unreadable. |
| `ctx.readText(path)` | Text or `null` on failure. |
| `ctx.readJson(path)`, `ctx.readJsonc(path)`, `ctx.readToml(path)`, `ctx.readYaml(path)` | Parsed JSON-compatible value; `null` on read/parse failure. |
| `ctx.wildcard(pattern, value)` | Case-sensitive match; `*` matches any text. |
| `catalog.xml()` | `{ format: "xml" }`. |
| `catalog.xmlEscape(text)` | XML-escaped text. |
| `catalog.truncate(text, limit)` | Limits Unicode characters with a trailing `…` when cut (a cut at limit 0 still returns `…`). |
| `catalog.byteLength(text)` | UTF-8 byte count. |

`ctx.native.*` is an unstable interface for built-in plugins; do not depend on it.

## Override a built-in Agent

Copy `resources/agents/<id>.js` from the repository's [main branch](https://github.com/what1f/kitter/tree/main/resources/agents) to the plugin directory with the same filename, then edit it. Keep the id unchanged. This replaces the entire definition, including installation and discovery rules.

## Debugging

```bash
kitter agents
kitter agents check
kitter agents check /path/to/agents/maki.js --project /path/to/project
kitter agents --json check /path/to/agents/maki.js --project /path/to/project
```

Without a file, `check` checks all loaded plugins. With a file, it checks that file alongside builtins, ignoring other user files. Without `--project`, it inspects the current directory. Output includes plugin diagnostics and effective-skill counts. For example (excerpt; paths and counts vary):

```text
Plugin directory: /tmp/maki-home/agents
  maki               Maki                 user
Project: /private/tmp/maki-home/project
Agents:
  Maki                 2 discovered    2 visible  ~56 tokens
```

The desktop Settings page shows warning icons for plugin load errors with an identifiable Agent id. Use the CLI for all diagnostics, including runtime hook errors. Check the diagnostics even when the command exits successfully.

## Limits

Loading a plugin and each hook call have a 2-second JavaScript execution deadline; each runtime has a 64 MiB memory limit. Host filesystem calls are synchronous and are not interrupted by that JavaScript deadline.

Plugins can read files exposed by `ctx`, including files outside the plugin directory. They cannot write files, start processes, or access the network. Use only `import … from "kitter"`; filesystem, package, and network module imports are unavailable. The internal bridge is not a public API.
