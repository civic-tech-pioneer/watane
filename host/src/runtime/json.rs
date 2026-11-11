use crate::{CallCode, CallError};
use serde_json::{Value, json};
use wasmtime::component::{Type, Val, types::ComponentFunc};

fn bad(path: &str, reason: impl std::fmt::Display) -> CallError {
    CallError::new(CallCode::BadRequest, format!("{path}: {reason}"))
}
pub(super) fn callable(ty: &Type) -> bool {
    match ty {
        Type::Own(_)
        | Type::Borrow(_)
        | Type::Future(_)
        | Type::Stream(_)
        | Type::ErrorContext
        | Type::Map(_)
        | Type::FixedLengthList(_) => false,
        Type::Option(inner) => !matches!(inner.ty(), Type::Option(_)) && callable(&inner.ty()),
        Type::List(inner) => callable(&inner.ty()),
        Type::Tuple(tuple) => tuple.types().all(|t| callable(&t)),
        Type::Record(record) => record.fields().all(|f| callable(&f.ty)),
        Type::Variant(variant) => variant.cases().all(|c| c.ty.is_none_or(|t| callable(&t))),
        Type::Result(result) => {
            result.ok().is_none_or(|t| callable(&t)) && result.err().is_none_or(|t| callable(&t))
        }
        _ => true,
    }
}
pub(super) fn arguments(func: &ComponentFunc, args: Value) -> Result<Vec<Val>, CallError> {
    let params: Vec<_> = func.params().collect();
    match args {
        Value::Null => arguments(func, json!({})),
        Value::Object(object) => {
            for name in object.keys() {
                if !params.iter().any(|(n, _)| n == name) {
                    return Err(bad(&format!("$.{name}"), "unknown parameter"));
                }
            }
            params
                .into_iter()
                .map(|(name, ty)| {
                    decode(
                        &ty,
                        object.get(name).unwrap_or(&Value::Null),
                        &format!("$.{name}"),
                    )
                })
                .collect()
        }
        Value::Array(values) => {
            if values.len() != params.len() {
                return Err(bad("$", format!("expected {} arguments", params.len())));
            }
            params
                .iter()
                .zip(&values)
                .enumerate()
                .map(|(i, ((_, ty), v))| decode(ty, v, &format!("$[{i}]")))
                .collect()
        }
        _ => Err(bad("$", "expected object, array or null")),
    }
}
fn decode(ty: &Type, value: &Value, path: &str) -> Result<Val, CallError> {
    let expected = || {
        bad(
            path,
            format!(
                "expected {}",
                match ty {
                    Type::Bool => "bool",
                    Type::U8 => "u8",
                    Type::U16 => "u16",
                    Type::U32 => "u32",
                    Type::U64 => "u64",
                    Type::S8 => "s8",
                    Type::S16 => "s16",
                    Type::S32 => "s32",
                    Type::S64 => "s64",
                    Type::Float32 => "f32",
                    Type::Float64 => "f64",
                    Type::Char => "one Unicode scalar",
                    Type::String => "string",
                    Type::List(_) => "list",
                    Type::Tuple(_) => "tuple",
                    Type::Record(_) => "record",
                    Type::Option(_) => "option",
                    Type::Enum(_) => "enum",
                    Type::Flags(_) => "flags",
                    Type::Variant(_) => "variant",
                    Type::Result(_) => "result",
                    _ => "supported WIT value",
                }
            ),
        )
    };
    macro_rules! unsigned {
        ($native:ty,$variant:ident) => {
            Val::$variant(
                value
                    .as_u64()
                    .and_then(|v| <$native>::try_from(v).ok())
                    .ok_or_else(expected)?,
            )
        };
    }
    macro_rules! signed {
        ($native:ty,$variant:ident) => {
            Val::$variant(
                value
                    .as_i64()
                    .and_then(|v| <$native>::try_from(v).ok())
                    .ok_or_else(expected)?,
            )
        };
    }
    Ok(match ty {
        Type::Bool => Val::Bool(value.as_bool().ok_or_else(expected)?),
        Type::U8 => unsigned!(u8, U8),
        Type::U16 => unsigned!(u16, U16),
        Type::U32 => unsigned!(u32, U32),
        Type::U64 => unsigned!(u64, U64),
        Type::S8 => signed!(i8, S8),
        Type::S16 => signed!(i16, S16),
        Type::S32 => signed!(i32, S32),
        Type::S64 => signed!(i64, S64),
        Type::Float32 => Val::Float32(value.as_f64().ok_or_else(expected)? as f32),
        Type::Float64 => Val::Float64(value.as_f64().ok_or_else(expected)?),
        Type::String => Val::String(value.as_str().ok_or_else(expected)?.into()),
        Type::Char => {
            let mut chars = value.as_str().ok_or_else(expected)?.chars();
            let c = chars.next().ok_or_else(expected)?;
            if chars.next().is_some() {
                return Err(expected());
            }
            Val::Char(c)
        }
        Type::List(list) => Val::List(
            value
                .as_array()
                .ok_or_else(expected)?
                .iter()
                .enumerate()
                .map(|(i, v)| decode(&list.ty(), v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        Type::Tuple(tuple) => {
            let values = value.as_array().ok_or_else(expected)?;
            if values.len() != tuple.types().len() {
                return Err(bad(
                    path,
                    format!("expected tuple of length {}", tuple.types().len()),
                ));
            }
            Val::Tuple(
                tuple
                    .types()
                    .zip(values)
                    .enumerate()
                    .map(|(i, (ty, v))| decode(&ty, v, &format!("{path}[{i}]")))
                    .collect::<Result<_, _>>()?,
            )
        }
        Type::Record(record) => {
            let values = value.as_object().ok_or_else(expected)?;
            for name in values.keys() {
                if !record.fields().any(|f| f.name == name) {
                    return Err(bad(&format!("{path}.{name}"), "unknown record field"));
                }
            }
            Val::Record(
                record
                    .fields()
                    .map(|f| {
                        Ok((
                            f.name.into(),
                            decode(
                                &f.ty,
                                values.get(f.name).unwrap_or(&Value::Null),
                                &format!("{path}.{}", f.name),
                            )?,
                        ))
                    })
                    .collect::<Result<_, CallError>>()?,
            )
        }
        Type::Option(option) => Val::Option(if value.is_null() {
            None
        } else {
            Some(Box::new(decode(&option.ty(), value, path)?))
        }),
        Type::Enum(enumeration) => {
            let name = value.as_str().ok_or_else(expected)?;
            if !enumeration.names().any(|n| n == name) {
                return Err(bad(
                    path,
                    format!(
                        "expected enum {{{}}}",
                        enumeration.names().collect::<Vec<_>>().join(", ")
                    ),
                ));
            }
            Val::Enum(name.into())
        }
        Type::Flags(flags) => {
            let mut names = std::collections::BTreeSet::new();
            for (i, value) in value.as_array().ok_or_else(expected)?.iter().enumerate() {
                let p = format!("{path}[{i}]");
                let name = value
                    .as_str()
                    .ok_or_else(|| bad(&p, "expected flag name"))?;
                if !flags.names().any(|n| n == name) {
                    return Err(bad(&p, "unknown flag"));
                }
                if !names.insert(name) {
                    return Err(bad(&p, "duplicate flag"));
                }
            }
            Val::Flags(
                flags
                    .names()
                    .filter(|n| names.contains(n))
                    .map(str::to_owned)
                    .collect(),
            )
        }
        Type::Variant(variant) => {
            if let Some(name) = value.as_str() {
                if !variant.cases().any(|c| c.name == name && c.ty.is_none()) {
                    return Err(expected());
                }
                Val::Variant(name.into(), None)
            } else {
                let object = value
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .ok_or_else(expected)?;
                let (name, payload) = object.iter().next().expect("one case");
                let case = variant
                    .cases()
                    .find(|c| c.name == name)
                    .ok_or_else(expected)?;
                let ty = case
                    .ty
                    .ok_or_else(|| bad(path, "case without payload must be a string"))?;
                Val::Variant(
                    name.clone(),
                    Some(Box::new(decode(&ty, payload, &format!("{path}.{name}"))?)),
                )
            }
        }
        Type::Result(result) => {
            let object = value
                .as_object()
                .filter(|o| o.len() == 1)
                .ok_or_else(expected)?;
            let (name, value) = object.iter().next().expect("one result case");
            let ty = match name.as_str() {
                "ok" => result.ok(),
                "err" => result.err(),
                _ => return Err(expected()),
            };
            let p = format!("{path}.{name}");
            let payload = match ty {
                Some(ty) => Some(Box::new(decode(&ty, value, &p)?)),
                None if value.is_null() => None,
                None => return Err(bad(&p, "expected null for a case without payload")),
            };
            Val::Result(if name == "ok" {
                Ok(payload)
            } else {
                Err(payload)
            })
        }
        _ => {
            return Err(CallError::new(
                CallCode::NoFunction,
                "function has an unsupported WIT type",
            ));
        }
    })
}
pub(super) fn encode(value: &Val) -> Result<Value, CallError> {
    Ok(match value {
        Val::Bool(v) => json!(v),
        Val::U8(v) => json!(v),
        Val::U16(v) => json!(v),
        Val::U32(v) => json!(v),
        Val::U64(v) => json!(v),
        Val::S8(v) => json!(v),
        Val::S16(v) => json!(v),
        Val::S32(v) => json!(v),
        Val::S64(v) => json!(v),
        Val::Float32(v) => json!(v),
        Val::Float64(v) => json!(v),
        Val::Char(v) => json!(v.to_string()),
        Val::String(v) => json!(v),
        Val::List(v) | Val::Tuple(v) => {
            Value::Array(v.iter().map(encode).collect::<Result<_, _>>()?)
        }
        Val::Record(v) => Value::Object(
            v.iter()
                .map(|(name, v)| Ok((name.clone(), encode(v)?)))
                .collect::<Result<_, CallError>>()?,
        ),
        Val::Enum(v) => json!(v),
        Val::Flags(v) => json!(v),
        Val::Option(None) => Value::Null,
        Val::Option(Some(v)) => encode(v)?,
        Val::Variant(name, None) => json!(name),
        Val::Variant(name, Some(v)) => json!({name:encode(v)?}),
        Val::Result(result) => {
            let (case, value) = match result {
                Ok(v) => ("ok", v),
                Err(v) => ("err", v),
            };
            json!({case:match value {Some(v)=>encode(v)?,None=>Value::Null}})
        }
        _ => {
            return Err(CallError::new(
                CallCode::NoFunction,
                "function has an unsupported WIT result",
            ));
        }
    })
}

pub(super) fn type_text(ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::U64 => "u64".into(),
        Type::S8 => "s8".into(),
        Type::S16 => "s16".into(),
        Type::S32 => "s32".into(),
        Type::S64 => "s64".into(),
        Type::Float32 => "f32".into(),
        Type::Float64 => "f64".into(),
        Type::Char => "char".into(),
        Type::String => "string".into(),
        Type::List(t) => format!("list<{}>", type_text(&t.ty())),
        Type::Option(t) => format!("option<{}>", type_text(&t.ty())),
        Type::Tuple(t) => format!(
            "tuple<{}>",
            t.types()
                .map(|t| type_text(&t))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Record(t) => format!(
            "record {{ {} }}",
            t.fields()
                .map(|f| format!("{}: {}", f.name, type_text(&f.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Enum(t) => format!("enum {{ {} }}", t.names().collect::<Vec<_>>().join(", ")),
        Type::Flags(t) => format!("flags {{ {} }}", t.names().collect::<Vec<_>>().join(", ")),
        Type::Variant(t) => format!(
            "variant {{ {} }}",
            t.cases()
                .map(|c| match c.ty {
                    Some(t) => format!("{}({})", c.name, type_text(&t)),
                    None => c.name.into(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Type::Result(t) => format!(
            "result<{}, {}>",
            t.ok().map(|t| type_text(&t)).unwrap_or("_".into()),
            t.err().map(|t| type_text(&t)).unwrap_or("_".into())
        ),
        _ => unreachable!("only callable types have text"),
    }
}
pub(super) fn object_schema<'a>(fields: impl Iterator<Item = (&'a str, Type)>) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, ty) in fields {
        properties.insert(name.into(), schema(&ty));
        if !matches!(ty, Type::Option(_)) {
            required.push(name.to_owned());
        }
    }
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn case_schema(name: &str, ty: Option<Type>) -> Value {
    let payload = ty.as_ref().map(schema).unwrap_or(json!({"type":"null"}));
    json!({"type":"object","properties":{name:payload},"required":[name],"additionalProperties":false})
}
fn schema(ty: &Type) -> Value {
    match ty {
        Type::Bool => json!({"type":"boolean"}),
        Type::U8 => json!({"type":"integer","minimum":0,"maximum":255}),
        Type::U16 => json!({"type":"integer","minimum":0,"maximum":65535}),
        Type::U32 => json!({"type":"integer","minimum":0,"maximum":4294967295u32}),
        Type::U64 => json!({"type":"integer","minimum":0,"maximum":u64::MAX}),
        Type::S8 => json!({"type":"integer","minimum":-128,"maximum":127}),
        Type::S16 => json!({"type":"integer","minimum":-32768,"maximum":32767}),
        Type::S32 => json!({"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}),
        Type::S64 => json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}),
        Type::Float32 | Type::Float64 => json!({"type":"number"}),
        Type::String => json!({"type":"string"}),
        Type::Char => json!({"type":"string","minLength":1,"maxLength":1}),
        Type::List(t) => json!({"type":"array","items":schema(&t.ty())}),
        Type::Option(t) => json!({"anyOf":[{"type":"null"},schema(&t.ty())]}),
        Type::Tuple(t) => {
            let items: Vec<_> = t.types().map(|t| schema(&t)).collect();
            json!({"type":"array","prefixItems":items,"minItems":items.len(),"maxItems":items.len()})
        }
        Type::Record(t) => object_schema(t.fields().map(|f| (f.name, f.ty))),
        Type::Enum(t) => json!({"type":"string","enum":t.names().collect::<Vec<_>>()}),
        Type::Flags(t) => {
            json!({"type":"array","items":{"type":"string","enum":t.names().collect::<Vec<_>>()},"uniqueItems":true})
        }
        Type::Variant(t) => {
            json!({"oneOf":t.cases().map(|c|if c.ty.is_none(){json!({"type":"string","const":c.name})}else{case_schema(c.name,c.ty)}).collect::<Vec<_>>()})
        }
        Type::Result(t) => json!({"oneOf":[case_schema("ok",t.ok()),case_schema("err",t.err())]}),
        _ => unreachable!("only callable types have schemas"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn types() -> std::collections::BTreeMap<String, Type> {
        let engine = crate::runtime::engine().unwrap();
        let component = wasmtime::component::Component::from_file(
            &engine,
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/mods/echo/logic.wasm"
            ),
        )
        .unwrap();
        let ty = component.component_type();
        let wasmtime::component::types::ComponentItem::ComponentInstance(api) =
            ty.get_export(&engine, "mods:echo/api@0.1.0").unwrap().ty
        else {
            panic!("api");
        };
        api.exports(&engine)
            .filter_map(|(name, item)| {
                if !name.starts_with("id-") {
                    return None;
                }
                let wasmtime::component::types::ComponentItem::ComponentFunc(func) = item.ty else {
                    return None;
                };
                Some((name.into(), func.params().next().unwrap().1))
            })
            .collect()
    }
    #[test]
    fn ac8_each_json_row_has_literal_val_trees_both_ways() {
        let types = types();
        for (name, input, output, expected) in [
            ("id-bool", json!(true), json!(true), Val::Bool(true)),
            ("id-u8", json!(255), json!(255), Val::U8(255)),
            ("id-u16", json!(65535), json!(65535), Val::U16(65535)),
            (
                "id-u32",
                json!(4294967295u32),
                json!(4294967295u32),
                Val::U32(u32::MAX),
            ),
            (
                "id-u64",
                json!(18446744073709551615u64),
                json!(18446744073709551615u64),
                Val::U64(u64::MAX),
            ),
            ("id-s8", json!(-128), json!(-128), Val::S8(-128)),
            ("id-s16", json!(-32768), json!(-32768), Val::S16(-32768)),
            (
                "id-s32",
                json!(-2147483648i32),
                json!(-2147483648i32),
                Val::S32(i32::MIN),
            ),
            (
                "id-s64",
                json!(-9223372036854775808i64),
                json!(-9223372036854775808i64),
                Val::S64(i64::MIN),
            ),
            ("id-f32", json!(1.5), json!(1.5), Val::Float32(1.5)),
            ("id-f64", json!(-2.25), json!(-2.25), Val::Float64(-2.25)),
            ("id-char", json!("🦀"), json!("🦀"), Val::Char('🦀')),
            (
                "id-string",
                json!("hello"),
                json!("hello"),
                Val::String("hello".into()),
            ),
            (
                "id-list",
                json!([0, 255]),
                json!([0, 255]),
                Val::List(vec![Val::U8(0), Val::U8(255)]),
            ),
            (
                "id-tuple",
                json!([7, "seven"]),
                json!([7, "seven"]),
                Val::Tuple(vec![Val::U8(7), Val::String("seven".into())]),
            ),
            (
                "id-record",
                json!({"n":2}),
                json!({"n":2,"note":null}),
                Val::Record(vec![
                    ("n".into(), Val::U32(2)),
                    ("note".into(), Val::Option(None)),
                ]),
            ),
            (
                "id-variant",
                json!("idle"),
                json!("idle"),
                Val::Variant("idle".into(), None),
            ),
            (
                "id-variant",
                json!({"number":7}),
                json!({"number":7}),
                Val::Variant("number".into(), Some(Box::new(Val::U32(7)))),
            ),
            (
                "id-enum",
                json!("done"),
                json!("done"),
                Val::Enum("done".into()),
            ),
            (
                "id-flags",
                json!(["execute", "read"]),
                json!(["read", "execute"]),
                Val::Flags(vec!["read".into(), "execute".into()]),
            ),
            ("id-option", json!(null), json!(null), Val::Option(None)),
            (
                "id-option",
                json!(3),
                json!(3),
                Val::Option(Some(Box::new(Val::U32(3)))),
            ),
            (
                "id-result",
                json!({"ok":42}),
                json!({"ok":42}),
                Val::Result(Ok(Some(Box::new(Val::U32(42))))),
            ),
            (
                "id-result",
                json!({"err":"bad"}),
                json!({"err":"bad"}),
                Val::Result(Err(Some(Box::new(Val::String("bad".into()))))),
            ),
            (
                "id-unit-result",
                json!({"ok":null}),
                json!({"ok":null}),
                Val::Result(Ok(None)),
            ),
            (
                "id-unit-result",
                json!({"err":null}),
                json!({"err":null}),
                Val::Result(Err(None)),
            ),
        ] {
            assert_eq!(
                format!("{:?}", decode(&types[name], &input, "$.x").unwrap()),
                format!("{expected:?}"),
                "{name}"
            );
            assert_eq!(encode(&expected).unwrap(), output, "{name}");
        }
        for value in [
            Val::Float32(f32::NAN),
            Val::Float64(f64::INFINITY),
            Val::Float64(f64::NEG_INFINITY),
        ] {
            assert_eq!(encode(&value).unwrap(), json!(null));
        }
    }
    #[test]
    fn ac8_bad_values_name_the_exact_path() {
        let types = types();
        for (name, input, path) in [
            ("id-u8", json!(1.0), "$.x:"),
            ("id-u8", json!(256), "$.x:"),
            ("id-record", json!({"n":1,"extra":2}), "$.x.extra:"),
            ("id-tuple", json!([1]), "$.x:"),
            ("id-list", json!([1, 256]), "$.x[1]:"),
            ("id-char", json!("ab"), "$.x:"),
            ("id-option", json!(1.0), "$.x:"),
            ("id-result", json!({"ok":1,"err":"x"}), "$.x:"),
        ] {
            let error = decode(&types[name], &input, "$.x").unwrap_err();
            assert!(error.message.starts_with(path), "{error}");
        }
    }
    #[test]
    fn ac8_resource_and_async_values_are_not_callable() {
        use wasmtime::component::{Component, ResourceType, types::ComponentItem};
        for ty in [
            Type::Own(ResourceType::host::<u32>()),
            Type::Borrow(ResourceType::host::<u32>()),
            Type::ErrorContext,
        ] {
            assert!(!callable(&ty));
            assert_eq!(
                decode(&ty, &json!(null), "$.x").unwrap_err().code,
                CallCode::NoFunction
            );
        }
        let engine = crate::runtime::engine().unwrap();
        let component = Component::new(
            &engine,
            r#"(component
          (type $api (instance
            (type $future (future u32))
            (type $stream (stream u32))
            (export "future" (type (eq $future)))
            (export "stream" (type (eq $stream)))
            (export "use-future" (func (param "x" $future) (result $future)))
            (export "use-stream" (func (param "x" $stream) (result $stream)))
          ))
          (import "api" (instance $api (type $api)))
        )"#,
        )
        .unwrap();
        let ty = component.component_type();
        let ComponentItem::ComponentInstance(api) = ty.get_import(&engine, "api").unwrap().ty
        else {
            panic!("api");
        };
        for (_, item) in api.exports(&engine) {
            if let ComponentItem::ComponentFunc(func) = item.ty {
                for ty in func.params().map(|(_, ty)| ty).chain(func.results()) {
                    assert!(!callable(&ty));
                    assert_eq!(
                        decode(&ty, &json!(null), "$.x").unwrap_err().code,
                        CallCode::NoFunction
                    );
                }
            }
        }
    }
}
