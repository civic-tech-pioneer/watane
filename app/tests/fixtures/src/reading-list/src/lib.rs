wit_bindgen::generate!({ path: "../../reading-list/wit", world: "reading-list", generate_all });
use exports::mods::reading_list::books::Guest;
use watane::host::state;
struct Mod;
impl Guest for Mod {
    fn add(title: String) -> u32 {
        let mut books = Self::all();
        books.push(title.clone());
        state::put("books", &serde_json::to_vec(&books).unwrap());
        mods::reading_list::events::added(&title);
        books.len() as u32
    }
    fn all() -> Vec<String> { state::get("books").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default() }
    fn list() -> Vec<String> { Self::all() }
}
export!(Mod);
