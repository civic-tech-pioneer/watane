//! Select the development-only scripted OpenAI preview setting (TECHNICAL 19.9).

#![expect(
    clippy::print_stdout,
    reason = "Cargo reads build script directives from stdout"
)]

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(watane_dev_profile)");
    if std::env::var("PROFILE").as_deref() == Ok("debug") {
        println!("cargo:rustc-cfg=watane_dev_profile");
    }
}
