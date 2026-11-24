//! Host interfaces and the linker (TECHNICAL 6.4): links WASI, `state`,
//! `caller`, `manage`, outlets, providers and fakes into a mod's instance.

use super::Ctx;
use super::limits::Limiter;
use crate::{CallError, HostError, Limits};
use std::time::Instant;
use wasmtime::component::types::ComponentItem;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, UpdateDeadline};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use watane_pkg::Mod;

/// The imports of step 11 (19.1) that a mod may hold, besides `streams` and `timers`.
pub(super) const AGENT_IMPORTS: [&str; 9] = [
    "watane:host/streams@0.1.0",
    "watane:host/timers@0.1.0",
    "watane:host/claude-types@0.1.0",
    "watane:host/claude-code@0.1.0",
    "watane:host/claude-code-shell@0.1.0",
    "watane:host/claude-session@0.1.0",
    "watane:host/https-stream@0.1.0",
    "watane:host/agent-tools@0.1.0",
    "watane:host/host-info@0.1.0",
];
impl WasiView for Ctx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}
pub(super) fn linker(
    engine: &Engine,
    component: &Component,
    m: &Mod,
) -> wasmtime::Result<Linker<Ctx>> {
    let mut linker = Linker::<Ctx>::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    let mut state = linker.instance("watane:host/state@0.1.0")?;
    state.func_wrap(
        "get",
        |mut ctx: wasmtime::StoreContextMut<'_, Ctx>, (key,): (String,)| {
            ctx.data().invocation.storage()?;
            Ok((ctx.data_mut().state.get(&key)?,))
        },
    )?;
    state.func_wrap(
        "put",
        |mut ctx: wasmtime::StoreContextMut<'_, Ctx>, (key, value): (String, Vec<u8>)| {
            ctx.data().invocation.storage()?;
            ctx.data_mut().state.put(key, value)?;
            Ok(())
        },
    )?;
    state.func_wrap(
        "delete",
        |mut ctx: wasmtime::StoreContextMut<'_, Ctx>, (key,): (String,)| {
            ctx.data().invocation.storage()?;
            ctx.data_mut().state.delete(key);
            Ok(())
        },
    )?;
    linker
        .instance("watane:host/caller@0.1.0")?
        .func_wrap("who", |ctx: wasmtime::StoreContextMut<'_, Ctx>, (): ()| {
            Ok((ctx.data().caller.clone(),))
        })?;
    linker.instance("watane:host/caller@0.1.0")?.func_wrap(
        "identity",
        |ctx: wasmtime::StoreContextMut<'_, Ctx>, (): ()| Ok((ctx.data().identity.clone(),)),
    )?;
    let ty = component.component_type();
    link_agent(&mut linker, engine, component)?;
    if m.world().manage
        && let Some(import) = ty.get_import(engine, crate::manage::MANAGE)
    {
        let ComponentItem::ComponentInstance(instance) = import.ty else {
            return Err(HostError::Internal("manage is not an interface".into()).into());
        };
        let mut interface = linker.instance(crate::manage::MANAGE)?;
        for (name, item) in instance.exports(engine) {
            if !matches!(item.ty, ComponentItem::ComponentFunc(_)) {
                continue;
            }
            let function = name.to_owned();
            interface.func_new(name, move |mut ctx, ty, values, results| {
                let args = ty
                    .params()
                    .zip(values)
                    .map(|((name, _), value)| Ok((name.to_owned(), super::json::encode(value)?)))
                    .collect::<Result<serde_json::Map<_, _>, CallError>>()?;
                let invocation = ctx.data().invocation.clone();
                let value =
                    invocation.manage(&mut ctx.data_mut().manages, &function, args.into())?;
                for (ty, result) in ty.results().zip(results) {
                    *result = super::json::decode(&ty, &value, "$").map_err(|error| {
                        CallError::new(
                            crate::CallCode::Trap,
                            format!("watane:host/manage.{function}: {error}"),
                        )
                    })?;
                }
                Ok(())
            })?;
        }
    }
    for outlet in &m.world().events {
        let Some(import) = ty.get_import(engine, &outlet.to_string()) else {
            continue;
        };
        let ComponentItem::ComponentInstance(instance) = import.ty else {
            return Err(HostError::Internal("outlet is not an interface".into()).into());
        };
        let mut interface = linker.instance(&outlet.to_string())?;
        for (name, item) in instance.exports(engine) {
            if let ComponentItem::ComponentFunc(func) = item.ty {
                if func.results().len() != 0 {
                    // A collect (9.4). The host checks the compiled type
                    // itself (5.3): one result, a list.
                    let mut results = func.results();
                    if !matches!(
                        (results.next(), results.next()),
                        (Some(wasmtime::component::types::Type::List(_)), None)
                    ) {
                        return Err(HostError::Internal(format!(
                            "outlet {outlet}.{name} must return nothing or a list"
                        ))
                        .into());
                    }
                    let collect_interface = outlet.clone();
                    let collect_function = name.to_owned();
                    interface.func_new(name, move |mut ctx, ty, values, results| {
                        let args = ty
                            .params()
                            .zip(values)
                            .map(|((name, _), value)| {
                                Ok((name.to_owned(), super::json::encode(value)?))
                            })
                            .collect::<Result<serde_json::Map<_, _>, CallError>>()?;
                        let invocation = ctx.data().invocation.clone();
                        let fuel = ctx.get_fuel()?;
                        let (value, fuel) = invocation.collect(
                            &collect_interface,
                            &collect_function,
                            args.into(),
                            fuel,
                        )?;
                        ctx.set_fuel(fuel)?;
                        for (ty, result) in ty.results().zip(results) {
                            *result = super::json::decode(&ty, &value, "$").map_err(|error| {
                                CallError::new(
                                    crate::CallCode::Trap,
                                    format!("{collect_interface}.{collect_function}: {error}"),
                                )
                            })?;
                        }
                        Ok(())
                    })?;
                    continue;
                }
                let event_interface = outlet.to_string();
                let event_function = name.to_owned();
                interface.func_new(name, move |mut ctx, ty, values, _| {
                    let args = ty
                        .params()
                        .zip(values)
                        .map(|((name, _), value)| {
                            Ok((name.to_owned(), super::json::encode(value)?))
                        })
                        .collect::<Result<serde_json::Map<_, _>, CallError>>()?;
                    ctx.data_mut().outbox.push(crate::bus::Emission {
                        interface: event_interface.clone(),
                        function: event_function.clone(),
                        args: args.into(),
                    });
                    Ok(())
                })?;
            }
        }
    }
    for import in &m.world().calls {
        let Some(item) = ty.get_import(engine, &import.to_string()) else {
            continue;
        };
        let ComponentItem::ComponentInstance(instance) = item.ty else {
            return Err(HostError::Internal("call import is not an interface".into()).into());
        };
        let mut interface = linker.instance(&import.to_string())?;
        for (name, item) in instance.exports(engine) {
            if let ComponentItem::ComponentFunc(_) = item.ty {
                let import = import.clone();
                let name_owned = name.to_owned();
                interface.func_new(name, move |mut ctx, ty, values, results| {
                    let args = ty
                        .params()
                        .zip(values)
                        .map(|((name, _), value)| {
                            Ok((name.to_owned(), super::json::encode(value)?))
                        })
                        .collect::<Result<serde_json::Map<_, _>, CallError>>()?;
                    let invocation = ctx.data().invocation.clone();
                    let fuel = ctx.get_fuel()?;
                    let (value, fuel) =
                        invocation.nested(&import, &name_owned, args.into(), fuel)?;
                    ctx.set_fuel(fuel)?;
                    let function_name = format!(
                        "{}:{}/{}.{name_owned}",
                        import.namespace(),
                        import.package(),
                        import.interface()
                    );
                    if ty.results().len() == 0 && !value.is_null() {
                        return Err(CallError::new(
                            crate::CallCode::Trap,
                            format!("{function_name}: expected null result"),
                        )
                        .into());
                    }
                    for (ty, result) in ty.results().zip(results) {
                        *result = super::json::decode(&ty, &value, "$").map_err(|error| {
                            CallError::new(
                                crate::CallCode::Trap,
                                format!("{function_name}: {error}"),
                            )
                        })?;
                    }
                    Ok(())
                })?;
            }
        }
    }
    Ok(linker)
}
/// The WIT `stream-error` (19.1), for the typed `streams.push`.
#[derive(Clone, Copy, wasmtime::component::ComponentType, wasmtime::component::Lower)]
#[component(enum)]
#[repr(u8)]
enum PushError {
    #[component(name = "not-yours")]
    NotYours,
    #[component(name = "unknown-stream")]
    UnknownStream,
    #[component(name = "full")]
    Full,
    #[component(name = "too-large")]
    TooLarge,
}

