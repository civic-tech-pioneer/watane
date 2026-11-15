wit_bindgen::generate!({ path: "../../mods/item-b/wit", world: "item-b", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    fn ask() -> Vec<String> {
        watane::host::state::put("asked", b"1");
        vec!["b".into()]
    }
    fn clicked(id: String) {
        watane::host::state::put("clicked", id.as_bytes());
    }
}
export!(Guest);
