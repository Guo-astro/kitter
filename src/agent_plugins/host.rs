use rquickjs::{
    Context, Ctx, Exception, Function, Module, Object, Persistent, Runtime, Value,
    loader::{BuiltinLoader, BuiltinResolver, ModuleLoader},
    module::{Declarations, Exports, ModuleDef},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    time::{Duration, Instant},
};

use super::{AgentSpec, HookPath, HostContext, RootsDecl, expand_roots};
use crate::effective_skills::{truncate_chars, xml_escape};

#[derive(Clone, Debug)]
pub struct PluginSource {
    pub name: String,
    pub code: Cow<'static, str>,
}
#[derive(Debug)]
pub struct PluginError {
    pub message: String,
    pub stack: Option<String>,
}
impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for PluginError {}
impl PluginError {
    fn message(message: impl ToString) -> Self {
        Self {
            message: message.to_string(),
            stack: None,
        }
    }
    fn caught(ctx: &Ctx<'_>, error: rquickjs::Error) -> Self {
        if error.is_exception() {
            let value = ctx.catch();
            if let Ok(ex) = Exception::from_value(value.clone()) {
                return Self {
                    message: ex.message().unwrap_or_default(),
                    stack: ex.stack(),
                };
            }
            use rquickjs::FromJs;
            return Self::message(
                rquickjs::Coerced::<String>::from_js(ctx, value)
                    .map(|s| s.0)
                    .unwrap_or_else(|_| error.to_string()),
            );
        }
        Self::message(error)
    }
}

pub(crate) fn from_json<'js>(
    ctx: &Ctx<'js>,
    value: &serde_json::Value,
) -> rquickjs::Result<Value<'js>> {
    ctx.json_parse(value.to_string())
}
pub(crate) fn to_json<'js>(
    ctx: &Ctx<'js>,
    value: Value<'js>,
) -> rquickjs::Result<serde_json::Value> {
    let text = ctx
        .json_stringify(value)?
        .ok_or_else(|| rquickjs::Error::new_from_js("undefined", "JSON"))?
        .to_string()?;
    serde_json::from_str(&text)
        .map_err(|e| rquickjs::Error::new_from_js_message("JSON", "value", e.to_string()))
}

struct NativeModule;
impl ModuleDef for NativeModule {
    fn declare(d: &Declarations<'_>) -> rquickjs::Result<()> {
        d.declare("__native")?;
        Ok(())
    }
    fn evaluate<'js>(ctx: &Ctx<'js>, e: &Exports<'js>) -> rquickjs::Result<()> {
        let native = Object::new(ctx.clone())?;
        native.set(
            "xmlEscape",
            Function::new(ctx.clone(), |s: String| xml_escape(&s))?,
        )?;
        native.set(
            "truncate",
            Function::new(ctx.clone(), |s: String, n: usize| truncate_chars(&s, n))?,
        )?;
        native.set(
            "byteLength",
            Function::new(ctx.clone(), |s: String| s.len())?,
        )?;
        native.set(
            "roots",
            Function::new(
                ctx.clone(),
                |c: Ctx<'js>, decl: Value<'js>, host: Object<'js>| {
                    let decl: RootsDecl = serde_json::from_value(to_json(&c, decl)?)
                        .map_err(|e| Exception::throw_type(&c, &e.to_string()))?;
                    let host = HostContext {
                        home: host.get::<_, String>("home")?.into(),
                        cwd: host.get::<_, String>("cwd")?.into(),
                        repo_root: host.get::<_, Option<String>>("repoRoot")?.map(Into::into),
                        is_global: host.get("isGlobal")?,
                    };
                    let roots = serde_json::to_value(expand_roots(&decl, &host))
                        .map_err(|e| Exception::throw_type(&c, &e.to_string()))?;
                    from_json(&c, &roots)
                },
            )?,
        )?;
        e.export("__native", native)?;
        Ok(())
    }
}

