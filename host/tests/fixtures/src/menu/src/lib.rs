wit_bindgen::generate!({ path: "../../mods/menu/wit", world: "shell", generate_all });
struct Guest;
impl exports::mods::menu::api::Guest for Guest {
    fn items() -> Vec<String> {
        mods::menu::menu::ask()
    }
    fn ping() -> String {
        "pong".into()
    }
    fn click(id: String) {
        mods::menu::menu::clicked(&id);
    }
}
export!(Guest);
