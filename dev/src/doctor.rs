use crate::{
    allowlist::{self, Allowlist},
    api::Client,
    env::Env,
    outcome::{Outcome, Problem},
    preview, toolchain,
};
use serde_json::{Value, json};

pub fn doctor() -> Outcome {
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
        "watane":watane,"home":home}));
    if errors.is_empty() {
        outcome.next = Some("watane-dev new <slug> --json".into());
        outcome
    } else {
        outcome.next = Some("Run each error's fix, then run: watane-dev doctor --json".into());
        outcome.fail(errors)
    }
}
