use std::{fs, path::Path};
use wit_component::{ComponentEncoder, StringEncoding, dummy_module, embed_component_metadata};
use wit_parser::{ManglingAndAbi, Resolve};

fn put(dir: &Path, name: &str, data: impl AsRef<[u8]>) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

pub fn hello() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, bytes) in [
        ("mod.toml", "name = \"Hello\"\nsummary = \"Says hello.\"\n"),
        ("ui/index.html", "<p>hello</p>\n"),
        (
            "wit/world.wit",
            "package mods:hello@0.1.0;\n\nworld hello {}\n",
        ),
    ] {
        put(dir.path(), path, bytes);
    }
    dir
}

pub fn built_fixture(name: &str) -> tempfile::TempDir {
    fn copy(source: &Path, dest: &Path) {
        fs::create_dir_all(dest).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let target = dest.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&path, &target);
            } else {
                fs::copy(path, target).unwrap();
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
        dir.path(),
    );
    dir
}

#[test]
#[ignore = "regenerates fixtures"]
fn regenerate_fixtures() {
    for name in ["reading-list", "recommendations"] {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        regenerate(&dir);
    }
}

fn regenerate(dir: &Path) {
    // Independent producer: the fixture's WIT -> dummy core module -> component.
    let mut resolve = Resolve::default();
    let (package, _) = resolve.push_dir(dir.join("wit")).unwrap();
    let world = resolve.select_world(&[package], None).unwrap();
    let mut module = dummy_module(&resolve, world, ManglingAndAbi::Standard32);
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8).unwrap();
    let component = ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap();
    put(dir, "logic.wasm", component);
}
