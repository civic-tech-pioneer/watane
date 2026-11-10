use crate::outcome::{Code, Problem};
use serde_json::{Value, json};
use std::{fs, path::Path, str::FromStr};
use watane_pkg::Slug;
const FILES: &[(&str, &str)] = &[
    (".gitignore", include_str!("../template/.gitignore")),
    ("AGENTS.md", include_str!("../template/AGENTS.md")),
    ("CLAUDE.md", include_str!("../template/CLAUDE.md")),
    ("mod.toml", include_str!("../template/mod.toml")),
    ("ui/app.js", include_str!("../template/ui/app.js")),
    ("ui/index.html", include_str!("../template/ui/index.html")),
    ("wit/world.wit", include_str!("../template/wit/world.wit")),
];
pub fn render(text: &str, slug: &Slug) -> String {
    let title = slug.to_string().replace('-', " ");
    let title = format!("{}{}", title[..1].to_uppercase(), &title[1..]);
    text.replace("{{slug}}", slug.as_str())
        .replace("{{crate}}", &slug.to_string().replace('-', "_"))
        .replace("{{title}}", &title)
}
pub fn new_mod(dir: &Path, slug: &str) -> Result<Value, Problem> {
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
    Ok(
        json!({"slug":slug,"dir":dir.canonicalize().map_err(Problem::internal)?,"files":FILES.iter().map(|(name,_)|name).collect::<Vec<_>>()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
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
