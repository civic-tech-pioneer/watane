use super::{Ctx, limits::Limiter};
use crate::{CallError, HostError, Limits};
use std::time::Instant;
use wasmtime::{
    Engine, Store, UpdateDeadline,
    component::{Component, Linker, types::ComponentItem},
};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use watane_pkg::Mod;
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
    let ty = component.component_type();
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
                    return Err(HostError::Internal(format!(
                        "outlet {outlet}.{name} must return nothing"
                    ))
                    .into());
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
pub(super) fn store(
    engine: &Engine,
    state: crate::state::Transaction,
    caller: String,
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
        caller,
        stderr,
        deadline,
        outbox: Vec::new(),
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
