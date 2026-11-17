//! Tools and MCP (TECHNICAL 10): toolsets, tokens and the MCP endpoint that
//! exposes mod functions to agents.

use crate::{Caller, Host, HostError, LocalName, api, home};
use axum::Json;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

macro_rules! name_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);
        impl $name {
            pub fn parse(s: &str) -> Result<Self, HostError> {
                if (1..=31).contains(&s.len())
                    && s.as_bytes()[0].is_ascii_lowercase()
                    && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                {
                    Ok(Self(s.into()))
                } else {
                    Err(HostError::BadName {
                        value: s.into(),
                        rule: "use 1 to 31 lowercase letters, digits or hyphens, starting with a letter",
                    })
                }
            }
        }
        impl TryFrom<String> for $name {
            type Error = HostError;
            fn try_from(s: String) -> Result<Self, HostError> { Self::parse(&s) }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
    }
}
name_type!(ToolsetName);
name_type!(Handle);
#[derive(Debug, Clone, Serialize)]
pub struct FunctionInfo {
    pub interface: watane_pkg::InterfaceName,
    pub name: String,
    pub params: Vec<(String, String)>,
    pub result: Option<String>,
    pub docs: Option<String>,
}
// Do not expose bearer secrets through Debug output.
pub struct Token(String);
impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Token").field(&self.id()).finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct TokenId(String);
impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn id(&self) -> TokenId {
        token_id(&blake3::hash(self.0.as_bytes()))
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct TokenInfo {
    pub id: TokenId,
    pub toolset: ToolsetName,
    pub handle: Handle,
}
fn token_id(hash: &blake3::Hash) -> TokenId {
    TokenId(hash.to_hex()[..16].into())
}
#[derive(Debug, Clone)]
struct Binding {
    toolset: ToolsetName,
    handle: Handle,
}
#[derive(Debug, Default)]
pub(crate) struct Tools {
    sets: BTreeMap<ToolsetName, Vec<String>>,
    tokens: BTreeMap<[u8; 32], Binding>,
}
impl Tools {
    pub(crate) fn open(home: &Path) -> Result<Self, HostError> {
        let sets = match std::fs::read(home.join("toolsets.json")) {
            Ok(b) => serde_json::from_slice::<BTreeMap<ToolsetName, Vec<String>>>(&b)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e.into()),
        };
        for (name, entries) in &sets {
            check_toolset(name, entries)?;
        }
        Ok(Self {
            sets,
            tokens: BTreeMap::new(),
        })
    }
}
fn entry_parts(entry: &str) -> Option<(LocalName, &str, Option<&str>)> {
    let (local, tail) = entry.split_once(':')?;
    let local = LocalName::parse(local).ok()?;
    let (interface, function) = match tail.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (tail, None),
    };
    if wit_parser::validate_id(interface).is_err()
        || function.is_some_and(|f| wit_parser::validate_id(f).is_err())
    {
        return None;
    }
    Some((local, interface, function))
}
fn check_toolset(name: &ToolsetName, entries: &[String]) -> Result<(), HostError> {
    if name.0 == "preview" {
        return Err(HostError::BadName {
            value: name.to_string(),
            rule: "preview is reserved",
        });
    }
    for entry in entries {
        if entry_parts(entry).is_none() {
            return Err(HostError::BadToolsetEntry {
                entry: entry.clone(),
                reason: "use <local>:<interface> or <local>:<interface>.<function>".into(),
            });
        }
    }
    Ok(())
}
fn tool_name(local: &LocalName, interface: &str, function: &str) -> Option<String> {
    let name = format!("{local}_{interface}_{function}");
    (name.len() <= 64).then_some(name)
}
struct Tool {
    local: LocalName,
    function: String,
    info: FunctionInfo,
    schema: Value,
}
impl Tool {
    fn description(&self) -> String {
        self.info.docs.clone().unwrap_or_else(|| {
            format!(
                "{}: func({}){}",
                self.info.name,
                self.info
                    .params
                    .iter()
                    .map(|(n, t)| format!("{n}: {t}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                self.info
                    .result
                    .as_ref()
                    .map(|r| format!(" -> {r}"))
                    .unwrap_or_default()
            )
        })
    }
}
impl Host {
    pub fn functions(&self, local: &LocalName) -> Result<Vec<FunctionInfo>, HostError> {
        let catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if !catalog.mods.iter().any(|m| &m.local == local) {
            return Err(HostError::NoSuchMod(local.to_string()));
        }
        Ok(catalog
            .components
            .get(local)
            .map(|c| c.functions())
            .unwrap_or_default())
    }
    pub fn set_toolset(&self, name: &ToolsetName, entries: Vec<String>) -> Result<(), HostError> {
        check_toolset(name, &entries)?;
        let mut tools = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut sets = tools.sets.clone();
        sets.insert(name.clone(), entries);
        home::write_json(&self.home().join("toolsets.json"), &sets)?;
        tools.sets = sets;
        Ok(())
    }
    pub fn toolsets(&self) -> BTreeMap<ToolsetName, Vec<String>> {
        self.tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .sets
            .clone()
    }
    pub fn remove_toolset(&self, name: &ToolsetName) -> Result<(), HostError> {
        let mut tools = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut sets = tools.sets.clone();
        if sets.remove(name).is_none() {
            return Err(HostError::NoSuchToolset(name.to_string()));
        }
        home::write_json(&self.home().join("toolsets.json"), &sets)?;
        tools.sets = sets;
        Ok(())
    }
    pub fn tokens(&self) -> Vec<TokenInfo> {
        let tools = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        tools
            .tokens
            .iter()
            .map(|(hash, binding)| TokenInfo {
                id: token_id(&blake3::Hash::from(*hash)),
                toolset: binding.toolset.clone(),
                handle: binding.handle.clone(),
            })
            .collect()
    }
    pub fn issue_token(&self, toolset: &ToolsetName, handle: &Handle) -> Result<Token, HostError> {
        let mut tools = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if !(toolset.0 == "preview" && self.is_preview()) && !tools.sets.contains_key(toolset) {
            return Err(HostError::NoSuchToolset(toolset.to_string()));
        }
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|e| HostError::Internal(e.to_string()))?;
        // RFC 4648 base64url without padding: 256 random bits give 43 characters.
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut text = String::from("wt_");
        let mut bits = 0u32;
        let mut count = 0;
        for b in bytes {
            bits = (bits << 8) | u32::from(b);
            count += 8;
            while count >= 6 {
                count -= 6;
                text.push(alphabet[((bits >> count) & 63) as usize] as char);
            }
        }
        if count > 0 {
            text.push(alphabet[((bits << (6 - count)) & 63) as usize] as char);
        }
        tools.tokens.insert(
            *blake3::hash(text.as_bytes()).as_bytes(),
            Binding {
                toolset: toolset.clone(),
                handle: handle.clone(),
            },
        );
        Ok(Token(text))
    }
    pub fn end_token(&self, id: &TokenId) -> bool {
        let mut tools = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let key = tools
            .tokens
            .keys()
            .find(|hash| token_id(&blake3::Hash::from(**hash)) == *id)
            .copied();
        key.is_some_and(|key| tools.tokens.remove(&key).is_some())
    }
    fn resolve_tools(&self, name: &ToolsetName) -> BTreeMap<String, Tool> {
        let entries = self
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .sets
            .get(name)
            .cloned()
            .unwrap_or_default();
        let catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut tools = BTreeMap::new();
        for m in catalog
            .mods
            .iter()
            .filter(|m| m.enabled && m.error.is_none())
        {
            let Some(compiled) = catalog.components.get(&m.local) else {
                continue;
            };
            for info in compiled.functions() {
                let interface = info.interface.interface();
                let included = name.0 == "preview" && self.is_preview()
                    || entries.iter().any(|e| {
                        entry_parts(e).is_some_and(|(local, i, f)| {
                            local == m.local && i == interface && f.is_none_or(|f| f == info.name)
                        })
                    });
                if !included {
                    continue;
                }
                let Some(name) = tool_name(&m.local, interface, &info.name) else {
                    continue;
                };
                let function = format!("{interface}.{}", info.name);
                let schema = compiled.schema(&function);
                tools.insert(
                    name,
                    Tool {
                        local: m.local.clone(),
                        function,
                        info,
                        schema,
                    },
                );
            }
        }
        tools
    }
}
fn rpc_error(id: Value, code: i32, message: impl ToString) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message.to_string()}})
}
fn rpc_response(value: Value) -> Response {
    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(value),
    )
        .into_response()
}
fn text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        v => v.to_string(),
    }
}
fn content(value: Value, is_result: bool) -> Value {
    let (value, error) = if is_result {
        if let Some(v) = value.get("err") {
            (v, true)
        } else {
            (value.get("ok").unwrap_or(&Value::Null), false)
        }
    } else {
        (&value, false)
    };
    json!({"content":[{"type":"text","text":text(value)}],"isError":error})
}
pub(crate) async fn respond(host: &Arc<Host>, request: Request<Body>) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return api::error(403, "forbidden", "Origin is forbidden on MCP");
    }
    if request.method() != Method::POST {
        let mut r = api::error(405, "bad_request", "use POST");
        r.headers_mut()
            .insert(header::ALLOW, header::HeaderValue::from_static("POST"));
        return r;
    }
    let binding = {
        let headers = request.headers();
        let token = if headers.get_all(header::AUTHORIZATION).iter().count() == 1 {
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
        } else {
            None
        };
        let tools = host
            .tools
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        token
            .and_then(|t| tools.tokens.get(blake3::hash(t.as_bytes()).as_bytes()))
            .cloned()
    };
    let Some(binding) = binding else {
        let mut r = api::error(401, "forbidden", "a live bearer token is required");
        r.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
        return r;
    };
    if request.uri().path() != format!("/mcp/{}", binding.toolset) {
        return api::error(403, "forbidden", "token belongs to another toolset");
    }
    let types = request.headers().get_all(header::CONTENT_TYPE);
    if types.iter().count() != 1
        || !types
            .iter()
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(';').next())
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("application/json"))
    {
        return api::error(415, "bad_request", "use application/json");
    }
    let bytes = match to_bytes(request.into_body(), 1 << 20).await {
        Ok(b) => b,
        Err(e) => return api::error(413, "bad_request", e),
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return rpc_response(rpc_error(Value::Null, -32600, "invalid request")),
    };
    if !value.is_object()
        || value["jsonrpc"] != "2.0"
        || !value["method"].is_string()
        || value
            .get("id")
            .is_some_and(|id| !(id.is_null() || id.is_string() || id.is_number()))
        || value
            .get("params")
            .is_some_and(|p| !p.is_object() && !p.is_array())
    {
        return rpc_response(rpc_error(Value::Null, -32600, "invalid request"));
    }
    let Some(id) = value.get("id").cloned() else {
        return StatusCode::ACCEPTED.into_response();
    };
    let params = &value["params"];
    let result = match value["method"].as_str().expect("checked method") {
        "initialize" => {
            let version = params["protocolVersion"]
                .as_str()
                .filter(|v| ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"].contains(v))
                .unwrap_or("2025-06-18");
            json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"watane","version":env!("CARGO_PKG_VERSION")}})
        }
        "ping" => json!({}),
        "tools/list" => {
            json!({"tools":host.resolve_tools(&binding.toolset).into_iter().map(|(name,t)|json!({"name":name,"description":t.description(),"inputSchema":t.schema})).collect::<Vec<_>>()})
        }
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                return rpc_response(rpc_error(id, -32602, "tool name is required"));
            };
            let Some(tool) = host.resolve_tools(&binding.toolset).remove(name) else {
                return rpc_response(rpc_error(id, -32602, format!("unknown tool {name}")));
            };
            let is_result = tool
                .info
                .result
                .as_ref()
                .is_some_and(|t| t.starts_with("result<"));
            match host
                .call_async(
                    Caller::Agent(binding.handle),
                    tool.local,
                    tool.function,
                    params.get("arguments").cloned().unwrap_or(json!({})),
                )
                .await
            {
                Ok(value) => content(value, is_result),
                Err(e) => {
                    let code = serde_json::to_value(&e).expect("CallError serializes")["code"]
                        .as_str()
                        .expect("a CallError serializes its code as a string")
                        .to_owned();
                    json!({"content":[{"type":"text","text":format!("{code}: {}",e.message)}],"isError":true})
                }
            }
        }
        _ => return rpc_response(rpc_error(id, -32601, "method not found")),
    };
    rpc_response(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac7_tool_name_limit_is_in_bytes() {
        let local = LocalName::parse("counter").unwrap();
        assert_eq!(
            tool_name(&local, "api", &"x".repeat(52)),
            Some(format!("counter_api_{}", "x".repeat(52)))
        );
        assert_eq!(tool_name(&local, "api", &"x".repeat(53)), None);
        assert_eq!(tool_name(&local, "api", &"é".repeat(27)), None);
    }
}
