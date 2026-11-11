use serde::Serialize;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "&'static str")]
pub enum Code {
    UsageArgs,
    UsageNotAMod,
    UsageSlug,
    UsageExists,
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
    BuildImports,
    Internal,
}
impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UsageArgs => "usage.args",
            Self::UsageNotAMod => "usage.not_a_mod",
            Self::UsageSlug => "usage.slug",
            Self::UsageExists => "usage.exists",
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
            Self::BuildImports => "build.imports",
            Self::Internal => "internal",
        }
    }
    pub fn class(self) -> Class {
        match self {
            Self::UsageArgs | Self::UsageNotAMod | Self::UsageSlug | Self::UsageExists => {
                Class::Usage
            }
            Self::EnvWataneMissing
            | Self::EnvPreviewStart
            | Self::EnvToolchain
            | Self::EnvNoWasmTarget
            | Self::EnvAllowlist => Class::Env,
            Self::ModInvalid
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
                text.push_str(&format!("{k}: {v}\n"));
            }
        }
        if let Some(next) = &self.next {
            text.push_str(&format!("next: {next}\n"));
        }
        text
    }
}
