wit_bindgen::generate!({ path: "../../mods/relay-front/wit", world: "relay-front", generate_all });
struct Guest;
impl exports::mods::relay_front::api::Guest for Guest {
    fn after_then_fail(id: String, ms: u64) {
        let _ = mods::relay::api::after(&id, ms);
        panic!("failed after");
    }
    fn push_then_fail(s: String, text: String) {
        let _ = mods::relay::api::push(&s, &text);
        panic!("failed push");
    }
    fn who() -> String {
        mods::relay::api::who()
    }
}
export!(Guest);
