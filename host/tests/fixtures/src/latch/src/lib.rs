wit_bindgen::generate!({ path: "../../mods/latch/wit", world: "latch", generate_all });
struct Guest;
impl exports::mods::latch::api::Guest for Guest {
    fn firing_origin() -> Option<String> {
        watane::host::timers::firing_origin()
    }
    fn trip(key: String) -> bool {
        let key = format!("trip/{key}");
        if watane::host::state::get(&key).is_some() {
            false
        } else {
            watane::host::state::put(&key, b"1");
            true
        }
    }
}
export!(Guest);
