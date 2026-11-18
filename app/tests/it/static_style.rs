use std::fs;
use std::path::Path;
#[test]
fn ac1_tokens_themes_monograms_and_local_styles() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let css = fs::read_to_string(root.join("ui/style.css")).unwrap();
    let (light, dark) = css
        .split_once("@media (prefers-color-scheme: dark)")
        .expect("system dark theme");
    let root_block = |sheet: &str| -> String {
        sheet
            .split_once(":root")
            .unwrap()
            .1
            .split_once('{')
            .unwrap()
            .1
            .split_once('}')
            .unwrap()
            .0
            .to_owned()
    };
    let light = root_block(light);
    let dark = root_block(dark);
    for (token, day, night) in [
        ("--bg", "#F6F6F4", "#111113"),
        ("--panel", "#FFFFFF", "#19191C"),
        ("--ink", "#18181B", "#EDEDEF"),
        ("--muted", "#5D5E66", "#A3A3AC"),
        ("--line", "#E3E3DF", "#2B2B30"),
        ("--hover", "#EFEFEC", "#222226"),
        ("--accent", "#3355D6", "#8EA4FF"),
        ("--accent-ink", "#FFFFFF", "#0D1330"),
        ("--accent-weak", "#ECF0FD", "#1D2340"),
        ("--warn", "#8A5300", "#F2B65A"),
        ("--warn-bg", "#FFF3DC", "#2B2112"),
        ("--bad", "#B42318", "#FF8A80"),
        ("--bad-bg", "#FDECEA", "#2E1615"),
        ("--ok", "#1B7346", "#6FD39E"),
        ("--off", "#5D5E66", "#A3A3AC"),
        ("--off-bg", "#ECECE9", "#26262A"),
    ] {
        for (sheet, value) in [(&light, day), (&dark, night)] {
            assert!(
                sheet.contains(&format!("{token}: {value}")),
                "{token}: {value}"
            );
        }
    }
    for (i, colour) in [
        "#475569", "#3355D6", "#0F766E", "#7C3AED", "#C2410C", "#B45309", "#0E7490", "#9F1239",
    ]
    .iter()
    .enumerate()
    {
        assert!(css.contains(&format!(".mono-{i} {{ background: {colour}; }}")));
    }
    for forbidden in ["https:", "http:", "@import", "@font-face"] {
        assert!(!css.contains(forbidden));
    }
    for url in css.split("url(").skip(1) {
        let url = url
            .split(')')
            .next()
            .unwrap()
            .trim()
            .trim_matches(['\'', '"']);
        assert!(
            !url.starts_with("//") && !url.contains(':'),
            "external URL: {url}"
        );
    }
    for required in [
        "system-ui, sans-serif",
        "prefers-reduced-motion: reduce",
        ".btn.primary",
        ".btn.quiet",
        ":focus-visible",
    ] {
        assert!(css.contains(required), "{required}");
    }
    for entry in fs::read_dir(root.join("ui")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "html") {
            let html = fs::read_to_string(&path).unwrap().to_lowercase();
            assert!(
                !html
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect::<String>()
                    .contains("style="),
                "{}",
                path.display()
            );
        }
    }
}
