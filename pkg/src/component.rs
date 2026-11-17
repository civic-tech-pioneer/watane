//! `Mod::check_component` (TECHNICAL 4.9): checks a mod's `logic.wasm` against
//! its world, after checking its size.

use crate::folder::{invalid, read_file};
use crate::{InterfaceName, Mod, ModError, Rule, Violation};
use std::collections::BTreeSet;
use wit_parser::decoding::{DecodedWasm, decode};
use wit_parser::{Resolve, WorldItem, WorldKey};

const MAX_COMPONENT_BYTES: u64 = 67_108_864;
const FILE: &str = "logic.wasm";

fn name(resolve: &Resolve, key: &WorldKey) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(id) => resolve
            .id_of(*id)
            .unwrap_or_else(|| "<anonymous interface>".into()),
    }
}

fn base_wasi(name: &str) -> bool {
    let Ok(interface) = name.parse::<InterfaceName>() else {
        return false;
    };
    let version = interface.version();
    interface.namespace() == "wasi"
        && matches!(
            interface.package(),
            "cli" | "clocks" | "filesystem" | "io" | "random" | "sockets"
        )
        && version.major == 0
        && version.minor == 2
}

impl Mod {
    /// Check the built logic part against the WIT contract, without running it.
    pub fn check_component(&self) -> Result<(), ModError> {
        if self.logic().is_none() {
            return Ok(());
        }
        let bytes = read_file(
            self.dir(),
            FILE,
            MAX_COMPONENT_BYTES,
            Rule::Component,
            false,
        )?
        .ok_or_else(|| invalid(Rule::Component, FILE, "required file is missing"))?;
        // decode() also accepts an empty core module as an empty component world.
        if !bytes.starts_with(b"\0asm\x0d\0\x01\0") {
            return Err(invalid(
                Rule::Component,
                FILE,
                "logic.wasm is not a WebAssembly component",
            ));
        }
        #[expect(
            clippy::disallowed_methods,
            reason = "defense in depth: wit-component may panic on untrusted bytes"
        )]
        let decoded = std::panic::catch_unwind(|| decode(&bytes));
        let Ok(Ok(DecodedWasm::Component(resolve, world))) = decoded else {
            return Err(invalid(
                Rule::Component,
                FILE,
                "logic.wasm is not a WebAssembly component",
            ));
        };
        let declared = self.component_imports()?;
        let world = &resolve.worlds[world];
        let mut violations = Vec::new();
        for (key, item) in &world.imports {
            let name = name(&resolve, key);
            if !matches!(
                (key, item),
                (WorldKey::Interface(_), WorldItem::Interface { .. })
            ) || !(declared.contains(&name) || base_wasi(&name))
            {
                violations.push(Violation {
                    rule: Rule::Component,
                    file: FILE.into(),
                    message: format!(
                        "logic.wasm imports `{name}`, which wit/world.wit does not declare"
                    ),
                });
            }
        }
        let actual: BTreeSet<_> = world
            .exports
            .keys()
            .map(|key| name(&resolve, key))
            .collect();
        let expected: BTreeSet<_> = self
            .world()
            .exports
            .iter()
            .chain(&self.world().subscriptions)
            .map(ToString::to_string)
            .collect();
        for name in expected.difference(&actual) {
            violations.push(Violation {
                rule: Rule::Component,
                file: FILE.into(),
                message: format!(
                    "logic.wasm does not export `{name}`, which wit/world.wit declares"
                ),
            });
        }
        for name in actual.difference(&expected) {
            violations.push(Violation {
                rule: Rule::Component,
                file: FILE.into(),
                message: format!(
                    "logic.wasm exports `{name}`, which wit/world.wit does not declare"
                ),
            });
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(ModError::Invalid(violations))
        }
    }
}
