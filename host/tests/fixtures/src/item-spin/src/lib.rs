wit_bindgen::generate!({ path: "../../mods/item-spin/wit", world: "item-spin", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    #[allow(clippy::empty_loop)]
    fn ask() -> Vec<String> {
        loop {}
    }
    fn clicked(_id: String) {}
}
export!(Guest);
