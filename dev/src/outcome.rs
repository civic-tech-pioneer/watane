use serde::Serialize;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum Code {
    UsageArgs,
    UsageNotAMod,
    UsageSlug,
    UsageExists,
    UsageProvider,
    ModSelfDependency,
    ModDependencyCycle,
    ModProviderMissing,
    EnvWataneMissing,
    EnvPreviewStart,
    ModInvalid,
    ModSymlink,
    HostDeclined,
    HostRefused,
    HostNoAnswer,
    EnvToolchain,
    EnvNoWasmTarget,
    EnvAllowlist,
    ModCargoKey,
    ModDependency,
    ModLogicFile,
    ModNoContractTests,
    TestUnit,
    TestUnitLimit,
    TestContractFile,
    TestContract,
    BuildResolve,
    BuildCompile,
    BuildReadsOutside,
    BuildImports,
    PreviewPrevious,
    Internal,
}
impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UsageArgs => "usage.args",
            Self::UsageNotAMod => "usage.not_a_mod",
            Self::UsageSlug => "usage.slug",
            Self::UsageExists => "usage.exists",
            Self::UsageProvider => "usage.provider",
            Self::ModSelfDependency => "mod.self_dependency",
            Self::ModDependencyCycle => "mod.dependency_cycle",
            Self::ModProviderMissing => "mod.provider_missing",
            Self::EnvWataneMissing => "env.watane_missing",
            Self::EnvPreviewStart => "env.preview_start",
            Self::ModInvalid => "mod.invalid",
            Self::ModSymlink => "mod.symlink",
            Self::HostDeclined => "host.declined",
            Self::HostRefused => "host.refused",
            Self::HostNoAnswer => "host.no_answer",
            Self::EnvToolchain => "env.toolchain",
            Self::EnvNoWasmTarget => "env.no_wasm_target",
            Self::EnvAllowlist => "env.allowlist",
            Self::ModCargoKey => "mod.cargo_key",
            Self::ModDependency => "mod.dependency",
            Self::ModLogicFile => "mod.logic_file",
            Self::ModNoContractTests => "mod.no_contract_tests",
            Self::TestUnit => "test.unit",
            Self::TestUnitLimit => "test.unit_limit",
            Self::TestContractFile => "test.contract_file",
            Self::TestContract => "test.contract",
            Self::BuildResolve => "build.resolve",
            Self::BuildCompile => "build.compile",
            Self::BuildReadsOutside => "build.reads_outside",
            Self::BuildImports => "build.imports",
            Self::PreviewPrevious => "preview.previous",
            Self::Internal => "internal",
        }
    }
    pub fn class(self) -> Class {
        match self {
            Self::PreviewPrevious => Class::Warning,
            Self::UsageArgs
            | Self::UsageNotAMod
            | Self::UsageSlug
            | Self::UsageExists
            | Self::UsageProvider => Class::Usage,
            Self::EnvWataneMissing
            | Self::EnvPreviewStart
            | Self::EnvToolchain
            | Self::EnvNoWasmTarget
            | Self::EnvAllowlist => Class::Env,
            Self::ModInvalid
            | Self::ModSelfDependency
            | Self::ModDependencyCycle
            | Self::ModProviderMissing
            | Self::ModSymlink
            | Self::ModCargoKey
            | Self::ModDependency
            | Self::ModLogicFile
            | Self::ModNoContractTests
            | Self::TestUnit
            | Self::TestUnitLimit
            | Self::TestContractFile
            | Self::TestContract
            | Self::BuildResolve
            | Self::BuildCompile
            | Self::BuildReadsOutside
            | Self::BuildImports => Class::Mod,
            Self::HostDeclined | Self::HostRefused | Self::HostNoAnswer => Class::Refused,
            Self::Internal => Class::Internal,
        }
    }
}
impl From<Code> for &'static str {
    fn from(code: Code) -> Self {
        code.as_str()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    Warning = 0,
    Mod = 1,
    Usage = 2,
    Env = 3,
    Refused = 4,
    Internal = 5,
}
#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    pub code: Code,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub fix: Option<String>,
}
impl Problem {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            fix: None,
        }
    }
    pub fn at(mut self, file: impl Into<String>, line: Option<u32>, column: Option<u32>) -> Self {
        self.file = Some(file.into());
        self.line = line;
        self.column = column;
        self
    }
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
    pub fn internal(error: impl ToString) -> Self {
        Self::new(Code::Internal, error.to_string())
            .fix("Check the paths and permissions, then run the command again")
    }
}
#[derive(Debug, Serialize)]
pub struct Outcome {
    pub schema: u32,
    pub command: &'static str,
    pub ok: bool,
    pub dir: Option<PathBuf>,
    pub result: Option<serde_json::Value>,
    pub errors: Vec<Problem>,
    pub warnings: Vec<Problem>,
    pub next: Option<String>,
}
impl Outcome {
    pub fn new(command: &'static str, dir: Option<PathBuf>) -> Self {
        Self {
            schema: 1,
            command,
            ok: true,
            dir,
            result: None,
            errors: vec![],
            warnings: vec![],
            next: None,
        }
    }
    pub fn fail(mut self, errors: Vec<Problem>) -> Self {
        self.ok = false;
        self.errors = errors;
        self
    }
    pub fn exit_code(&self) -> u8 {
        self.errors.first().map_or(0, |e| e.code.class() as u8)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("outcome serializes")
    }
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for (kind, items) in [("error", &self.errors), ("warning", &self.warnings)] {
            for e in items {
                text.push_str(&format!("{kind}[{}]: {}\n", e.code.as_str(), e.message));
                if let Some(f) = &e.file {
                    text.push_str(&format!("  --> {f}"));
                    if let Some(l) = e.line {
                        text.push_str(&format!(":{l}"));
                        if let Some(c) = e.column {
                            text.push_str(&format!(":{c}"));
                        }
                    }
                    text.push('\n');
                }
                if let Some(f) = &e.fix {
                    text.push_str(&format!("  fix: {f}\n"));
                }
            }
        }
        text.push_str(&format!(
            "{}: {}\n",
            self.command,
            if self.ok { "ok" } else { "failed" }
        ));
        if let Some(serde_json::Value::Object(fields)) = &self.result {
            for (k, v) in fields {
                text_fields(k, v, &mut text);
            }
        }
        if let Some(next) = &self.next {
            text.push_str(&format!("next: {next}\n"));
        }
        text
    }
}

