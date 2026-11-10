use serde_json::{Value, json};
use watane_host::{CallCode, Caller, Host, Limits, LocalName};
#[tokio::test]
async fn ac8_json_rows_round_trip_and_refuse_bad_paths() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Limits::default()).unwrap();
    host.install_folders(
        &[std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/echo")],
        0,
    )
    .unwrap();
    let call = |function: &str, args: Value| {
        host.call_async(
            Caller::Page,
            LocalName::parse("echo").unwrap(),
            format!("api.{function}"),
            args,
        )
    };
    for (function, value) in [
        ("id-bool", json!(true)),
        ("id-u8", json!(255)),
        ("id-u16", json!(65535)),
        ("id-u32", json!(4294967295u32)),
        ("id-u64", json!(18446744073709551615u64)),
        ("id-s8", json!(-128)),
        ("id-s16", json!(-32768)),
        ("id-s32", json!(-2147483648i32)),
        ("id-s64", json!(-9223372036854775808i64)),
        ("id-f32", json!(1.5)),
        ("id-f64", json!(-2.25)),
        ("id-char", json!("🦀")),
        ("id-string", json!("hello")),
        ("id-list", json!([0, 255])),
        ("id-tuple", json!([7, "seven"])),
        ("id-record", json!({"n":2,"note":"two"})),
        ("id-variant", json!("idle")),
        ("id-variant", json!({"number":7})),
        ("id-enum", json!("done")),
        ("id-flags", json!(["read", "execute"])),
        ("id-option", json!(null)),
        ("id-option", json!(3)),
        ("id-result", json!({"ok":42})),
        ("id-result", json!({"err":"bad"})),
        ("id-unit-result", json!({"ok":null})),
        ("id-unit-result", json!({"err":null})),
    ] {
        for args in [json!({"x":value}), json!([value])] {
            assert_eq!(call(function, args).await.unwrap(), value, "{function}");
        }
    }
    assert_eq!(call("id-option", json!(null)).await.unwrap(), json!(null));
    assert_eq!(
        call("id-record", json!({"x":{"n":1}})).await.unwrap(),
        json!({"n":1,"note":null})
    );
    assert_eq!(
        call("id-flags", json!([["execute", "read"]]))
            .await
            .unwrap(),
        json!(["read", "execute"])
    );
    for function in ["nan", "infinity"] {
        assert_eq!(call(function, json!(null)).await.unwrap(), json!(null));
    }
    assert_eq!(
        call("opt-opt", json!([null])).await.unwrap_err().code,
        CallCode::NoFunction
    );
    for (function, args, path) in [
        ("id-u8", json!({"x":1.0}), "$.x:"),
        ("id-record", json!({"x":{"n":1,"extra":2}}), "$.x.extra:"),
        ("id-tuple", json!({"x":[1]}), "$.x:"),
        ("id-u8", json!({"x":256}), "$.x:"),
        ("id-list", json!({"x":[1,256]}), "$.x[1]:"),
        ("id-char", json!({"x":"ab"}), "$.x:"),
        ("id-flags", json!({"x":["read","read"]}), "$.x[1]:"),
        ("id-enum", json!({"x":"bad"}), "$.x:"),
        ("id-u8", json!({"extra":1,"x":1}), "$.extra:"),
        ("id-u8", json!([]), "$:"),
        ("id-f64", json!({"x":null}), "$.x:"),
    ] {
        let error = call(function, args).await.unwrap_err();
        assert_eq!(error.code, CallCode::BadRequest);
        assert!(error.message.contains(path), "{function}: {error}");
    }
}
