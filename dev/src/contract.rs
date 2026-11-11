use crate::{
    json_eq::json_eq,
    outcome::{Code, Problem},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

pub struct ContractFile {
    pub fakes: watane_host::Fakes,
    pub steps: Vec<Step>,
}
pub struct Step {
    pub target: Target,
    pub args: Value,
    pub expect: Expect,
    pub emits: Option<Vec<ExpectedEvent>>,
}
pub enum Target {
    Call(String),
}
pub enum Expect {
    Success,
    Returns(Value),
    Fails(Option<String>),
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedEvent {
    pub event: String,
    pub args: Value,
}
#[derive(Default, Serialize)]
pub struct ContractReport {
    pub files: u32,
    pub passed: u32,
    pub failed: u32,
    pub failures: Vec<ContractFailure>,
}
#[derive(Debug, Serialize)]
pub struct ContractFailure {
    pub file: String,
    pub step: u32,
    pub call: String,
    pub message: String,
    pub expected: Option<Value>,
    pub got: Option<Value>,
}

fn object(value: Value) -> Result<Map<String, Value>, String> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err("expected an object".into()),
    }
}

fn no_unknown(map: &Map<String, Value>) -> Result<(), String> {
    if let Some(key) = map.keys().next() {
        Err(format!("unknown field `{key}`"))
    } else {
        Ok(())
    }
}

pub fn parse(text: &str) -> Result<ContractFile, String> {
    let mut file = object(serde_json::from_str(text).map_err(|e| e.to_string())?)?;
    let fakes: watane_host::Fakes = match file.remove("fakes") {
        Some(value) => serde_json::from_value(value).map_err(|e| e.to_string())?,
        None => Default::default(),
    };
    if let Some(key) = fakes.keys().find(|key| key.contains('@')) {
        return Err(format!("fakes key `{key}` must have no @version"));
    }
    let steps = match file.remove("steps") {
        Some(Value::Array(steps)) if !steps.is_empty() => steps,
        _ => return Err("steps must be a nonempty array".into()),
    };
    no_unknown(&file)?;
    let steps = steps
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_step(value).map_err(|e| format!("step {}: {e}", index + 1)))
        .collect::<Result<_, _>>()?;
    Ok(ContractFile { fakes, steps })
}

fn parse_step(value: Value) -> Result<Step, String> {
    let mut step = object(value)?;
    let call = step.remove("call");
    let deliver = step.remove("deliver");
    if call.is_some() && deliver.is_some() {
        return Err("exactly one of call and deliver is required".into());
    }
    if deliver.is_some() {
        return Err("deliver steps arrive with links between mods (step 2)".into());
    }
    let target = match call {
        Some(Value::String(name)) if !name.is_empty() => Target::Call(name),
        _ => return Err("call must name an export function".into()),
    };
    let args = step.remove("args").unwrap_or_else(|| json!({}));
    if !args.is_object() && !args.is_array() {
        return Err("args must be an object or array".into());
    }
    let returns = step.remove("returns");
    let fails = step.remove("fails");
    if returns.is_some() && fails.is_some() {
        return Err("returns and fails cannot both be set".into());
    }
    let expect = match (returns, fails) {
        (Some(value), None) => Expect::Returns(value),
        (None, Some(Value::Bool(true))) => Expect::Fails(None),
        (None, Some(Value::String(text))) => Expect::Fails(Some(text)),
        (None, None) => Expect::Success,
        _ => return Err("fails must be true or a string".into()),
    };
    let emits: Option<Vec<ExpectedEvent>> = step
        .remove("emits")
        .map(|value| serde_json::from_value(value).map_err(|e| e.to_string()))
        .transpose()?;
    if emits
        .as_ref()
        .is_some_and(|events| events.iter().any(|e| !e.args.is_object()))
    {
        return Err("event args must be an object".into());
    }
    no_unknown(&step)?;
    Ok(Step {
        target,
        args,
        expect,
        emits,
    })
}

