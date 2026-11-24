//! The Windows preview CLI refuses commands that need Unix process groups.

use serde_json::Value;
use std::process::Command;

#[test]
fn s110_preview_refuses_start_and_stop_before_touching_the_cache() {
    let temp = tempfile::tempdir().expect("create empty working directory");
    let cache = temp.path().join("cache");
    for flags in [&[][..], &["--stop"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .args(["serve", "--json"])
            .args(flags)
            .current_dir(temp.path())
            .env("WATANE_DEV_CACHE", &cache)
            .output()
            .expect("run watane-dev");
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let result: Value = serde_json::from_slice(&output.stdout).expect("read JSON envelope");
        assert_eq!(result["errors"][0]["code"], "env.preview_start");
        assert!(
            result["errors"][0]["message"]
                .as_str()
                .is_some_and(|message| message.contains("requires Unix process-group controls")),
            "{result}"
        );
        assert_eq!(
            result["errors"][0]["fix"],
            "Run watane-dev serve on Linux or macOS; on Windows, watane-dev test still builds and tests the mod"
        );
        assert_eq!(result["next"], result["errors"][0]["fix"]);
        assert!(!cache.exists());
    }
}
