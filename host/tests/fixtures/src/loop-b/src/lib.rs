wit_bindgen::generate!({ path: "../../mods/loop-b/wit", world: "loop-b", generate_all });
struct Guest;
impl exports::mods::loop_b::api::Guest for Guest {
    fn count() -> u32 {
        watane::host::state::get("count")
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
            .unwrap_or(0)
    }
}
impl exports::mods::loop_a::events::Guest for Guest {
    fn ping(n: u32) {
        let count = <Self as exports::mods::loop_b::api::Guest>::count() + 1;
        watane::host::state::put("count", &count.to_le_bytes());
        mods::loop_b::events::ping(n + 1);
    }
}
export!(Guest);
