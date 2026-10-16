# Agent 插件

[English](agent-plugins.md)

## 位置与加载

运行 `kitter agents` 查看可用 Agent、插件错误和插件目录（`<Kitter 数据目录>/agents`；设置 `KITTER_HOME` 后为 `$KITTER_HOME/agents`）。将 ES module 直接放在该目录下；不扫描子目录。文件名必须为 `<id>.js`。

用户插件与内置插件同 id 时，会替换整个内置定义。覆盖插件加载失败后，该 Agent 被禁用，不会回退到内置版本。修改插件后需要重启桌面端；每次执行 CLI 命令都会重新加载。

## 最小插件

保存为 `maki.js`。示例声明 Maki 的路径；Agent 本身必须支持这些路径。

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

## Agent 字段

`defineAgent(definition)` 返回传入的定义，将其作为模块的默认导出。定义中出现未知字段会报错。

| 字段 | 类型 | 默认值 | 含义 |
| --- | --- | --- | --- |
| `apiVersion` | number | 必填 | 必须为 `1`。 |
| `id` | string | 必填 | 1–32 个 ASCII 字符；以小写字母开头，其余为小写字母、数字或 `-`。`universal` 为保留值。必须与文件名去掉扩展名后相同。使用 `claude-code` 等规范 id。 |
| `name` | string | 必填 | 显示名称。 |
| `icon` | string 或 `{ light, dark }` | 首字母图标 | 内联 SVG、按主题提供的内联 SVG 字符串，或 `builtin:<name>`。每个 SVG 去掉首尾空白后必须以 `<svg` 开头，且不超过 64 KiB。 |
| `globalOnly` | boolean | `false` | Agent 仅在用户级可用。 |
| `reads` | string[] | `[]` | Agent 能读取的安装目标 id；`universal` 表示 `.agents/skills`。不用于设置发现目录。 |
| `install` | `{ project, global }` | 无 | 启用安装目标。`project` 为相对目录；`global` 为路径模板或 `(ctx) => string`。钩子应返回绝对路径。 |
| `skills` | object | 无 | 启用生效 Skill 发现与目录上下文估算。必须提供 `roots`，见下文。 |

内置图标名：`amp`、`antigravity`、`claude`、`codex`、`copilot`、`cursor`、`dimagent`、`droid`、`gemini`、`grok`、`hermes`、`omp`、`openclaw`、`opencode`、`pi`、`trae`。图标名与 Agent id 是两套独立名称。

### 路径

路径模板支持 `~`、`~/…`、`${env:NAME}` 和 `${env:NAME:-default}`。变量未设置或为空时使用默认值（没有默认值则为空字符串）。例如：`"${env:MAKI_HOME:-~/.maki}/skills"`。

`install.global`、`roots.user` 和 `ctx.path(template)` 中的相对路径以用户主目录为基准；`ctx.path(template, base)` 使用指定基准。`install.project` 拼接到目标项目目录。声明式 `roots.project` 是相对路径，逐一拼接到从工作目录向上至仓库根目录（没有仓库时至文件系统根目录）的每一级目录，不按模板展开。全局检查跳过项目目录。`envPaths` 指定一个环境变量名，其值为操作系统分隔符分隔的路径列表；相对项以 `ctx.cwd` 为基准。

## Skill 设置

`standard.skills({ project: [], user: [], envPaths, ...overrides })` 生成声明式 `roots`，并设置 `validate: "agentskills"`、`visibility: "frontmatter"`、`collision: "firstWins"` 和 `catalog: catalog.xml()`。可覆盖这些字段，包括 `roots`。除特别说明外，下表默认值也适用于直接编写的 `skills` 对象。