pub fn run_steps(
    file: &str,
    steps: &[Step],
    slug: &str,
    call: &mut dyn FnMut(&Target, Value) -> Result<watane_host::Outcome, watane_host::CallError>,
) -> (u32, Vec<ContractFailure>) {
    let mut passed = 0;
    let mut failures = vec![];
    for (index, step) in steps.iter().enumerate() {
        let outcome = call(&step.target, step.args.clone());
        let mismatch = match (&step.expect, &outcome) {
            (Expect::Returns(expected), Ok(outcome)) if !json_eq(expected, &outcome.value) => {
                Some((expected.clone(), outcome.value.clone()))
            }
            (Expect::Fails(expected), Ok(outcome)) => Some((
                json!(expected.as_deref().unwrap_or("failure")),
                outcome.value.clone(),
            )),
            (expect, Err(error)) => {
                // Use the host's serialized code instead of duplicating its enum mapping.
                let encoded = serde_json::to_value(error).expect("CallError serializes");
                let got = format!(
                    "{}: {}",
                    encoded["code"].as_str().expect("error code"),
                    error.message
                );
                match expect {
                    Expect::Fails(text) if text.as_ref().is_none_or(|s| got.contains(s)) => None,
                    Expect::Fails(text) => Some((json!(text), json!(got))),
                    Expect::Returns(value) => Some((value.clone(), json!(got))),
                    Expect::Success => Some((json!("success"), json!(got))),
                }
            }
            _ => None,
        };
        // A failed call has no events. Omitted emits makes no assertion about events.
        let mismatch = mismatch.or_else(|| {
            let expected = step.emits.as_ref()?;
            let got: Vec<ExpectedEvent> = outcome
                .as_ref()
                .ok()
                .map(|o| {
                    o.events
                        .iter()
                        .map(|e| {
                            let interface = e.interface.to_string();
                            ExpectedEvent {
                                event: format!(
                                    "{}.{}",
                                    interface.split('@').next().unwrap(),
                                    e.function
                                ),
                                args: e.args.clone(),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let same = expected.len() == got.len()
                && expected.iter().zip(&got).all(|(a, b)| {
                    let name = if a.event.starts_with("mods:") {
                        a.event.clone()
                    } else {
                        format!("mods:{slug}/{}", a.event)
                    };
                    name == b.event && json_eq(&a.args, &b.args)
                });
            (!same).then(|| (json!(expected), json!(got)))
        });
        if let Some((expected, got)) = mismatch {
            let Target::Call(name) = &step.target;
            let number = index as u32 + 1;
            failures.push(ContractFailure {
                file: file.into(),
                step: number,
                call: name.clone(),
                message: format!("{file} step {number} ({name}): expected {expected}, got {got}"),
                expected: Some(expected),
                got: Some(got),
            });
        } else {
            passed += 1;
        }
    }
    (passed, failures)
}

pub fn contract_tests(
    dir: &Path,
    slug: &watane_pkg::Slug,
) -> Result<(ContractReport, Vec<Problem>), Vec<Problem>> {
    let entries = fs::read_dir(dir.join("tests")).map_err(|e| vec![Problem::internal(e)])?;
    let mut paths = vec![];
    for entry in entries {
        let entry = entry.map_err(|e| vec![Problem::internal(e)])?;
        if entry.path().extension().is_some_and(|e| e == "json")
            && entry
                .file_type()
                .map_err(|e| vec![Problem::internal(e)])?
                .is_file()
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    let mut report = ContractReport::default();
    let mut errors = vec![];
    for path in paths {
        report.files += 1;
        let file = format!("tests/{}", path.file_name().unwrap().to_string_lossy());
        let text = fs::read_to_string(&path)
            .map_err(|e| vec![Problem::internal(e).at(&file, None, None)])?;
        let contract = match parse(&text) {
            Ok(contract) => contract,
            Err(message) => {
                let location = serde_json::from_str::<Value>(&text).err();
                errors.push(
                    Problem::new(Code::TestContractFile, format!("{file}: {message}"))
                        .at(
                            &file,
                            location.as_ref().map(|e| e.line() as u32),
                            location.as_ref().map(|e| e.column() as u32),
                        )
                        .fix("See the contract test form in AGENTS.md"),
                );
                continue;
            }
        };
        let failures = match watane_host::Harness::open(dir, contract.fakes) {
            Ok(harness) => {
                let (passed, failures) = run_steps(
                    &file,
                    &contract.steps,
                    slug.as_str(),
                    &mut |target, args| {
                        let Target::Call(name) = target;
                        harness.call(name, args)
                    },
                );
                report.passed += passed;
                failures
            }
            Err(error) => {
                errors.push(
                    Problem::new(
                        Code::TestContract,
                        format!("{file}: cannot open contract host: {error}"),
                    )
                    .at(&file, None, None)
                    .fix("Correct the mod, then run: watane-dev test --json"),
                );
                continue;
            }
        };
        for failure in failures {
            errors.push(
                Problem::new(Code::TestContract, &failure.message)
                    .at(&file, None, None)
                    .fix(
                        "Correct the contract test or its logic, then run: watane-dev test --json",
                    ),
            );
            report.failed += 1;
            report.failures.push(failure);
        }
    }
    Ok((report, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use watane_host::{CallError, EventRecord, Outcome};

    #[test]
    fn ac6_parse_rejects_literal_invalid_files() {
        for text in [
            "{",
            "[]",
            "{}",
            r#"{"steps":[]}"#,
            r#"{"steps":[{"call":"api.get"}],"extra":true}"#,
            r#"{"steps":[{"call":"api.get","extra":true}]}"#,
            r#"{"steps":[{"call":"api.get","deliver":"changes.counted"}]}"#,
            r#"{"steps":[{"call":"api.get","returns":null,"fails":true}]}"#,
            r#"{"steps":[{}]}"#,
            r#"{"steps":[{"call":1}]}"#,
            r#"{"steps":[{"call":"api.get","fails":false}]}"#,
            r#"{"steps":[{"call":"api.get","args":1}]}"#,
            r#"{"steps":[{"call":"api.get","emits":[{"event":"changes.counted","args":{},"extra":0}]}]}"#,
            r#"{"steps":[{"call":"api.get","emits":[{"event":"changes.counted"}]}]}"#,
            r#"{"fakes":{"mods:books/api@0.1.0":{}},"steps":[{"call":"api.get"}]}"#,
        ] {
            assert!(parse(text).is_err(), "accepted {text}");
        }
        let err = parse(r#"{"steps":[{"deliver":"changes.counted"}]}"#)
            .err()
            .unwrap();
        assert!(
            err.contains("deliver steps arrive with links between mods (step 2)"),
            "{err}"
        );
    }

    #[test]
    fn ac6_parse_accepts_all_s4_field_forms_and_fakes() {
        let file = parse(
            r#"{"fakes":{"mods:books/api":{"all":["Dune"]}},"steps":[
            {"call":"api.get","returns":null},
            {"call":"api.add","args":[1],"fails":true},
            {"call":"api.add","args":{"count":1},"fails":"trap"},
            {"call":"api.get","emits":[{"event":"changes.counted","args":{"count":1}}]},
            {"call":"api.get"}
        ]}"#,
        )
        .unwrap();
        assert_eq!(file.fakes["mods:books/api"]["all"], json!(["Dune"]));
        assert_eq!(file.steps.len(), 5);
        assert_eq!(file.steps[0].args, json!({}));
        assert!(matches!(file.steps[0].expect, Expect::Returns(Value::Null)));
        assert!(matches!(file.steps[1].expect, Expect::Fails(None)));
        assert!(matches!(&file.steps[2].expect, Expect::Fails(Some(s)) if s == "trap"));
        assert!(matches!(file.steps[4].expect, Expect::Success));
    }

    #[test]
    fn ac7_run_steps_uses_closure_arguments_and_continues_after_failures() {
        let steps = parse(
            r#"{"steps":[
            {"call":"api.add","args":{"count":3},"returns":1},
            {"call":"api.get","returns":2},
            {"call":"api.get","returns":{"a":[1.0]}},
            {"call":"api.get"}
        ]}"#,
        )
        .unwrap()
        .steps;
        let mut calls = 0;
        let (passed, failures) = run_steps(
            "tests/literal.json",
            &steps,
            "counter",
            &mut |target, args| {
                calls += 1;
                let Target::Call(name) = target;
                if calls == 1 {
                    assert_eq!(name, "api.add");
                    assert_eq!(args, json!({"count":3}));
                } else {
                    assert_eq!(name, "api.get");
                    assert_eq!(args, json!({}));
                }
                Ok(Outcome {
                    value: if calls == 3 {
                        json!({"a":[1]})
                    } else {
                        json!(1)
                    },
                    events: vec![],
                })
            },
        );
        assert_eq!(calls, 4);
        assert_eq!(passed, 3);
        assert_eq!(failures.len(), 1);
        let failure = &failures[0];
        assert_eq!(failure.file, "tests/literal.json");
        assert_eq!(failure.step, 2);
        assert_eq!(failure.call, "api.get");
        assert_eq!(failure.expected, Some(json!(2)));
        assert_eq!(failure.got, Some(json!(1)));
        assert_eq!(
            failure.message,
            "tests/literal.json step 2 (api.get): expected 2, got 1"
        );
    }

    #[test]
    fn ac5_run_steps_fails_boolean_and_substring_and_unexpected_results() {
        for (expect, succeeds, passes) in [
            (Expect::Fails(None), false, true),
            (Expect::Fails(Some("trap: deliberate".into())), false, true),
            (Expect::Fails(Some("absent".into())), false, false),
            (Expect::Fails(None), true, false),
            (Expect::Success, false, false),
            (Expect::Returns(json!(1)), false, false),
        ] {
            let step = Step {
                target: Target::Call("api.add".into()),
                args: json!({}),
                expect,
                emits: None,
            };
            let (passed, failures) =
                run_steps("tests/counter.json", &[step], "counter", &mut |_, _| {
                    if succeeds {
                        Ok(Outcome {
                            value: json!(1),
                            events: vec![],
                        })
                    } else {
                        Err(CallError {
                            code: watane_host::CallCode::Trap,
                            message: "deliberate failure".into(),
                        })
                    }
                });
            assert_eq!(passed, u32::from(passes));
            assert_eq!(failures.len(), usize::from(!passes));
        }
    }

    #[test]
    fn ac4_run_steps_events_exact_order_names_and_failed_calls() {
        for (emits, fails, passes) in [
            (
                json!([{"event":"changes.counted","args":{"count":1.0}}, {"event":"mods:counter/changes.counted","args":{"count":2}}]),
                false,
                true,
            ),
            (
                json!([{"event":"changes.counted","args":{"count":2}}, {"event":"changes.counted","args":{"count":1}}]),
                false,
                false,
            ),
            (
                json!([{"event":"mods:other/changes.counted","args":{"count":1}}]),
                false,
                false,
            ),
            (json!([]), true, true),
            (
                json!([{"event":"changes.counted","args":{"count":1}}]),
                true,
                false,
            ),
        ] {
            let mut step = json!({"call":"api.add", "emits":emits});
            if fails {
                step["fails"] = json!(true);
            }
            let steps = parse(&json!({"steps":[step]}).to_string()).unwrap().steps;
            let (passed, failures) =
                run_steps("tests/counter.json", &steps, "counter", &mut |_, _| {
                    if fails {
                        return Err(CallError {
                            code: watane_host::CallCode::Trap,
                            message: "failed".into(),
                        });
                    }
                    Ok(Outcome {
                        value: json!(2),
                        events: [1, 2]
                            .into_iter()
                            .map(|count| EventRecord {
                                interface: "mods:counter/changes@0.1.0".parse().unwrap(),
                                function: "counted".into(),
                                args: json!({"count":count}),
                            })
                            .collect(),
                    })
                });
            assert_eq!(passed, u32::from(passes), "{failures:?}");
            assert_eq!(failures.len(), usize::from(!passes));
            if !passes {
                assert!(failures[0].expected.as_ref().unwrap().is_array());
                assert!(failures[0].got.as_ref().unwrap().is_array());
            }
        }
    }

    #[test]
    fn ac6_open_error_is_one_file_problem_without_a_synthetic_step() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("tests")).unwrap();
        fs::write(
            temp.path().join("tests/a.json"),
            r#"{"steps":[{"call":"api.get"},{"call":"api.add"}]}"#,
        )
        .unwrap();
        let (report, errors) = contract_tests(temp.path(), &"counter".parse().unwrap()).unwrap();
        assert_eq!(report.files, 1);
        assert_eq!(report.failed, 0);
        assert!(report.failures.is_empty());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, Code::TestContract);
        assert_eq!(errors[0].file.as_deref(), Some("tests/a.json"));
    }
}
