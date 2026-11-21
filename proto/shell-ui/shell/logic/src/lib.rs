//! A shell that keeps menu and toolbar items contributed by other mods.
wit_bindgen::generate!({ path: "../wit", world: "shell", generate_all });

use exports::mods::shell::registry::{self, Item, Place};
use exports::mods::shell::ui;
use mods::shell::{announce, commands};
use watane::host::{caller, state};

/// One stored item: (owner, id, label, place). Tuples, because the mod may not use serde derive.
type Row = (String, String, String, String);

struct Mod;

impl registry::Guest for Mod {
    fn add(item: Item) {
        let owner = caller::who();
        let mut rows = load();
        upsert(&mut rows, (owner, item.id, item.label, place_name(item.place).into()));
        save(&rows);
    }

    fn list() -> Vec<Item> {
        load()
            .into_iter()
            .map(|(_, id, label, place)| Item { id, label, place: place_of(&place) })
            .collect()
    }

    fn notify(text: String) {
        let line = format!("{}: {text}", caller::who().trim_start_matches("mod:"));
        state::put("status", line.as_bytes());
    }

    fn status() -> Option<String> {
        state::get("status").and_then(|b| String::from_utf8(b).ok())
    }
}

impl ui::Guest for Mod {
    fn start() {
        // ponytail: clear-then-announce drops items of removed mods; a mod removed while the page is open stays until reload.
        state::delete("items");
        announce::announce();
    }

    fn click(id: String) {
        // ponytail: broadcast, every contributor gets every click; the event cannot address the owner.
        commands::invoke(&id);
    }
}

export!(Mod);

fn load() -> Vec<Row> {
    state::get("items").and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(rows: &[Row]) {
    state::put("items", &serde_json::to_vec(rows).expect("strings always encode"));
}

/// Replaces the row with the same owner and id, else appends it.
fn upsert(rows: &mut Vec<Row>, row: Row) {
    match rows.iter_mut().find(|r| r.0 == row.0 && r.1 == row.1) {
        Some(old) => *old = row,
        None => rows.push(row),
    }
}

fn place_name(place: Place) -> &'static str {
    match place {
        Place::Menu => "menu",
        Place::Toolbar => "toolbar",
    }
}

fn place_of(name: &str) -> Place {
    if name == "toolbar" { Place::Toolbar } else { Place::Menu }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(owner: &str, id: &str, label: &str) -> Row {
        (owner.into(), id.into(), label.into(), "menu".into())
    }

    #[test]
    fn upsert_replaces_by_owner_and_id() {
        let mut rows = vec![];
        upsert(&mut rows, row("mod:a", "x", "one"));
        upsert(&mut rows, row("mod:b", "x", "other owner"));
        upsert(&mut rows, row("mod:a", "x", "two"));
        assert_eq!(rows, vec![row("mod:a", "x", "two"), row("mod:b", "x", "other owner")]);
    }
}
