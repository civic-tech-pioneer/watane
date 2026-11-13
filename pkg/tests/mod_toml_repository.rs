#[path = "support/release.rs"]
#[allow(dead_code)]
mod support;

use std::fs;
use watane_pkg::{Mod, ModError, Package, Rule, pack};

const URL: &str = "https://github.com/alice/hello";

#[test]
fn ac1_ac2_ac3_mod_read_and_package_round_trip() {
    let plain = support::built_fixture("hello");
    let plain_mod = Mod::read(plain.path()).unwrap();
    assert_eq!(plain_mod.repository(), None);
    let plain_id = pack(plain.path(), false).unwrap().id();

    let dir = support::built_fixture("hello");
    let toml = dir.path().join("mod.toml");
    let text = fs::read_to_string(&toml).unwrap();
    fs::write(&toml, format!("{text}repository = \"{URL}\"\n")).unwrap();
    assert_eq!(Mod::read(dir.path()).unwrap().repository(), Some(URL));

    let package = pack(dir.path(), false).unwrap();
    assert_ne!(package.id(), plain_id);
    let package = Package::from_bytes(package.as_bytes().to_vec()).unwrap();
    let out = tempfile::tempdir().unwrap();
    let unpacked = package.unpack(&out.path().join("m")).unwrap();
    assert_eq!(unpacked.repository(), Some(URL));
    assert_eq!(
        Mod::read(&out.path().join("m")).unwrap().repository(),
        Some(URL)
    );

    fs::write(&toml, format!("{text}repository = \"http://x/y\"\n")).unwrap();
    match Mod::read(dir.path()) {
        Err(ModError::Invalid(v)) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].rule, Rule::ModToml);
            assert_eq!(v[0].file, "mod.toml");
            assert_eq!(v[0].message, "`repository` must start with https://");
        }
        other => panic!("{other:?}"),
    }
}
