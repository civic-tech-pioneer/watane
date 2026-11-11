wit_bindgen::generate!({ path: "../../mods/shelf/wit", world: "shelf", generate_all });
struct Guest;
impl exports::mods::shelf::items::Guest for Guest {
    fn all() -> Vec<String> {
        watane::host::state::get("items")
            .map(|v| {
                String::from_utf8(v)
                    .unwrap()
                    .lines()
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn add(title: String) {
        let mut titles = Self::all();
        titles.push(title.clone());
        watane::host::state::put("items", titles.join("\n").as_bytes());
        mods::shelf::events::added(&title);
    }
    fn add_then_fail(title: String) {
        Self::add(title);
        panic!("failed add");
    }
    fn who() -> String {
        watane::host::caller::who()
    }
}
export!(Guest);
