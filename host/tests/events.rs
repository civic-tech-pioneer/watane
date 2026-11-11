use super::*;
use watane_host::{Caller, Host, Limits, LocalName};

struct Stream {
    response: reqwest::Response,
    buffered: Vec<u8>,
}
impl Stream {
    async fn open(client: &Client, origin: &str) -> Self {
        let response = client.get(format!("{origin}/events")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        Self {
            response,
            buffered: vec![],
        }
    }
    async fn frame(&mut self) -> String {
        loop {
            if let Some(end) = self.buffered.windows(2).position(|w| w == b"\n\n") {
                return String::from_utf8(self.buffered.drain(..end + 2).collect()).unwrap();
            }
            self.buffered
                .extend(self.response.chunk().await.unwrap().expect("stream ended"));
        }
    }
    async fn message(&mut self) -> Value {
        let frame = tokio::time::timeout(Duration::from_secs(30), self.frame()) // hang guard
            .await
            .expect("no message");
        assert!(frame.starts_with("data: "), "{frame:?}");
        assert!(frame.ends_with("\n\n"));
        serde_json::from_str(
            frame
                .strip_prefix("data: ")
                .unwrap()
                .strip_suffix("\n\n")
                .unwrap(),
        )
        .unwrap()
    }
    async fn quiet(&mut self) {
        assert!(
            tokio::time::timeout(Duration::from_millis(500), self.frame())
                .await
                .is_err()
        );
    }
}

async fn setup() -> (
    TempDir,
    std::sync::Arc<Host>,
    watane_host::Served,
    Client,
    String,
) {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("counter"), fixture("echo")], 0)
        .unwrap();
    let served = host.serve(0).await.unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("counter.localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("echo.localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("absent.localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let origin = format!("http://counter.localhost:{}", served.port);
    (home, host, served, client, origin)
}
async fn call(client: &Client, origin: &str, function: &str) -> reqwest::Response {
    client
        .post(format!("{origin}/call"))
        .header("Origin", origin)
        .json(&json!({"fn":function,"args":null}))
        .send()
        .await
        .unwrap()
}
fn counted(count: u32) -> Value {
    json!({"type":"event","from":"counter","interface":"mods:counter/changes@0.1.0","function":"counted","args":{"count":count}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac1_s13_outlet_then_changed_and_get_is_quiet() {
    let (_home, _host, served, client, origin) = setup().await;
    let mut stream = Stream::open(&client, &origin).await;
    assert_eq!(
        call(&client, &origin, "api.add")
            .await
            .json::<Value>()
            .await
            .unwrap(),
        json!({"value":1})
    );
    assert_eq!(stream.message().await, counted(1));
    assert_eq!(stream.message().await, json!({"type":"changed"}));
    assert_eq!(
        call(&client, &origin, "api.get")
            .await
            .json::<Value>()
            .await
            .unwrap(),
        json!({"value":1})
    );
    stream.quiet().await;
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_emission_order_and_failed_call_is_quiet() {
    let (_home, _host, served, client, origin) = setup().await;
    let mut stream = Stream::open(&client, &origin).await;
    for value in [1, 2, 3] {
        assert_eq!(
            call(&client, &origin, "api.add")
                .await
                .json::<Value>()
                .await
                .unwrap(),
            json!({"value":value})
        );
    }
    for count in [1, 2, 3] {
        assert_eq!(stream.message().await, counted(count));
        assert_eq!(stream.message().await, json!({"type":"changed"}));
    }
    assert_eq!(
        call(&client, &origin, "api.add-then-fail").await.status(),
        422
    );
    stream.quiet().await;
    assert_eq!(
        call(&client, &origin, "api.get")
            .await
            .json::<Value>()
            .await
            .unwrap(),
        json!({"value":3})
    );
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_fan_out_stays_on_the_emitters_origin() {
    let (_home, host, served, client, origin) = setup().await;
    let mut first = Stream::open(&client, &origin).await;
    let mut second = Stream::open(&client, &origin).await;
    let mut echo = Stream::open(&client, &format!("http://echo.localhost:{}", served.port)).await;
    // A library call and the server share the bus.
    assert_eq!(
        host.call_async(
            Caller::Page,
            LocalName::parse("counter").unwrap(),
            "api.add".into(),
            Value::Null
        )
        .await
        .unwrap(),
        json!(1)
    );
    for stream in [&mut first, &mut second] {
        assert_eq!(stream.message().await, counted(1));
        assert_eq!(stream.message().await, json!({"type":"changed"}));
    }
    echo.quiet().await;
    served.shutdown().await;
}

fn copy_counter() -> TempDir {
    fn copy(source: &Path, dest: &Path) {
        fs::create_dir_all(dest).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = dest.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let folder = tempfile::tempdir().unwrap();
    copy(&fixture("counter"), folder.path());
    folder
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_canonical_id_changes_reload_pages_only_once() {
    let (_home, host, served, client, origin) = setup().await;
    let folder = copy_counter();
    let folders = [folder.path().to_owned()];
    let mut stream = Stream::open(&client, &origin).await;
    let installed = host.install_folders(&folders, served.port).unwrap();
    assert_eq!(
        installed[0].package,
        watane_pkg::pack(folder.path(), false).unwrap().id()
    );
    stream.quiet().await;
    let path = folder.path().join("ui/index.html");
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
    let updated = host.install_folders(&folders, served.port).unwrap();
    assert_ne!(updated[0].package, installed[0].package);
    assert_eq!(stream.message().await, json!({"type":"reload"}));
    host.install_folders(&folders, served.port).unwrap();
    stream.quiet().await;
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac4_s23_update_keeps_count_local_and_reloads_only_changed_builds() {
    let (_home, host, served, client, origin) = setup().await;
    for value in [1, 2] {
        assert_eq!(
            call(&client, &origin, "api.add")
                .await
                .json::<Value>()
                .await
                .unwrap(),
            json!({"value":value})
        );
    }
    let mut stream = Stream::open(&client, &origin).await;
    let folder = copy_counter();
    for (http, bytes) in [
        (false, "<p>revision two</p>\n"),
        (true, "<p>revision three</p>\n"),
    ] {
        fs::write(folder.path().join("ui/index.html"), bytes).unwrap();
        if http {
            let response = client
                .post(format!("{}install", served.api_url()))
                .json(&json!({"folders":[folder.path()]}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(
                response.json::<Value>().await.unwrap()["installed"][0]["local"],
                "counter"
            );
        } else {
            assert_eq!(
                host.install_folders(&[folder.path().to_owned()], served.port)
                    .unwrap()[0]
                    .local
                    .as_ref(),
                "counter"
            );
        }
        assert_eq!(stream.message().await, json!({"type":"reload"}));
        assert_eq!(
            client
                .get(format!("{origin}/"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            bytes
        );
        assert_eq!(
            call(&client, &origin, "api.get")
                .await
                .json::<Value>()
                .await
                .unwrap(),
            json!({"value":2})
        );
        host.install_folders(&[folder.path().to_owned()], served.port)
            .unwrap();
        stream.quiet().await;
    }
    // A failed catalog commit must never announce a revision.
    fs::write(folder.path().join("ui/index.html"), "uncommitted").unwrap();
    fs::create_dir(host.home().join("installed.json.tmp")).unwrap();
    assert!(
        host.install_folders(&[folder.path().to_owned()], served.port)
            .is_err()
    );
    stream.quiet().await;
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_events_headers_origin_installed_and_method_guards() {
    let (_home, _host, served, client, origin) = setup().await;
    for supplied in [None, Some(origin.as_str())] {
        let mut request = client.get(format!("{origin}/events"));
        if let Some(origin) = supplied {
            request = request.header("Origin", origin);
        }
        let response = request.send().await.unwrap();
        assert_eq!(response.status(), 200);
        pages::headers(&response);
        assert_eq!(response.headers()["content-type"], "text/event-stream");
    }
    for supplied in [
        "null".to_owned(),
        String::new(),
        format!("{origin}/"),
        format!("http://echo.localhost:{}", served.port),
    ] {
        let response = client
            .get(format!("{origin}/events"))
            .header("Origin", supplied)
            .send()
            .await
            .unwrap();
        pages::call_error(response, 403, "forbidden").await;
    }
    let response = client
        .get(format!("{origin}/events"))
        .header("Origin", &origin)
        .header("Origin", &origin)
        .send()
        .await
        .unwrap();
    pages::call_error(response, 403, "forbidden").await;
    let response = client
        .get(format!("http://absent.localhost:{}/events", served.port))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    pages::headers(&response);
    let response = client
        .post(format!("{origin}/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 405);
    assert_eq!(response.headers()["allow"], "GET");
    pages::headers(&response);
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac8_program_emits_counter_events() {
    let home = tempfile::tempdir().unwrap();
    let counter = fixture("counter");
    let mut preview =
        Preview::start_with(home.path(), &["--install".as_ref(), counter.as_os_str()]);
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    let client = preview.client("counter.localhost", "127.0.0.1");
    let mut stream = Stream::open(&client, &origin).await;
    assert_eq!(
        call(&client, &origin, "api.add")
            .await
            .json::<Value>()
            .await
            .unwrap(),
        json!({"value":1})
    );
    assert_eq!(stream.message().await, counted(1));
    assert_eq!(stream.message().await, json!({"type":"changed"}));
    preview.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac8_s11_subscriber_pages_receive_event_then_changed() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("recs"), fixture("shelf")], 0)
        .unwrap();
    let served = host.serve(0).await.unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("recs.localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let mut stream = Stream::open(&client, &format!("http://recs.localhost:{}", served.port)).await;
    host.call_async(
        Caller::Page,
        LocalName::parse("shelf").unwrap(),
        "items.add".into(),
        json!({"title":"Dune"}),
    )
    .await
    .unwrap();
    assert_eq!(
        stream.message().await,
        json!({"type":"event","from":"shelf","interface":"mods:shelf/events@0.1.0","function":"added","args":{"title":"Dune"}})
    );
    assert_eq!(stream.message().await, json!({"type":"changed"}));
    stream.quiet().await;
    served.shutdown().await;
}
