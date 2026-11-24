//! `doctor`: reports every gap in the maker's environment, not only the first
//! (TECHNICAL 2.4).

use crate::allowlist::{self, Allowlist};
use crate::api::Client;
use crate::env::Env;
use crate::outcome::{Outcome, Problem};
use crate::{preview, toolchain};
use serde_json::{Value, json};
use std::path::Path;

pub fn doctor(dir: &Path) -> Outcome {
    let mut outcome = Outcome::new("doctor", None);
    let mut errors = Vec::new();
    let toolchain = match Env::from_process().and_then(|env| toolchain::discover(&env)) {
        Ok(toolchain) => {
            let target = toolchain.check_target();
            let present = target.is_ok();
            if let Err(error) = target {
                errors.push(error);
            }
            json!({"cargo":toolchain.cargo,"rustc":toolchain.rustc,
                "sysroot":toolchain.sysroot,"version":toolchain.version,"wasm32_wasip2":present})
        }
        Err(error) => {
            errors.push(error);
            Value::Null
        }
    };
    let allowlist = match allowlist::vendor_dir().and_then(|vendor| {
        allowlist::verify_vendor(&vendor, &Allowlist::embedded())?;
        Ok(vendor)
    }) {
        Ok(vendor) => {
            let allow = Allowlist::embedded();
            json!({"vendor":vendor,"direct":allow.direct().iter()
                .map(|(name, version)| format!("{name} {version}")).collect::<Vec<_>>(),
                "crates":allow.package_count()})
        }
        Err(error) => {
            errors.push(error);
            Value::Null
        }
    };
    let watane = match preview::find_watane() {
        Ok(path) => json!(path),
        Err(error) => {
            errors.push(error);
            Value::Null
        }
    };
    let fake_claude = match preview::find_fake_claude() {
        Ok(path) => json!(path),
        Err(warning) => {
            outcome.warnings.push(warning);
            Value::Null
        }
    };
    let home = match watane_host::default_home() {
        Ok(path) => {
            let status = Client::home_status(&path);
            json!({"path":path,"running":status.is_some(),
                "app":status.is_some_and(|s| s["app"] == true)})
        }
        Err(error) => {
            errors.push(Problem::internal(error));
            Value::Null
        }
    };
    outcome.result = Some(json!({"toolchain":toolchain,"allowlist":allowlist,
        "watane":watane,"fake_claude":fake_claude,"home":home}));
    outcome.next = Some(doctor_next(dir, errors.is_empty()).into());
    if errors.is_empty() {
        outcome
    } else {
        outcome.fail(errors)
    }
}

fn doctor_next(dir: &Path, ok: bool) -> &'static str {
    if !ok {
        "Run each error's fix, then run: watane-dev doctor --json"
    } else if dir.join("mod.toml").is_file() {
        "watane-dev serve --json"
    } else {
        "watane-dev new <slug> --json"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac3_doctor_next_serves_existing_mod_and_creates_outside() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            doctor_next(dir.path(), true),
            "watane-dev new <slug> --json"
        );
        std::fs::write(dir.path().join("mod.toml"), "").unwrap();
        assert_eq!(doctor_next(dir.path(), true), "watane-dev serve --json");
        assert_eq!(
            doctor_next(dir.path(), false),
            "Run each error's fix, then run: watane-dev doctor --json"
        );
    }

    #[test]
    fn ac7_fake_claude_field_and_warning() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("nope");
        let outcome =
            crate::env::with_vars(&[("WATANE_FAKE_CLAUDE", Some(missing.as_os_str()))], || {
                doctor(temp.path())
            });
        let json = serde_json::to_value(&outcome).unwrap();
        assert!(json["result"]["fake_claude"].is_null());
        assert_eq!(json["warnings"][0]["code"], "preview.fake_claude_missing");
        let program = temp
            .path()
            .join(crate::toolchain::program_name("fake-claude"));
        std::fs::write(&program, "x").unwrap();
        #[cfg(unix)] // A Unix executable fixture needs execute bits.
        std::fs::set_permissions(
            &program,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        let outcome =
            crate::env::with_vars(&[("WATANE_FAKE_CLAUDE", Some(program.as_os_str()))], || {
                doctor(temp.path())
            });
        let json = serde_json::to_value(&outcome).unwrap();
        assert_eq!(
            json["result"]["fake_claude"],
            json!(program.canonicalize().unwrap())
        );
        assert!(json["warnings"].as_array().is_none_or(Vec::is_empty));
    }
}
