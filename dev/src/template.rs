use crate::outcome::{Code, Problem};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};
use watane_pkg::Slug;
const FILES: &[(&str, &str)] = &[
    (".gitignore", include_str!("../template/.gitignore")),
    ("AGENTS.md", include_str!("../template/AGENTS.md")),
    ("CLAUDE.md", include_str!("../template/CLAUDE.md")),
    (
        "logic/Cargo.toml",
        include_str!("../template/logic/Cargo.toml.tmpl"),
    ),
    (
        "logic/src/lib.rs",
        include_str!("../template/logic/src/lib.rs"),
    ),
    ("mod.toml", include_str!("../template/mod.toml")),
    (
        "tests/counter.json",
        include_str!("../template/tests/counter.json"),
    ),
    ("ui/app.js", include_str!("../template/ui/app.js")),
    ("ui/index.html", include_str!("../template/ui/index.html")),
    ("wit/deps/watane-host/host.wit", watane_host::HOST_WIT),
    ("wit/world.wit", include_str!("../template/wit/world.wit")),
];
pub fn render(text: &str, slug: &Slug) -> String {
    let title = slug.to_string().replace('-', " ");
    let title = format!("{}{}", title[..1].to_uppercase(), &title[1..]);
    text.replace("{{slug}}", slug.as_str())
        .replace("{{crate}}", &slug.to_string().replace('-', "_"))
        .replace("{{title}}", &title)
        .replace(
            "{{crates}}",
            &crate::allowlist::Allowlist::embedded()
                .direct()
                .iter()
                .map(|(name, version)| format!("- {name} {version}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
}
#[derive(Serialize)]
pub struct NewReport {
    pub slug: String,
    pub dir: PathBuf,
    pub files: Vec<String>,
}
pub fn new_mod(dir: &Path, slug: &str) -> Result<NewReport, Problem> {
    let slug=Slug::from_str(slug).map_err(|e|Problem::new(Code::UsageSlug,format!("{slug:?} is not a valid slug: {e}")).fix("Use lowercase words of letters and digits joined by \"-\", each starting with a letter, at most 40 bytes, not a WIT keyword. Example: reading-list"))?;
    if let Err(e) = fs::create_dir(dir) {
        return Err(if e.kind() == std::io::ErrorKind::AlreadyExists {
            Problem::new(Code::UsageExists, format!("{} exists", dir.display()))
                .fix("Choose another slug, or pass --dir <new folder>")
        } else {
            Problem::internal(e)
        });
    }
    let write = || -> Result<(), std::io::Error> {
        for (name, text) in FILES {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, render(text, &slug))?;
        }
        Ok(())
    };
    if let Err(e) = write() {
        let _ = fs::remove_dir_all(dir);
        return Err(Problem::internal(e));
    }
    Ok(NewReport {
        slug: slug.to_string(),
        dir: dir.canonicalize().map_err(Problem::internal)?,
        files: FILES.iter().map(|(name, _)| (*name).to_string()).collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac3_guide_documents_fakes_and_delivery() {
        let guide = include_str!("../template/AGENTS.md");
        assert!(!guide.contains("arrive with links"));
        assert!(guide.contains("mods:<slug>/<iface>"));
        assert!(guide.contains("no fake"));
        let examples: Vec<serde_json::Value> = guide
            .split("```json\n")
            .skip(1)
            .map(|block| serde_json::from_str(block.split("```").next().unwrap()).unwrap())
            .collect();
        assert!(
            examples
                .iter()
                .any(|v| v["fakes"]["mods:reading-list/books"]["all"]
                    == serde_json::json!(["Dune"])
                    && v["steps"][0]["deliver"] == "events.added"
                    && v["steps"][0]["args"] == serde_json::json!({"title": "Emma"}))
        );
    }

    #[test]
    fn render_all_placeholders() {
        let slug = "a-b-c".parse().unwrap();
        assert_eq!(
            render("{{slug}} {{crate}} {{title}}", &slug),
            "a-b-c a_b_c A b c"
        );
        for (_, text) in FILES {
            assert!(!render(text, &slug).contains("{{"));
        }
    }
}
