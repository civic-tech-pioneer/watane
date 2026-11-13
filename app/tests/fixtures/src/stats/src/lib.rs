wit_bindgen::generate!({ path: "../../stats/wit", world: "stats", generate_all });
struct Mod;
impl exports::mods::stats::api::Guest for Mod { fn result() -> u32 { mods::ratings::api::score() } }
export!(Mod);
