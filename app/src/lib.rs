#[cfg(all(feature = "driver", not(debug_assertions)))]
compile_error!("the driver feature is only for debug test builds");

pub mod consent;
pub mod mark;
