# Write an Agent plugin

Use this guide when the user wants Kitter to support an Agent it does not know yet, or to change how Kitter treats a built-in Agent. A plugin tells Kitter where an Agent reads skills and where to install them. It does not change the Agent itself, so first confirm which directories the Agent actually reads.

## Minimal plugin

Save the file as `<id>.js`, for example `maki.js`:

```js
import { defineAgent, standard } from "kitter";

export default defineAgent({
  apiVersion: 1,
  id: "maki",
  name: "Maki",
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

| Field | Meaning |
| --- | --- |
| `apiVersion` | Always `1`. |
| `id` | Lowercase letters, digits, and `-`, starting with a letter. Must match the file name. |
| `name` | Display name. |
| `icon` | Optional. Inline SVG string, `{ light, dark }` SVG strings, or `builtin:<name>`. Without it Kitter shows the first letter of the name. |
| `globalOnly` | Optional. `true` when the Agent only reads user-level skills. |
| `install` | Optional. Makes the Agent an install target: `project` is a relative directory or a hook returning an absolute path; `global` is a user-level template or hook. Choose one recommended native directory per scope, or `.agents/skills` when appropriate. Hooks can return `null` to disable installation for the current destination. |
| `skills` | Optional. Lets `kitter project` show which skills the Agent sees and their context cost. |

Declare all supported read directories in `skills.roots`, separately for project and user scope. Universal install badges use these roots to check `<project>/.agents/skills` and `~/.agents/skills` independently. `install` selects the preferred write directory and does not declare compatibility. Its directory must be one the Agent reads.


In `standard.skills`, list project directories (relative, searched from the working directory up to the repository root) and user directories in the order the Agent reads them. Paths may use `~` and environment variables such as `"${env:MAKI_HOME:-~/.maki}/skills"`.

Agents with special rules, such as reading extra config files or hiding some skills, need hooks. See the [full reference](https://github.com/what1f/kitter/blob/main/docs/agent-plugins.md) and the built-in plugins in [resources/agents](https://github.com/what1f/kitter/tree/main/resources/agents) for examples.

## Verify

Check the file before adding it:

```bash
kitter agents check /path/to/maki.js --project /path/to/project
```

Fix every reported diagnostic, even when the command exits successfully. Confirm that the skill counts match what the Agent really loads in that project.

## Add and share

```bash
kitter agents add /path/to/maki.js
kitter agents add /path/to/folder
kitter agents add https://github.com/owner/repo
```

To share plugins, publish a GitHub repository with the files in `agents/` (or at the repository root). Users add it with `kitter agents add <url>` and later run `kitter agents update` to pick up new commits. Use `--only <id>` to add only some Agents from a source.

## Change a built-in Agent

Copy the built-in file from [resources/agents](https://github.com/what1f/kitter/tree/main/resources/agents), keep the same id, edit it, then add it as above. It replaces the built-in Agent completely. Removing it with `kitter agents remove <id>` restores the built-in.

## Limits

Plugins can read files but cannot write files, run programs, or access the network. They can only import from `"kitter"`.
