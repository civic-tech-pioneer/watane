wit_bindgen::generate!({ path: "../../mods/item-a/wit", world: "item-a", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    fn ask() -> Vec<String> {
        watane::host::state::put("asked", b"1");
        vec!["a".into()]
    }
    fn clicked(id: String) {
        watane::host::state::put("clicked", id.as_bytes());
    }
}
export!(Guest);
