use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value, json};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[test]
fn wide_integers_retain_their_type_and_digits() {
    for text in [
        "18446744073709551616".to_owned(),
        "-9223372036854775809".to_owned(),
        i128::MIN.to_string(),
        u128::MAX.to_string(),
        "9".repeat(1000),
    ] {
        let value: Value = serde_json::from_str(&text).unwrap();
        assert!(!value.as_number().unwrap().is_f64());
        assert_eq!(serde_json::to_string(&value).unwrap(), text);
        assert_eq!(serde_json::to_value(&value).unwrap(), value);
        assert_eq!(
            serde_json::from_value::<Value>(value.clone()).unwrap(),
            value
        );
        assert_eq!(
            serde_json::to_string(&json!([value])).unwrap(),
            format!("[{text}]")
        );
    }
    assert_eq!(Number::from(i128::MIN).as_i128(), Some(i128::MIN));
    assert_eq!(Number::from(u128::MAX).as_u128(), Some(u128::MAX));
    assert_eq!(
        serde_json::from_value::<i128>(json!(i128::MIN)).unwrap(),
        i128::MIN
    );
    assert_eq!(
        serde_json::from_value::<u128>(json!(u128::MAX)).unwrap(),
        u128::MAX
    );
    assert_eq!(
        serde_json::from_value::<f64>(json!(u128::MAX)).unwrap(),
        u128::MAX as f64
    );
}

#[test]
fn floats_use_numeric_equality_and_canonical_spelling() {
    for text in ["1e2", "1.00e+2", "100.0"] {
        let value: Value = serde_json::from_str(text).unwrap();
        assert_eq!(value, json!(100.0));
        assert_eq!(serde_json::to_string(&value).unwrap(), "100.0");
    }
    assert_eq!(serde_json::from_str::<Value>("-0").unwrap(), json!(0));
    assert_eq!("-0".parse::<Number>().unwrap(), Number::from(0));
    let positive = json!(0.0);
    let negative = json!(-0.0);
    let hash = |value: &Value| {
        let mut state = DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    };
    assert_eq!(positive, negative);
    assert_eq!(hash(&positive), hash(&negative));
    assert_eq!(serde_json::to_string(&negative).unwrap(), "-0.0");
}

#[test]
fn nonfinite_numbers_round_trip_through_values_and_typed_floats() {
    for (text, expected) in [
        ("NaN", f64::NAN),
        ("Infinity", f64::INFINITY),
        ("-Infinity", f64::NEG_INFINITY),
        ("1e309", f64::INFINITY),
        ("-1e9999", f64::NEG_INFINITY),
    ] {
        let value: Value = serde_json::from_str(text).unwrap();
        let number = value.as_number().unwrap();
        assert!(number.is_f64());
        for actual in [
            number.as_f64().unwrap(),
            serde_json::from_str::<f64>(text).unwrap(),
            serde_json::from_value::<f64>(value.clone()).unwrap(),
        ] {
            assert!(actual == expected || actual.is_nan() && expected.is_nan());
        }
        let canonical = if expected.is_nan() {
            "NaN"
        } else if expected.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        };
        assert_eq!(number.to_string(), canonical);
        assert_eq!(serde_json::to_string(&expected).unwrap(), canonical);
        assert_eq!(
            serde_json::to_string(&(expected as f32)).unwrap(),
            canonical
        );
        assert_eq!(serde_json::to_string(&value).unwrap(), canonical);
        assert_eq!(serde_json::to_value(expected).unwrap(), value);
        assert!(serde_json::from_str::<i64>(text).is_err());
    }
}

#[test]
fn finite_single_precision_values_round_trip() {
    for value in [
        0.0_f32,
        -0.0,
        1.0,
        f32::EPSILON,
        f32::MIN_POSITIVE,
        f32::MIN,
        f32::MAX,
        f32::from_bits(1),
        -f32::from_bits(1),
    ] {
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<f32>(&encoded).unwrap().to_bits(),
            value.to_bits()
        );
    }
}

#[test]
fn raw_and_ignored_values_accept_nonfinite_numbers() {
    #[derive(Deserialize)]
    struct Known {
        keep: i32,
    }
    let text = r#"{"skip":[NaN,Infinity,-Infinity,{"large":18446744073709551616}],"keep":7}"#;
    assert_eq!(serde_json::from_str::<Known>(text).unwrap().keep, 7);
    let raw: Box<serde_json::value::RawValue> = serde_json::from_str(text).unwrap();
    assert_eq!(raw.get(), text);
    let value: Value = serde_json::from_str(text).unwrap();
    assert_eq!(serde_json::to_value(&raw).unwrap(), value);
    assert_eq!(
        serde_json::from_value::<Value>(value.clone()).unwrap(),
        value
    );
}

#[test]
fn reserved_marker_names_remain_literal_object_keys() {
    #[derive(Deserialize, Serialize)]
    struct Flattened {
        keep: i32,
        #[serde(flatten)]
        fields: Map<String, Value>,
    }
    for key in [
        "$serde_json::private::Number",
        "$serde_json::private::RawValue",
    ] {
        let text = format!(r#"{{"{key}":"1e2","keep":7}}"#);
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value[key], "1e2");
        assert_eq!(
            serde_json::from_value::<Value>(value.clone()).unwrap(),
            value
        );
        assert_eq!(serde_json::to_value(&value).unwrap(), value);
        let flattened: Flattened = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_value(flattened).unwrap(), value);
    }
}

#[test]
fn malformed_number_tokens_remain_invalid() {
    for text in [
        "+Infinity",
        "-NaN",
        "nan",
        "inf",
        "NaNx",
        "Infinity0",
        "01",
        "-01",
        "1.",
        "1e",
        "1e+",
        "--1",
    ] {
        assert!(serde_json::from_str::<Value>(text).is_err(), "{text}");
        assert!(
            serde_json::from_str::<Box<serde_json::value::RawValue>>(text).is_err(),
            "{text}"
        );
    }
}