fn runtime() -> Runtime {
    let rt = Runtime::new().expect("create plugin runtime");
    rt.set_memory_limit(64 * 1024 * 1024);
    rt.set_loader(
        BuiltinResolver::default()
            .with_module("kitter")
            .with_module("kitter-native"),
        (
            BuiltinLoader::default().with_module("kitter", include_str!("prelude.js")),
            ModuleLoader::default().with_module("kitter-native", NativeModule),
        ),
    );
    rt
}
fn load<'js>(ctx: &Ctx<'js>, source: &PluginSource) -> rquickjs::Result<Value<'js>> {
    let (module, promise) =
        Module::declare(ctx.clone(), source.name.as_bytes(), source.code.as_bytes())?.eval()?;
    promise.finish::<()>()?;
    module.get("default")
}

pub fn load_spec(source: &PluginSource) -> Result<AgentSpec, PluginError> {
    SpecLoader::new().load(source)
}

pub(crate) struct SpecLoader {
    context: Context,
    runtime: Runtime,
}
impl SpecLoader {
    pub(crate) fn new() -> Self {
        let runtime = runtime();
        let context = Context::full(&runtime).expect("create plugin context");
        Self { context, runtime }
    }
    pub(crate) fn load(&self, source: &PluginSource) -> Result<AgentSpec, PluginError> {
        let _interrupt = Interrupt::start(&self.runtime);
        self.context.with(|ctx| {
            let result = (|| {
                let def = load(&ctx, source)?;
                let kitter: Object = Module::import(&ctx, "kitter")?.finish()?;
                let describe: Function = kitter.get("__describe")?;
                to_json(&ctx, describe.call((def,))?)
            })()
            .map_err(|e| PluginError::caught(&ctx, e))?;
            let spec: AgentSpec = serde_json::from_value(result).map_err(PluginError::message)?;
            spec.validate().map_err(PluginError::message)?;
            Ok(spec)
        })
    }
}

pub struct JsHost {
    // Fields drop in declaration order: all Persistent values precede the runtime.
    modules: HashMap<String, Persistent<Object<'static>>>,
    host: Persistent<Object<'static>>,
    context: Context,
    runtime: Runtime,
}
impl JsHost {
    pub fn new(host: HostContext) -> Self {
        let runtime = runtime();
        let context = Context::full(&runtime).expect("create plugin context");
        let host =
            context.with(|ctx| Persistent::save(&ctx, host.object(&ctx).expect("create host API")));
        Self {
            modules: HashMap::new(),
            host,
            context,
            runtime,
        }
    }
    pub fn call(
        &mut self,
        plugin: &PluginSource,
        hook: &HookPath,
        input: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, PluginError> {
        let _interrupt = Interrupt::start(&self.runtime);
        self.context.with(|ctx| {
            let result = (|| {
                if !self.modules.contains_key(&plugin.name) {
                    let def: Object = Object::from_value(load(&ctx, plugin)?)?;
                    self.modules
                        .insert(plugin.name.clone(), Persistent::save(&ctx, def));
                }
                let mut value = self.modules[&plugin.name]
                    .clone()
                    .restore(&ctx)?
                    .into_value();
                for part in hook.0.split('.') {
                    value = Object::from_value(value)?.get(part)?;
                }
                let function = Function::from_value(value)?;
                let host = self.host.clone().restore(&ctx)?;
                let output = if let Some(input) = input {
                    function.call((from_json(&ctx, &input)?, host))?
                } else {
                    function.call((host,))?
                };
                to_json(&ctx, output)
            })();
            result.map_err(|e| PluginError::caught(&ctx, e))
        })
    }
}

/// Interrupts plugin code that runs longer than two seconds until dropped.
struct Interrupt<'a>(&'a Runtime);
impl<'a> Interrupt<'a> {
    fn start(runtime: &'a Runtime) -> Self {
        let deadline = Instant::now() + Duration::from_secs(2);
        runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
        Self(runtime)
    }
}
impl Drop for Interrupt<'_> {
    fn drop(&mut self) {
        self.0.set_interrupt_handler(None);
    }
}
