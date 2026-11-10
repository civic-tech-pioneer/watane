use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_call_refuses_wrong_method_origin_type_size_and_body() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(&fixture("counter")).await.status(), 200);
    assert_eq!(preview.install(&fixture("hello")).await.status(), 200);
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    let url = format!("{origin}/call");
    let client = preview.client("counter.localhost", "127.0.0.1");
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(
        response
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok()),
        Some("POST")
    );
    pages::call_error(response, 405, "bad_request").await;
    for wrong_origin in [
        None,
        Some(format!("http://hello.localhost:{}", preview.ready.port)),
        Some(format!("http://localhost:{}", preview.ready.port)),
        Some(format!("{origin}/")),
        Some(origin.replace("http:", "https:")),
        Some("null".into()),
        Some(format!("{origin}.evil.example")),
    ] {
        let mut request = client.post(&url).json(&json!({"fn":"api.add","args":null}));
        if let Some(wrong_origin) = wrong_origin {
            request = request.header("Origin", wrong_origin);
        }
        pages::call_error(request.send().await.unwrap(), 403, "forbidden").await;
    }
    for content_type in [None, Some("text/plain")] {
        let mut request = client
            .post(&url)
            .header("Origin", &origin)
            .body(r#"{"fn":"api.add","args":null}"#);
        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }
        pages::call_error(request.send().await.unwrap(), 415, "bad_request").await;
    }
    let response = client
        .post(&url)
        .header("Origin", &origin)
        .header(
            "Origin",
            format!("http://hello.localhost:{}", preview.ready.port),
        )
        .json(&json!({"fn":"api.add","args":null}))
        .send()
        .await
        .unwrap();
    pages::call_error(response, 403, "forbidden").await;
    let response = client
        .post(&url)
        .header("Origin", &origin)
        .header("Content-Type", "application/json")
        .body(" ".repeat(2 << 20))
        .send()
        .await
        .unwrap();
    pages::call_error(response, 413, "bad_request").await;
    for body in [
        r#"{"fn":"api.add","args":null,"caller":"agent:evil"}"#,
        r#"{"fn":42,"args":null}"#,
        r#"{"fn":"api.add","args":42}"#,
        r#"{"fn":"api.add","args":true}"#,
        r#"{"fn":"api.add","args":"bad"}"#,
        r#"{"fn":"api.add"}"#,
        r#"{"args":null}"#,
        "broken",
        "[]",
        "null",
        r#"["api.add",null]"#,
    ] {
        let response = client
            .post(&url)
            .header("Origin", &origin)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        pages::call_error(response, 400, "bad_request").await;
    }
    // Refused requests must not run the function or change its state.
    let response = client
        .post(&url)
        .header("Origin", &origin)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(r#"{"fn":"api.get","args":null}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    pages::headers(&response);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"value":0}));
}