| 字段 | 取值 / 结构 | 默认值与行为 |
| --- | --- | --- |
| `roots` | `{ project?: string[], user?: string[], envPaths?: string }` 或钩子 | 直接编写 `skills` 时必填；`standard.skills` 默认使用空数组。顺序为项目目录、用户目录、环境变量路径。 |
| `scan` | `"recursive"`、`"directChildren"`、`"providerChildren"`、`"dimRecursive"`、`"piIgnored"` 或 `{ recursive: { maxDepth?, maxDirectories?, maxEntries? } }` | 默认 `"recursive"`；未指定的数值限制为无限。递归扫描遇到 `SKILL.md` 后不再深入，跳过隐藏目录和 `node_modules`。`directChildren` 包含根目录及直接子目录的清单；`providerChildren` 只包含非隐藏子目录的清单。`dimRecursive` 包含隐藏目录，不包含根目录清单，深度/目录数/条目数上限为 11/2,000/20,000。`piIgnored` 遵守 `.gitignore`、`.ignore` 等忽略规则。 |
| `metadata` | `"strict"`、`"bodyFallback"`、`"provider"` | 默认 `"strict"`，要求可解析的 frontmatter。缺少名称时使用目录名/文件名；缺少描述时使用正文首段。`bodyFallback` 还允许缺失或无效的 frontmatter；`provider` 允许缺失或无效的 frontmatter，缺少描述时使用空字符串。 |
| `validate` | `"agentskills"`、`"none"` | 默认 `"agentskills"`，检查 Skill 名称和描述是否合法；`"none"` 跳过此检查。 |
| `id` | `"name"`、`"path"` | 默认 `"name"`；`"path"` 根据根目录内的路径生成 id。 |
| `visibility` | `"frontmatter"`、`"automatic"` | 默认 `"frontmatter"`，使用调用标记；`"automatic"` 让所有扫描条目对模型可见。 |
| `collision` | `"firstWins"`、`"lastWins"`、`"keepAll"` | 默认 `"firstWins"`；在 `filter` 后处理重复 id，内置条目排在文件系统条目前。 |
| `promptPath` | `"none"`、`"file"` | 默认 `"none"`；`"file"` 设置 Skill 文件的绝对路径。 |
| `catalog` | `catalog.xml()` / `{ format: "xml" }` 或 `{ render }` | 默认 XML 名称/描述目录。 |
| `builtins`、`filter`、`resolve` | 函数 | 无，见钩子。 |

Frontmatter 可见性：`disable-model-invocation: true` 对应 `manualOnly`；否则已配置的 `paths` 对应 `conditional`；其余为 `automatic`。需要 `nameOnly` 时通过钩子设置。

## 钩子

钩子是同步函数，返回值必须兼容 JSON。管线顺序：`roots` → `builtins` → 扫描/元数据/校验 → `filter` → 冲突处理 → `resolve` → 可见条目 → `catalog.render`。文件系统规范路径在 `filter` 前去重。渲染器只接收 `automatic` 和 `nameOnly` 条目。

| 钩子 | 入参 | 返回值 |
| --- | --- | --- |
| `roots(ctx)` | 上下文 | `RootSpec[]`。 |
| `builtins(ctx)` | 上下文 | `{ id, name, description, whenToUse?, visibility?, scope? }[]`。默认可见性为 `automatic`，作用域为 `system`；不扫描文件、不校验。 |
| `filter(entries, ctx)` | 扫描得到的文件系统/插件 `SkillEntry[]`，在冲突处理前，不含内置条目 | 按期望顺序返回条目更新；省略条目即移除。 |
| `resolve(entries, ctx)` | 冲突处理后剩余的全部 `SkillEntry[]`，含内置条目 | 与 `filter` 相同的更新格式。 |
| `catalog.render(entries, ctx)` | 可见的 `SkillEntry[]` | 字符串，或 `{ text: string, included: number }`，用于报告不同的纳入数量。返回字符串时计入全部输入条目。 |

`RootSpec` 必填 `path`（应使用绝对路径）和 `scope`（`local`、`repository`、`user`、`system`）。可选字段：`layout`（`default`、`rootMarkdown`、`flatMarkdown`、`directChildren`）、`exactFile`（只扫描此文件）、`followSymlinks`（默认 `true`）、`plugin: { id, displayName }`。`rootMarkdown` 额外扫描根目录下的 Markdown 文件；`flatMarkdown` 只扫描根目录 `.md` 文件；`directChildren` 选择根目录及直接子目录的清单。`standard.roots(ctx, { project, user, envPaths })` 按声明式路径规则返回根目录。

### SkillEntry

| 字段 | 类型 / 含义 |
| --- | --- |
| `key` | 数字；用于更新的稳定标识。 |
| `id`、`name`、`description` | 字符串；Skill id、显示名称、描述。 |
| `whenToUse` | 字符串或 `null`。 |
| `hasExplicitDescription` | 布尔值；frontmatter 是否提供了描述（内置条目始终为 `true`）。 |
| `file`、`dir` | 字符串；文件和目录路径。内置条目分别为 `"<built-in>"` 和 `""`。 |
| `root` | `{ path, scope, plugin }`；内置条目为 `null`。`plugin` 为 `{ id, displayName }` 或 `null`。 |
| `scope` | `local`、`repository`、`user` 或 `system`。 |
| `frontmatter` | 解析后的 frontmatter 对象；内置条目为空对象。 |
| `visibility` | `automatic`、`nameOnly`、`manualOnly` 或 `conditional`。 |
| `promptPath` | 字符串或 `null`。 |
| `source` | `filesystem`、`builtin` 或 `plugin`。 |

