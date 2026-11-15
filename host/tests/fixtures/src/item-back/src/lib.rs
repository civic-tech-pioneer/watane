wit_bindgen::generate!({ path: "../../mods/item-back/wit", world: "item-back", generate_all });
struct Guest;
impl exports::mods::menu::menu::Guest for Guest {
    fn ask() -> Vec<String> {
        vec![mods::menu::api::ping()]
    }
    fn clicked(_id: String) {}
}
export!(Guest);
