//! Checked-provider component for the manager's judge key test (TECHNICAL 19.6, S63).

wit_bindgen::generate!({
    path: "../../judge-provider/wit",
    world: "judge-provider",
    generate_all,
});

struct Mod;

impl exports::mods::judge_provider::api::Guest for Mod {
    fn claude(launch: String) -> bool {
        watane::host::claude_code::start(&launch).is_ok()
    }

    fn judge(request: String) -> bool {
        watane::host::https_stream::start(&request).is_ok()
    }
}

export!(Mod);
