//! The `system/init` check (TECHNICAL 19.3, The control protocol, step 3): the line must
//! match the launch in tools, MCP servers, agents, permission mode and plugins.

use super::launch::Launch;
use serde_json::Value;

/// Claude Code's own agents, which its `system/init` lists beside the launch's `subagents`:
/// the built-in `agentType`s of Claude Code 2.1.330. Any other name is a difference, so a
/// user's or a project's agent fails closed, and so does a built-in agent added later until
/// it is named here.
const BUILT_IN_AGENTS: [&str; 9] = [
    "general-purpose",
    "statusline-setup",
    "Explore",
    "Plan",
    "claude-code-guide",
    "claude",
    "fork",
    "worker",
    "comment-thread-analyst",
];

/// The names in a list of strings or of objects with a `name`.
fn names(v: Option<&Value>) -> Option<Vec<String>> {
    let Some(v) = v else {
        return Some(vec![]);
    };
    v.as_array()?
        .iter()
        .map(|i| match i {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => o.get("name")?.as_str().map(str::to_owned),
            _ => None,
        })
        .collect()
}

/// `Ok` when the line matches; else the field that differs, named as `system/init` names it.
/// Skills are not compared. A field that is absent holds nothing; one that is not a list of
/// names differs. Only what the launch did not ask for is a difference: a plugin that is
/// `watane-run` or one of Claude Code's own `cc-plugin-*` is the launch's.
pub(crate) fn check(launch: &Launch, init: &Value) -> Result<(), &'static str> {
    let extra = |field: &'static str, allowed: &dyn Fn(&str) -> bool| match names(init.get(field)) {
        Some(list) if list.iter().all(|n| allowed(n)) => Ok(()),
        _ => Err(field),
    };
    extra("tools", &|n| launch.tools.iter().any(|t| t == n))?;
    extra("mcp_servers", &|_| false)?;
    extra("agents", &|n| {
        BUILT_IN_AGENTS.contains(&n) || launch.agents.iter().any(|a| a == n)
    })?;
    if init.get("permissionMode").and_then(Value::as_str) != Some(launch.init_mode) {
        return Err("permissionMode");
    }
    extra("plugins", &|n| {
        n == "watane-run" || n.starts_with("cc-plugin-")
    })?;
    Ok(())
}
