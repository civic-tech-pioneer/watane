wit_bindgen::generate!({ path: "../../mods/cyc-b/wit", world: "cyc-b", generate_all });
struct Guest;
impl exports::mods::cyc_b::api::Guest for Guest {
    fn ping() {
        mods::cyc_a::api::ping();
    }
}
export!(Guest);
