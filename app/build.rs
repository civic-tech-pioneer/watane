fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "status",
            "mods_list",
            "mod_open",
            "mod_disable",
            "mod_enable",
            "mod_remove",
            "grant_withdraw",
            "grant_give",
            "install_folders",
            "tools_list",
            "toolsets_list",
            "toolset_save",
            "tokens_list",
            "token_issue",
            "token_end",
            "consent_get",
            "consent_answer",
        ]),
    ))
    .expect("cannot build app manifest");
}
