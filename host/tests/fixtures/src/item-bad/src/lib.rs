wit_bindgen::generate!({ path: "../../mods/item-bad/wit", world: "item-bad", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    fn ask() -> Vec<String> {
        watane::host::state::put("key", b"1");
        panic!("bad answer");
    }
    fn clicked(_id: String) {}
}
export!(Guest);
