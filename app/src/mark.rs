//! The consent mark (TECHNICAL 6.4): two words in `<home>/app/mark` that a mod
//! cannot know.

use std::path::Path;
pub const WORDS: [&str; 64] = [
    "amber", "anchor", "apple", "arrow", "aspen", "badge", "basil", "beacon", "birch", "bison",
    "bloom", "cabin", "cactus", "canyon", "cedar", "cliff", "clover", "comet", "coral", "crane",
    "delta", "dune", "eagle", "ember", "fern", "fjord", "flint", "forest", "garnet", "glacier",
    "harbor", "hazel", "heron", "island", "ivory", "jade", "juniper", "kayak", "lagoon", "lantern",
    "lemon", "lily", "maple", "meadow", "mesa", "moss", "nectar", "oak", "olive", "orchid",
    "otter", "pebble", "pine", "prairie", "quartz", "raven", "reef", "river", "saffron", "sage",
    "spruce", "tulip", "walnut", "willow",
];
pub fn read_or_create(home: &Path) -> std::io::Result<String> {
    use std::fs;
    use std::io::{self, Write};
    let directory = home.join("app");
    let path = directory.join("mark");
    match fs::read(&path) {
        Ok(bytes) => {
            if let Ok(text) = std::str::from_utf8(&bytes)
                && let Some(mark) = text.strip_suffix('\n')
                && let Some((first, second)) = mark.split_once(' ')
                && WORDS.contains(&first)
                && WORDS.contains(&second)
            {
                return Ok(mark.into());
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(directory)?;
    let mut random = [0; 2];
    getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
    let mark = format!(
        "{} {}",
        WORDS[usize::from(random[0]) % 64],
        WORDS[usize::from(random[1]) % 64]
    );
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600)
    };
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?
    };
    writeln!(file, "{mark}")?;
    Ok(mark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;

    fn valid(home: &Path) -> String {
        let mark = read_or_create(home).unwrap();
        let bytes = fs::read_to_string(home.join("app/mark")).unwrap();
        assert_eq!(bytes, format!("{mark}\n"));
        let words: Vec<_> = mark.split(' ').collect();
        assert_eq!(words.len(), 2);
        assert!(
            words
                .iter()
                .all(|word| WORDS.contains(word) && word.bytes().all(|c| c.is_ascii_lowercase()))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(home.join("app/mark"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            )
        };
        mark
    }

    #[test]
    fn ac10_create_reuse_repair_and_64_distinct_words() {
        assert_eq!(WORDS.len(), 64);
        assert_eq!(WORDS.into_iter().collect::<HashSet<_>>().len(), 64);
        let home = tempfile::tempdir().unwrap();
        let mark = valid(home.path());
        assert_eq!(valid(home.path()), mark);
        for bad in [
            "",
            "cedar\n",
            "cedar notaword\n",
            "cedar otter",
            "cedar  otter\n",
            "cedar otter\n\n",
        ] {
            fs::write(home.path().join("app/mark"), bad).unwrap();
            valid(home.path());
        }
        fs::write(home.path().join("app/mark"), [0xff]).unwrap();
        valid(home.path());
    }
}
