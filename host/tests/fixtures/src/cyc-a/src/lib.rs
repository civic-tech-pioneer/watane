wit_bindgen::generate!({ path: "../../mods/cyc-a/wit", world: "cyc-a", generate_all });
struct Guest;
impl exports::mods::cyc_a::api::Guest for Guest {
    fn ping() {
        mods::cyc_b::api::ping();
    }
}
export!(Guest);
