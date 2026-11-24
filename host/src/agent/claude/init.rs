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
    // The one MCP server is the host's, with the launch's handle (19.3).
    extra("mcp_servers", &|n| {
        launch.agent_handle.is_some() && n == "watane"
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::claude::launch::Kind;
    use serde_json::json;

    fn launch(extra: &str) -> Launch {
        let doc = format!(r#"{{"version":1,"workspace":"scratch"{extra}}}"#);
        Launch::parse(&doc, Kind::Code).unwrap()
    }
    fn line(servers: Value) -> Value {
        json!({"tools": [], "mcp_servers": servers, "agents": [], "permissionMode": "default",
               "plugins": [{"name": "watane-run"}]})
    }

    /// 19.3 step 3: with a handle the host's one MCP server, `watane`, is the launch's; any
    /// other server, and any server without a handle, is a difference.
    #[test]
    fn ac5_only_the_watane_server_of_a_handle_matches() {
        let handle = launch(&format!(r#","agent-handle":"{}""#, "a".repeat(32)));
        let plain = launch("");
        let one = line(json!([{"name": "watane", "status": "connected"}]));
        assert_eq!(check(&handle, &one), Ok(()));
        assert_eq!(check(&handle, &line(json!([]))), Ok(()));
        assert_eq!(check(&handle, &line(json!(["other"]))), Err("mcp_servers"));
        assert_eq!(
            check(&handle, &line(json!(["watane", "x"]))),
            Err("mcp_servers")
        );
        assert_eq!(check(&plain, &one), Err("mcp_servers"));
        assert_eq!(check(&plain, &line(json!([]))), Ok(()));
    }
}
