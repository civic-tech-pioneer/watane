//! Equality of two JSON values for tests: object members in any order, numbers
//! by value (TECHNICAL 8.1).

use serde_json::Value;

pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| json_eq(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| json_eq(a, b)))
        }
        (Value::Number(a), Value::Number(b)) => {
            if a == b {
                return true;
            }
            // Never round two integers through f64: adjacent large integers differ.
            let integer_float = |integer: &serde_json::Number, float: &serde_json::Number| {
                let Some(f) = float.as_f64().filter(|_| float.is_f64()) else {
                    return false;
                };
                if let Some(i) = integer.as_i64() {
                    (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&f)
                        && f as i64 == i
                        && i as f64 == f
                } else if let Some(i) = integer.as_u64() {
                    (0.0..18_446_744_073_709_551_616.0).contains(&f)
                        && f as u64 == i
                        && i as f64 == f
                } else {
                    false
                }
            };
            integer_float(a, b) || integer_float(b, a)
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac7_json_equality_literal_pairs() {
        for (a, b, equal) in [
            (r#"{"a":1,"b":2}"#, r#"{"b":2,"a":1}"#, true),
            ("1", "1.0", true),
            ("-1", "-1.0", true),
            ("0", "-0.0", true),
            (
                r#"[{"x":[1,null,true]}]"#,
                r#"[{"x":[1.0,null,true]}]"#,
                true,
            ),
            ("1", "true", false),
            ("1", r#""1""#, false),
            ("null", "{}", false),
            ("[]", "{}", false),
            ("[1,2]", "[2,1]", false),
            (r#"{"x":null}"#, "{}", false),
            ("1", "1.01", false),
            ("9007199254740992", "9007199254740993", false),
            ("9007199254740993", "9007199254740992.0", false),
            ("18446744073709551615", "18446744073709551616.0", false),
        ] {
            let a = serde_json::from_str(a).unwrap();
            let b = serde_json::from_str(b).unwrap();
            assert_eq!(json_eq(&a, &b), equal, "{a} vs {b}");
            assert_eq!(json_eq(&b, &a), equal, "{b} vs {a}");
        }
    }
}
