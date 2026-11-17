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

#[cfg(feature = "real-github")]
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
