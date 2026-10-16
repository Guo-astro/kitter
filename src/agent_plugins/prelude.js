import { __native } from "kitter-native";

export function defineAgent(def) { return def; }
export const catalog = {
  xml() { return { format: "xml" }; },
  xmlEscape: __native.xmlEscape,
  truncate: __native.truncate,
  byteLength: __native.byteLength,
};
export const standard = {
  skills({ project = [], user = [], envPaths, ...overrides }) {
    return {
      roots: { project, user, envPaths },
      validate: "agentskills", visibility: "frontmatter", collision: "firstWins",
      catalog: catalog.xml(), ...overrides,
    };
  },
  roots(ctx, { project = [], user = [], envPaths } = {}) {
    return __native.roots({ project, user, envPaths }, ctx);
  },
};
export function __describe(def, path = "") {
  if (typeof def === "function") return { $hook: path };
  if (Array.isArray(def)) return def.map((v, i) => __describe(v, path ? `${path}.${i}` : `${i}`));
  if (def !== null && typeof def === "object") {
    return Object.fromEntries(Object.entries(def).map(([k, v]) => [k, __describe(v, path ? `${path}.${k}` : k)]));
  }
  return def;
}
