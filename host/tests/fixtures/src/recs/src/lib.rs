wit_bindgen::generate!({ path: "../../mods/recs/wit", world: "recs", generate_all });
struct Guest;
impl exports::mods::recs::api::Guest for Guest {
    fn suggest() -> String {
        let title = mods::shelf::items::all()
            .first()
            .cloned()
            .unwrap_or_default();
        mods::recs::events::suggested(&title);
        title
    }
    fn received() -> Vec<String> {
        watane::host::state::get("received")
            .map(|v| {
                String::from_utf8(v)
                    .unwrap()
                    .lines()
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }
    fn ping() -> String {
        "pong".into()
    }
    fn who_of_shelf() -> String {
        mods::shelf::items::who()
    }
}
impl exports::mods::shelf::events::Guest for Guest {
    fn added(title: String) {
        let mut titles = <Self as exports::mods::recs::api::Guest>::received();
        titles.push(title);
        watane::host::state::put("received", titles.join("\n").as_bytes());
    }
}
export!(Guest);
