#[allow(
    dead_code,
    reason = "Each window test binary uses a different subset of the driver"
)]
pub mod driver;
#[allow(
    dead_code,
    reason = "gui_shell includes support but does not use fixture servers"
)]
pub mod pages;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod journey;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod publish;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod dev;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod github;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod node;

#[allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
pub mod manage;

/// Serializes the window tests of gui_shell, gui_isolation, j2 and manage. Each file had its own
/// lock when it was its own binary, and binaries never ran at the same time; one lock keeps that.
#[cfg(feature = "driver")]
pub static GUI: std::sync::Mutex<()> = std::sync::Mutex::new(());