/// `streams` and `timers` (19.2, 19.6) and the stubs of the interfaces that A2 to A4 build.
fn link_agent(
    linker: &mut Linker<Ctx>,
    engine: &Engine,
    component: &Component,
) -> wasmtime::Result<()> {
    let ty = component.component_type();
    for name in AGENT_IMPORTS {
        let Some(import) = ty.get_import(engine, name) else {
            continue;
        };
        let ComponentItem::ComponentInstance(instance) = import.ty else {
            return Err(HostError::Internal(format!("{name} is not an interface")).into());
        };
        let short = name
            .trim_start_matches("watane:host/")
            .trim_end_matches("@0.1.0")
            .to_owned();
        let mut interface = linker.instance(name)?;
        if short == "streams" {
            // A typed function: a `list<u8>` of 4 MiB lifted as `Val`s would use up the guest's
            // hostcall fuel (a `Val` per byte).
            interface.func_wrap(
                "push",
                |ctx: wasmtime::StoreContextMut<'_, Ctx>, (id, body): (String, Vec<u8>)| {
                    let invocation = ctx.data().invocation.clone();
                    Ok((match crate::agent::push(&invocation, &id, body) {
                        Ok(()) => Ok(()),
                        Err(crate::agent::streams::StreamError::NotYours) => {
                            Err(PushError::NotYours)
                        }
                        Err(crate::agent::streams::StreamError::UnknownStream) => {
                            Err(PushError::UnknownStream)
                        }
                        Err(crate::agent::streams::StreamError::Full) => Err(PushError::Full),
                        Err(crate::agent::streams::StreamError::TooLarge) => {
                            Err(PushError::TooLarge)
                        }
                    },))
                },
            )?;
        }
        for (function, item) in instance.exports(engine) {
            if !matches!(item.ty, ComponentItem::ComponentFunc(_))
                || (short == "streams" && function == "push")
            {
                continue;
            }
            let (short, function_name) = (short.clone(), function.to_owned());
            interface.func_new(function, move |ctx, ty, values, results| {
                let invocation = ctx.data().invocation.clone();
                let value = match short.as_str() {
                    "streams" | "timers" | "claude-session" | "agent-tools" | "https-stream" => {
                        let who = ctx.data().who.clone();
                        crate::agent::call(&invocation, &who, &short, &function_name, values)?
                    }
                    "host-info" => {
                        // The harness fake returns 1 (19.1).
                        let host = &invocation.chain.host;
                        serde_json::json!(if invocation.chain.fakes.is_some() {
                            1
                        } else {
                            host.generation
                        })
                    }
                    "claude-code" | "claude-code-shell" => {
                        let grant = if short == "claude-code" {
                            watane_pkg::Grant::ClaudeCode
                        } else {
                            watane_pkg::Grant::ClaudeCodeShell
                        };
                        invocation.granted(&grant, &short)?;
                        let who = ctx.data().who.clone();
                        crate::agent::call(&invocation, &who, &short, &function_name, values)?
                    }
                    other => {
                        // An interface with no function of the host: a clear error at the call.
                        return Err(CallError::internal(format!(
                            "watane:host/{other}.{function_name} is not available in this host yet"
                        ))
                        .into());
                    }
                };
                for (ty, result) in ty.results().zip(results) {
                    *result = super::json::decode(&ty, &value, "$").map_err(|error| {
                        CallError::new(
                            crate::CallCode::Trap,
                            format!("watane:host/{short}.{function_name}: {error}"),
                        )
                    })?;
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}
pub(super) fn store(
    engine: &Engine,
    state: crate::state::Transaction,
    (who, identity): (super::call::Who, String),
    limits: &Limits,
    deadline: Instant,
    invocation: super::call::Invocation,
) -> Result<Store<Ctx>, CallError> {
    let stderr = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(64 << 10);
    let mut builder = WasiCtx::builder();
    builder
        .stderr(stderr.clone())
        .allow_tcp(false)
        .allow_udp(false)
        .allow_ip_name_lookup(false);
    let ctx = Ctx {
        wasi: builder.build(),
        table: wasmtime::component::ResourceTable::new(),
        limiter: Limiter::new(limits.memory),
        state,
        caller: who.who(),
        who,
        identity,
        stderr,
        deadline,
        outbox: Vec::new(),
        manages: Vec::new(),
        invocation,
    };
    let mut store = Store::new(engine, ctx);
    store.limiter(|ctx| &mut ctx.limiter);
    store.set_fuel(limits.fuel).map_err(CallError::internal)?;
    store.set_epoch_deadline(1);
    store.epoch_deadline_callback(|ctx| {
        if Instant::now() >= ctx.data().deadline {
            return Err(CallError::limit(crate::Limit::Time, "call exceeded wall time").into());
        }
        Ok(UpdateDeadline::Continue(1))
    });
    Ok(store)
}
