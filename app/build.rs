fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "status",
            "mods_list",
            "mod_open",
            "mod_disable",
            "consent_get",
            "consent_answer",
        ]),
    ))
    .expect("cannot build app manifest");
}
