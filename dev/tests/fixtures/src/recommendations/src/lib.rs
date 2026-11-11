wit_bindgen::generate!({ path: "../../recommendations/wit", world: "recommendations", generate_all });
struct Mod;
use watane::host::state;
impl exports::mods::recommendations::api::Guest for Mod {
    fn all() -> Vec<String> { mods::reading_list::books::all() }
    fn list() -> Vec<String> { mods::reading_list::books::list() }
    fn received() -> Vec<String> {
        state::get("received").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}
impl exports::mods::reading_list::events::Guest for Mod {
    fn added(title: String) {
        let mut titles = <Self as exports::mods::recommendations::api::Guest>::received();
        titles.push(title.clone());
        state::put("received", &serde_json::to_vec(&titles).unwrap());
        mods::recommendations::changes::received(&title);
    }
}
export!(Mod);
