//! Contributes two items to the shell and answers their clicks.
wit_bindgen::generate!({ path: "../wit", world: "greeter", generate_all });

use mods::shell::registry::{self, Item, Place};
use watane::host::state;

// ponytail: ids are a namespace shared by every contributor, so prefix them with the slug.
const HELLO: &str = "greeter.hello";
const RESET: &str = "greeter.reset";

struct Mod;

impl exports::mods::shell::announce::Guest for Mod {
    fn announce() {
        registry::add(&Item { id: HELLO.into(), label: "Say hello".into(), place: Place::Menu });
        registry::add(&Item { id: RESET.into(), label: "Reset greetings".into(), place: Place::Toolbar });
    }
}

impl exports::mods::shell::commands::Guest for Mod {
    fn invoke(id: String) {
        let count = match id.as_str() {
            HELLO => count().saturating_add(1),
            RESET => 0,
            _ => return, // another contributor's item
        };
        state::put("count", &count.to_string().into_bytes());
        registry::notify(&greeting(count));
    }
}

impl exports::mods::greeter::api::Guest for Mod {
    fn count() -> u32 {
        count()
    }
}

export!(Mod);

fn count() -> u32 {
    state::get("count").and_then(|b| String::from_utf8(b).ok()?.parse().ok()).unwrap_or(0)
}

fn greeting(count: u32) -> String {
    match count {
        0 => "greetings reset".into(),
        n => format!("hello #{n}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn greets_by_number() {
        assert_eq!(super::greeting(0), "greetings reset");
        assert_eq!(super::greeting(3), "hello #3");
    }
}
