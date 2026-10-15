//! Agent plugins: every Agent Kitter knows about is described by one JS file.
//! See `.plans/agent-plugins.md` for the design.

mod template;

pub use template::expand_path;
