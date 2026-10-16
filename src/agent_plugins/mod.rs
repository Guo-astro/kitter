//! Agent plugins: every Agent Kitter knows about is described by one JS file.
//! See `.plans/agent-plugins.md` for the design.

mod builtin;
mod ctx;
mod host;
mod id;
mod registry;
mod spec;
mod template;

pub use builtin::BUILTIN_PLUGINS;
pub use ctx::{HostContext, NativeFn};
pub use host::{JsHost, PluginError, PluginSource, load_spec};
pub use id::{AgentId, InstallTarget};
pub use registry::{
    AgentDef, AgentIcon, AgentRegistry, InstallDef, PathValue, PluginDiagnostic, PluginOrigin,
    plugin_dir,
};
pub use spec::*;
pub use template::expand_path;

#[cfg(test)]
mod tests;