fn text_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "missing".into(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => {
            items.iter().map(text_value).collect::<Vec<_>>().join(", ")
        }
        serde_json::Value::Object(fields) => fields
            .iter()
            .map(|(k, v)| format!("{k}: {}", text_value(v)))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    }
}
fn text_fields(key: &str, value: &serde_json::Value, text: &mut String) {
    if let serde_json::Value::Object(fields) = value {
        for (field, value) in fields {
            text_fields(&format!("{key}.{field}"), value, text);
        }
    } else {
        text.push_str(&format!("{key}: {}\n", text_value(value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac4_text_flattens_objects_and_formats_values() {
        let mut out = Outcome::new("doctor", None);
        out.result = Some(
            serde_json::json!({"allowlist":{"vendor":"/abs/vendor","direct":["serde 1", "wit 2"]}, "toolchain":{"wasm32_wasip2":true}, "watane":null}),
        );
        assert_eq!(
            out.to_text(),
            "doctor: ok\nallowlist.direct: serde 1, wit 2\nallowlist.vendor: /abs/vendor\ntoolchain.wasm32_wasip2: true\nwatane: missing\n"
        );
    }

    #[test]
    fn ac3_previous_preview_is_only_a_warning() {
        let mut outcome = Outcome::new("serve", None);
        outcome.warnings.push(Problem::new(
            Code::PreviewPrevious,
            "the preview at http://localhost:52011/api/ still serves the previous build",
        ));
        assert_eq!(Code::PreviewPrevious.class(), Class::Warning);
        assert!(outcome.ok);
        assert_eq!(outcome.exit_code(), 0);
        assert_eq!(
            serde_json::to_value(&outcome.warnings[0]).unwrap()["code"],
            "preview.previous"
        );
        let outcome = outcome.fail(vec![Problem::new(Code::TestContract, "broken contract")]);
        assert!(!outcome.ok);
        assert_eq!(outcome.exit_code(), 1);
    }
    #[test]
    fn ac3_exit_classes_and_problem_envelope() {
        use super::{Class, Code, Outcome, Problem};
        use serde_json::json;
        for (code, class, status, text) in [
            (Code::ModInvalid, Class::Mod, 1, "mod.invalid"),
            (Code::UsageArgs, Class::Usage, 2, "usage.args"),
            (Code::EnvWataneMissing, Class::Env, 3, "env.watane_missing"),
            (Code::HostRefused, Class::Refused, 4, "host.refused"),
            (Code::Internal, Class::Internal, 5, "internal"),
        ] {
            assert_eq!(code.class(), class);
            assert_eq!(code, code);
            assert_eq!(serde_json::to_value(code).unwrap(), text);
            let problem = Problem::new(code, "message").at("ui/app.js", Some(12), Some(5));
            let cloned = problem.clone();
            let outcome = Outcome::new("check", None).fail(vec![cloned]);
            let command: &'static str = outcome.command;
            assert_eq!(command, "check");
            assert_eq!(outcome.exit_code(), status);
            assert_eq!(
                serde_json::to_value(problem).unwrap(),
                json!({"code":text,"message":"message","file":"ui/app.js","line":12,"column":5,"fix":null})
            );
        }
    }
}
