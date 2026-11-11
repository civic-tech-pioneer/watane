use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Limits {
    pub fuel: u64,
    pub memory: usize,
    pub time: Duration,
    pub depth: u32,
    pub hops: u8,
    pub deliveries: usize,
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub call_write_bytes: usize,
    pub quota_bytes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            fuel: 5_000_000_000,
            memory: 64 << 20,
            time: Duration::from_secs(10),
            depth: 8,
            hops: 8,
            deliveries: 256,
            key_bytes: 1024,
            value_bytes: 4 << 20,
            call_write_bytes: 16 << 20,
            quota_bytes: 256 << 20,
        }
    }
}

use crate::{CallError, Limit};
use wasmtime::ResourceLimiter;
pub(crate) struct Limiter {
    maximum: usize,
    allocated: usize,
}
impl Limiter {
    pub fn new(maximum: usize) -> Self {
        Self {
            maximum,
            allocated: 0,
        }
    }
}
impl ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let total = self
            .allocated
            .saturating_add(desired.saturating_sub(current));
        if total > self.maximum || maximum.is_some_and(|max| desired > max) {
            return Err(CallError::limit(Limit::Memory, "linear memory limit exceeded").into());
        }
        self.allocated = total;
        Ok(true)
    }
    fn memory_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        Err(CallError::limit(Limit::Memory, format!("memory allocation failed: {error}")).into())
    }
    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > 100_000 || maximum.is_some_and(|max| desired > max) {
            return Err(CallError::limit(Limit::Memory, "table element limit exceeded").into());
        }
        Ok(true)
    }
    fn instances(&self) -> usize {
        64
    }
    fn tables(&self) -> usize {
        64
    }
    fn memories(&self) -> usize {
        16
    }
}
pub(crate) fn classify(
    error: wasmtime::Error,
    stderr: &wasmtime_wasi::p2::pipe::MemoryOutputPipe,
) -> CallError {
    if let Some(cause) = error
        .chain()
        .filter_map(|e| e.downcast_ref::<CallError>())
        .last()
    {
        return cause.clone();
    }
    if error.downcast_ref::<wasmtime::Trap>() == Some(&wasmtime::Trap::OutOfFuel) {
        return CallError::limit(Limit::Fuel, "call exhausted its fuel");
    }
    let bytes = stderr.contents();
    let tail = &bytes[bytes.len().saturating_sub(4096)..];
    CallError::new(
        crate::CallCode::Trap,
        format!("{error:#}\n{}", String::from_utf8_lossy(tail)),
    )
}
