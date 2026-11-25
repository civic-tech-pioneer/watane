//! Select the development-only scripted OpenAI preview setting (TECHNICAL 19.9).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(watane_dev_profile)");
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        println!("cargo:rustc-cfg=watane_dev_profile");
    }
}
