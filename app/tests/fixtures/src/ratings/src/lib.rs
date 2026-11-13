wit_bindgen::generate!({ path: "../../ratings/wit", world: "ratings", generate_all });
struct Mod;
impl exports::mods::ratings::api::Guest for Mod {
    fn score() -> u32 {
        let count: u32 = watane::host::state::get("calls").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(0) + 1;
        watane::host::state::put("calls", &serde_json::to_vec(&count).unwrap());
        count
    }
}
export!(Mod);
