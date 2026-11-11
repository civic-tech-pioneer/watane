use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use watane_host::{Host, Limits, LocalName};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
fn setup() -> (tempfile::TempDir, Arc<Host>) {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Limits::default()).unwrap();
    host.install_folders(&[fixture("counter"), fixture("echo"), fixture("hello")], 0)
        .unwrap();
    (home, host)
}
#[test]
fn ac2_functions_expose_callable_metadata() {
    let (_home, host) = setup();
    let functions = host
        .functions(&LocalName::parse("counter").unwrap())
        .unwrap();
    assert_eq!(
        serde_json::to_value(functions).unwrap(),
        json!([
        {"interface":"mods:counter/api@0.1.0","name":"add","params":[],"result":"u32","docs":"Increment the counter."},
        {"interface":"mods:counter/api@0.1.0","name":"add-then-fail","params":[],"result":null,"docs":null},
        {"interface":"mods:counter/api@0.1.0","name":"get","params":[],"result":"u32","docs":null},
        {"interface":"mods:counter/api@0.1.0","name":"who","params":[],"result":"string","docs":null}
        ])
    );
    assert!(
        host.functions(&LocalName::parse("hello").unwrap())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        host.functions(&LocalName::parse("absent").unwrap())
            .unwrap_err()
            .code(),
        "no_mod"
    );
}
fn name(s: &str) -> watane_host::ToolsetName {
    watane_host::ToolsetName::parse(s).unwrap()
}
fn token(host: &Host, t: &str) -> watane_host::Token {
    host.issue_token(
        &name(t),
        &watane_host::Handle::parse("alice-agent").unwrap(),
    )
    .unwrap()
}
async fn rpc(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    method: &str,
    params: Value,
) -> Value {
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}
async fn server(host: &Arc<Host>) -> (watane_host::Served, reqwest::Client, String) {
    let served = host.serve(0).await.unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .resolve("localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("counter.localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let url = served.mcp_url(&name("t"));
    (served, client, url)
}
#[tokio::test]
async fn ac1_tools_list_docs_schemas_and_selection() {
    let (_home, host) = setup();
    host.set_toolset(&name("t"), vec!["counter:api".into()])
        .unwrap();
    let token = token(&host, "t");
    let (served, client, url) = server(&host).await;
    let result = rpc(&client, &url, token.as_str(), "tools/list", json!({})).await;
    let tools = result["result"]["tools"].as_array().unwrap();
    assert_eq!(
        tools.iter().map(|t| t["name"].clone()).collect::<Vec<_>>(),
        vec![
            json!("counter_api_add"),
            json!("counter_api_add-then-fail"),
            json!("counter_api_get"),
            json!("counter_api_who")
        ]
    );
    for tool in tools {
        assert_eq!(
            tool["inputSchema"],
            json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
        );
        assert!(tool["description"].is_string());
    }
    assert_eq!(tools[0]["description"], "Increment the counter.");
    assert_eq!(tools[2]["description"], "get: func() -> u32");
    host.set_toolset(
        &name("t"),
        vec![
            "counter:api.get".into(),
            "counter:api.get".into(),
            "echo:api.id-u32".into(),
            "echo:api.id-option".into(),
        ],
    )
    .unwrap();
    let result = rpc(&client, &url, token.as_str(), "tools/list", json!({})).await;
    let tools = result["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 3);
    assert_eq!(tools[0]["name"], "counter_api_get");
    assert_eq!(
        tools[2]["inputSchema"],
        json!({"type":"object","properties":{"x":{"type":"integer","minimum":0,"maximum":4294967295u32}},"required":["x"],"additionalProperties":false})
    );
    assert_eq!(
        tools[1]["inputSchema"],
        json!({"type":"object","properties":{"x":{"anyOf":[{"type":"null"},{"type":"integer","minimum":0,"maximum":4294967295u32}]}},"required":[],"additionalProperties":false})
    );
    served.shutdown().await;
}
#[tokio::test]
async fn ac3_transport_guards_and_token_revocation() {
    let (_home, host) = setup();
    host.set_toolset(&name("t"), vec![]).unwrap();
    host.set_toolset(&name("other"), vec![]).unwrap();
    let active = token(&host, "t");
    let other = token(&host, "other");
    let ended = token(&host, "t");
    assert!(host.end_token(&ended.id()));
    assert!(!host.end_token(&ended.id()));
    let (served, client, url) = server(&host).await;
    for supplied in [None, Some(ended.as_str())] {
        let mut r = client.post(&url);
        if let Some(t) = supplied {
            r = r.bearer_auth(t);
        }
        let r = r.json(&json!({})).send().await.unwrap();
        assert_eq!(r.status(), 401);
        assert_eq!(r.headers()["www-authenticate"], "Bearer");
    }
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(other.as_str())
            .json(&json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for origin in ["", "null", "http://localhost"] {
        assert_eq!(
            client
                .post(&url)
                .header("Origin", origin)
                .bearer_auth(active.as_str())
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    }
    for method in [reqwest::Method::GET, reqwest::Method::DELETE] {
        let r = client
            .request(method, &url)
            .bearer_auth(active.as_str())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 405);
        assert_eq!(r.headers()["allow"], "POST");
    }
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(active.as_str())
            .header("content-type", "text/plain")
            .body("{}")
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(active.as_str())
            .header("content-type", "application/json")
            .body(" ".repeat(1048577))
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    // MCP accepts bodies above the local API's separate 64 KiB limit.
    let r = client
        .post(&url)
        .bearer_auth(active.as_str())
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"ping","padding":" ".repeat(70_000)}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.json::<Value>().await.unwrap(),
        json!({"jsonrpc":"2.0","id":1,"result":{}})
    );
    let r = client
        .post(&url)
        .bearer_auth(active.as_str())
        .json(&json!([]))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.json::<Value>().await.unwrap()["error"]["code"], -32600);
    let r = client
        .post(&url)
        .bearer_auth(active.as_str())
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);
    assert_eq!(r.bytes().await.unwrap().len(), 0);
    served.shutdown().await;
}
#[tokio::test]
async fn ac4_protocol_methods_and_versions() {
    let (_home, host) = setup();
    host.set_toolset(&name("t"), vec![]).unwrap();
    let token = token(&host, "t");
    let (served, client, url) = server(&host).await;
    for (params, v) in [
        (json!({"protocolVersion":"2025-03-26"}), "2025-03-26"),
        (json!({"protocolVersion":"1999-01-01"}), "2025-06-18"),
        (json!({}), "2025-06-18"),
    ] {
        assert_eq!(
            rpc(&client, &url, token.as_str(), "initialize", params).await,
            json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":v,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"watane","version":"0.1.0"}}})
        );
    }
    assert_eq!(
        rpc(&client, &url, token.as_str(), "ping", json!({})).await,
        json!({"jsonrpc":"2.0","id":1,"result":{}})
    );
    assert_eq!(
        rpc(&client, &url, token.as_str(), "absent", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        rpc(
            &client,
            &url,
            token.as_str(),
            "tools/call",
            json!({"name":"counter_api_get"})
        )
        .await,
        json!({"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"unknown tool counter_api_get"}})
    );
    served.shutdown().await;
}
#[tokio::test]
async fn ac5_agent_calls_publish_changes_and_render_results() {
    let (_home, host) = setup();
    host.set_toolset(&name("t"), vec!["counter:api".into(), "echo:api".into()])
        .unwrap();
    let token = token(&host, "t");
    let (served, client, url) = server(&host).await;
    let mut stream = client
        .get(format!("http://counter.localhost:{}/events", served.port))
        .send()
        .await
        .unwrap();
    assert_eq!(
        rpc(
            &client,
            &url,
            token.as_str(),
            "tools/call",
            json!({"name":"counter_api_add"})
        )
        .await["result"],
        json!({"content":[{"type":"text","text":"1"}],"isError":false})
    );
    let mut bytes = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !String::from_utf8_lossy(&bytes).contains("data: {\"type\":\"changed\"}\n\n") {
            bytes.extend(stream.chunk().await.unwrap().unwrap());
        }
    })
    .await
    .unwrap();
    assert_eq!(
        rpc(
            &client,
            &url,
            token.as_str(),
            "tools/call",
            json!({"name":"counter_api_who"})
        )
        .await["result"],
        json!({"content":[{"type":"text","text":"agent:alice-agent"}],"isError":false})
    );
    let r = rpc(
        &client,
        &url,
        token.as_str(),
        "tools/call",
        json!({"name":"counter_api_add-then-fail"}),
    )
    .await;
    assert_eq!(r["result"]["isError"], true);
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("trap: ")
    );
    for (function, x, text, error) in [
        ("id-result", json!({"ok":42}), "42", false),
        ("id-result", json!({"err":"bad"}), "bad", true),
        ("id-string", json!("hello"), "hello", false),
        ("id-unit-result", json!({"ok":null}), "", false),
    ] {
        assert_eq!(
            rpc(
                &client,
                &url,
                token.as_str(),
                "tools/call",
                json!({"name":format!("echo_api_{function}"),"arguments":{"x":x}})
            )
            .await["result"],
            json!({"content":[{"type":"text","text":text}],"isError":error})
        );
    }
    drop(stream);
    served.shutdown().await;
}
#[test]
fn ac7_toolset_grammar_and_persistence() {
    let (home, host) = setup();
    assert_eq!(
        host.set_toolset(&name("preview"), vec![])
            .unwrap_err()
            .code(),
        "bad_name"
    );
    for bad in [
        "",
        "Counter",
        "1t",
        "t_x",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert_eq!(
            watane_host::ToolsetName::parse(bad).unwrap_err().code(),
            "bad_name"
        );
        assert!(serde_json::from_value::<watane_host::Handle>(json!(bad)).is_err());
    }
    for bad in [
        "Counter:api",
        "counter",
        "counter:api.",
        "counter:api.add.x",
    ] {
        assert_eq!(
            host.set_toolset(&name("t"), vec![bad.into()])
                .unwrap_err()
                .code(),
            "bad_toolset_entry"
        );
    }
    // WIT permits uppercase identifier segments and digits after a hyphen.
    host.set_toolset(
        &name("t"),
        vec!["counter:api.UPPER".into(), "counter:api.read-2".into()],
    )
    .unwrap();
    host.set_toolset(&name("t"), vec!["counter:api".into()])
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(home.path().join("toolsets.json")).unwrap())
            .unwrap(),
        json!({"t":["counter:api"]})
    );
    drop(host);
    let reopened = Host::open(home.path(), Limits::default()).unwrap();
    assert!(
        reopened
            .issue_token(&name("t"), &watane_host::Handle::parse("alice").unwrap())
            .is_ok()
    );
}
