use crate::{CmdError, core::ModView};
#[derive(serde::Deserialize)]
struct Catalog {
    mods: Vec<ModView>,
}
pub(crate) fn read() -> Result<Vec<ModView>, CmdError> {
    match std::env::var("WATANE_APP_FAKE") {
        Ok(text) => serde_json::from_str::<Catalog>(&text)
            .map(|c| c.mods)
            .map_err(|e| CmdError::new("bad_input", format!("WATANE_APP_FAKE: {e}"))),
        Err(std::env::VarError::NotPresent) => Ok(Vec::new()),
        Err(e) => Err(CmdError::new("bad_input", format!("WATANE_APP_FAKE: {e}"))),
    }
}