`filter` 和 `resolve` 返回的对象应包含 `key`、`id`、`name`、`visibility`，以及可选的 `promptPath`（省略时清空）。仅应用这些字段，其他返回字段会被忽略。用 `{ ...entry, ...changes }` 保留完整条目。未知 key 会报错；钩子不能创建条目。

### 钩子示例

将完整示例保存为 `maki-hooks.js`。每个钩子都很短；根目录使用与内置插件相同的辅助函数。

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

## 公开辅助接口

| API | 结果 |
| --- | --- |
| `ctx.home`、`ctx.cwd`、`ctx.repoRoot` | 用户主目录/工作目录/仓库的绝对路径；仓库根目录可为 `null`。 |
| `ctx.isGlobal`、`ctx.platform` | 布尔值；`"macos"`、`"windows"` 或 `"linux"`。 |
| `ctx.env(name)` | 非空环境变量字符串或 `null`。 |
| `ctx.path(template, base?)` | 展开后的路径；基准默认为用户主目录。 |
| `ctx.walkUp()` | 从工作目录到仓库/文件系统根目录，包含两端；全局上下文为 `[]`。 |
| `ctx.envPaths(name)` | 按操作系统分隔符拆分的环境变量路径，相对路径以 `cwd` 转为绝对路径。 |
| `ctx.exists(path)`、`ctx.isFile(path)`、`ctx.isDir(path)` | 布尔值。 |
| `ctx.canonical(path)` | 规范路径；解析失败则返回原路径。 |
| `ctx.listDir(path)` | 排序后的条目名；不可读时为 `[]`。 |
| `ctx.readText(path)` | 文本；失败时为 `null`。 |
| `ctx.readJson(path)`、`ctx.readJsonc(path)`、`ctx.readToml(path)`、`ctx.readYaml(path)` | 解析后的 JSON 兼容值；读取/解析失败时为 `null`。 |
| `ctx.wildcard(pattern, value)` | 区分大小写的匹配；`*` 匹配任意文本。 |
| `catalog.xml()` | `{ format: "xml" }`。 |
| `catalog.xmlEscape(text)` | XML 转义后的文本。 |
| `catalog.truncate(text, limit)` | 限制 Unicode 字符数，截断时以 `…` 结尾（上限为 0 时发生截断仍返回 `…`）。 |
| `catalog.byteLength(text)` | UTF-8 字节数。 |

`ctx.native.*` 是内置插件专用的不稳定接口，请勿依赖。

## 覆盖内置 Agent

从仓库 [main 分支](https://github.com/what1f/kitter/tree/main/resources/agents) 复制 `resources/agents/<id>.js` 到插件目录，保持文件名后再修改。保留原 id。覆盖会替换整个定义，包括安装和发现规则。

## 调试

```bash
kitter agents
kitter agents check
kitter agents check /path/to/agents/maki.js --project /path/to/project
kitter agents --json check /path/to/agents/maki.js --project /path/to/project
```

不指定文件时，`check` 检查全部已加载插件；指定文件时，只检查该文件和内置插件，忽略其他用户文件。不指定 `--project` 时检查当前目录。输出包含插件诊断与生效 Skill 数量。例如（节选；路径和数量会变化）：

```text
Plugin directory: /tmp/maki-home/agents
  maki               Maki                 user
Project: /private/tmp/maki-home/project
Agents:
  Maki                 2 discovered    2 visible  ~56 tokens
```

桌面端设置页为能识别 Agent id 的插件加载错误显示警告图标。使用 CLI 查看全部诊断，包括钩子运行错误。即使命令成功退出，也要检查诊断内容。

## 限制

加载插件和每次钩子调用的 JavaScript 执行超时均为 2 秒，每个运行时的内存上限为 64 MiB。宿主文件系统调用为同步执行，不会被这个 JavaScript 超时中断。

插件可通过 `ctx` 读取文件，包括插件目录外的文件。不能写文件、启动进程或访问网络。仅使用 `import … from "kitter"`；不支持文件系统、包或网络模块导入。内部桥接模块不是公开 API。
