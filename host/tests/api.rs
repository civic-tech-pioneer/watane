use super::*;

// Python struct.pack('<Q', size) framed the fixture files independently;
// a standalone BLAKE3 invocation produced this literal.
const HELLO_PACKAGE: &str = "3bf89275fdfc74838367648fcda2df616528442cf1f2abbe66653cb6d0d0a8f5";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_status_and_api_guards() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}status", preview.ready.api);
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "watane":1,"mode":"preview","home":home.path().canonicalize().unwrap(),
            "port":preview.ready.port,"app":false
        })
    );
    for origin in ["null", "", "http://hello.localhost", "http://localhost"] {
        let response = client
            .get(&url)
            .header("Origin", origin)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "forbidden"
        );
    }
    for host in [
        "evil.example",
        "localhost",
        "localhost:1",
        "HELLO.localhost:1",
    ] {
        let response = client.get(&url).header("Host", host).send().await.unwrap();
        assert_eq!(response.status(), 421);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
    }
    for ip in ["127.0.0.1", "[::1]"] {
        let response = client
            .get(format!("http://{ip}:{}/api/status", preview.ready.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let response = client
        .get(&url)
        .body("x".repeat(65_537))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_install_page_folder_and_reject_invalid_inputs() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let response = preview.install(&fixture("hello")).await;
    assert_eq!(response.status(), 200);
    let body = response.json::<Value>().await.unwrap();
    let installed = &body["installed"][0];
    assert_eq!(installed["local"], "hello");
    assert_eq!(installed["identity"], "local/hello");
    assert_eq!(installed["package"], HELLO_PACKAGE);
    assert_eq!(
        installed["page"],
        format!("http://hello.localhost:{}/", preview.ready.port)
    );
    assert_eq!(installed["has_logic"], false);
    assert_eq!(installed["enabled"], true);
    assert_eq!(installed["grants"], json!([]));
    assert_eq!(installed["links"], json!([]));
    serde_json::from_value::<watane_host::InstalledMod>(installed.clone()).unwrap();
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        assert_eq!(
            fs::read(home.path().join("mods/hello").join(file)).unwrap(),
            fs::read(fixture("hello").join(file)).unwrap()
        );
    }
    let catalog: Value =
        serde_json::from_slice(&fs::read(home.path().join("installed.json")).unwrap()).unwrap();
    assert_eq!(catalog["watane_installed"], 1);
    assert_eq!(catalog["generation"], 1);
    assert_eq!(
        catalog["mods"][0],
        json!({
            "local":"hello","identity":"local/hello","package":HELLO_PACKAGE,
            "release":null,"enabled":true,"grants":[],"links":[]
        })
    );
    let response = preview.install(Path::new("relative")).await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
    let response = preview.install(&fixture("empty")).await;
    assert_eq!(response.status(), 422);
    let error = response.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_mod");
    let pkg_text = watane_pkg::Mod::read(&fixture("empty"))
        .unwrap_err()
        .to_string();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&pkg_text)
    );

    let logic = copy_hello();
    fs::write(
        logic.path().join("wit/world.wit"),
        "package mods:hello@0.1.0;\ninterface api { ping: func(); }\nworld hello { export api; }\n",
    )
    .unwrap();
    fs::write(logic.path().join("logic.wasm"), b"unbuilt").unwrap();
    assert!(
        watane_pkg::Mod::read(logic.path())
            .unwrap()
            .logic()
            .is_some()
    );
    let response = preview.install(logic.path()).await;
    assert_eq!(response.status(), 422);
    let error = response.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_mod");
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("logic parts arrive in a later step")
    );
    assert_eq!(
        fs::read_dir(home.path().join("staging")).unwrap().count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_install_transport_and_symlink_guards() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}install", preview.ready.api);
    for (body, status) in [
        ("broken".to_owned(), 400),
        ("{}".to_owned(), 400),
        ("x".repeat(65_537), 413),
    ] {
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "bad_request"
        );
    }
    let response = client.post(&url).body("{}").send().await.unwrap();
    assert_eq!(response.status(), 415);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
    let response = client
        .post(&url)
        .header("Origin", "null")
        .json(&json!({"folders":[fixture("hello")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(!home.path().join("mods/hello").exists());

    let folder = copy_hello();
    std::os::unix::fs::symlink("/etc/passwd", folder.path().join("ui/leak.txt")).unwrap();
    let response = preview.install(folder.path()).await;
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_batch_install_orders_local_names_and_rejects_a_bad_batch() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let zeta = copy_hello();
    let alpha = copy_hello();
    for (folder, slug) in [(&zeta, "zeta"), (&alpha, "alpha")] {
        fs::write(
            folder.path().join("wit/world.wit"),
            format!("package mods:{slug}@0.1.0;\nworld hello {{}}\n"),
        )
        .unwrap();
    }
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}install", preview.ready.api);
    let response = client
        .post(&url)
        .json(&json!({"folders":[zeta.path(),alpha.path()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = response.json::<Value>().await.unwrap();
    let names: Vec<_> = body["installed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["local"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["alpha", "zeta"]);
    let response = client
        .post(&url)
        .json(&json!({"folders":[fixture("hello"),fixture("empty")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
    let response = client
        .post(&url)
        .json(&json!({"folders":[fixture("hello"),fixture("hello")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
    assert_eq!(
        fs::read_dir(home.path().join("staging")).unwrap().count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_long_slugs_truncate_and_collisions_keep_distinct_origins() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    for (slug, local) in [
        ("abcdefghijklmnopqrstu-one", "abcdefghijklmnopqrstu"),
        ("abcdefghijklmnopqrstu-two", "abcdefghijklmnopqrstu-2"),
        ("abcdefghijklmnopqrst-one", "abcdefghijklmnopqrst"),
    ] {
        let folder = copy_hello();
        fs::write(
            folder.path().join("wit/world.wit"),
            format!("package mods:{slug}@0.1.0;\nworld hello {{}}\n"),
        )
        .unwrap();
        let response = preview.install(folder.path()).await;
        assert_eq!(response.status(), 200);
        let body = response.json::<Value>().await.unwrap();
        assert_eq!(body["installed"][0]["local"], local);
        let response = preview
            .client(&format!("{local}.localhost"), "127.0.0.1")
            .get(format!("http://{local}.localhost:{}/", preview.ready.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
}
