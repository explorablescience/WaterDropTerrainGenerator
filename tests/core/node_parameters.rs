use waterdrop_terrain_generator::core::node_parameters::{NParamConstraints, NParamValue};

#[test]
fn int_range_accepts_values_inside_the_bounds() {
    let constraint = NParamConstraints::IntRange { min: 0, max: 10 };
    assert!(constraint.validate(&NParamValue::Int(0)).is_ok());
    assert!(constraint.validate(&NParamValue::Int(10)).is_ok());
    assert!(constraint.validate(&NParamValue::Int(5)).is_ok());
}

#[test]
fn int_range_rejects_values_outside_the_bounds() {
    let constraint = NParamConstraints::IntRange { min: 0, max: 10 };
    assert!(constraint.validate(&NParamValue::Int(-1)).is_err());
    assert!(constraint.validate(&NParamValue::Int(11)).is_err());
}

#[test]
fn float_range_accepts_values_inside_the_bounds() {
    let constraint = NParamConstraints::FloatRange { min: 0.0, max: 1.0 };
    assert!(constraint.validate(&NParamValue::Float(0.0)).is_ok());
    assert!(constraint.validate(&NParamValue::Float(1.0)).is_ok());
    assert!(constraint.validate(&NParamValue::Float(0.5)).is_ok());
}

#[test]
fn float_range_rejects_values_outside_the_bounds() {
    let constraint = NParamConstraints::FloatRange { min: 0.0, max: 1.0 };
    assert!(constraint.validate(&NParamValue::Float(-0.01)).is_err());
    assert!(constraint.validate(&NParamValue::Float(1.01)).is_err());
}

#[test]
fn vector2_range_accepts_values_inside_the_bounds() {
    let constraint = NParamConstraints::Vector2Range {
        min: (0.0, 0.0),
        max: (1.0, 1.0)
    };
    assert!(constraint.validate(&NParamValue::Vector2(0.0, 0.0)).is_ok());
    assert!(constraint.validate(&NParamValue::Vector2(1.0, 1.0)).is_ok());
    assert!(
        constraint
            .validate(&NParamValue::Vector2(0.5, 0.25))
            .is_ok()
    );
}

#[test]
fn vector2_range_rejects_values_outside_the_bounds_on_either_axis() {
    let constraint = NParamConstraints::Vector2Range {
        min: (0.0, 0.0),
        max: (1.0, 1.0)
    };
    assert!(
        constraint
            .validate(&NParamValue::Vector2(-0.01, 0.5))
            .is_err()
    );
    assert!(
        constraint
            .validate(&NParamValue::Vector2(0.5, 1.01))
            .is_err()
    );
}

#[test]
fn vector2_int_range_accepts_values_inside_the_bounds() {
    let constraint = NParamConstraints::Vector2IntRange {
        min: (0, 0),
        max: (10, 10)
    };
    assert!(constraint.validate(&NParamValue::Vector2Int(0, 0)).is_ok());
    assert!(
        constraint
            .validate(&NParamValue::Vector2Int(10, 10))
            .is_ok()
    );
    assert!(constraint.validate(&NParamValue::Vector2Int(5, 3)).is_ok());
}

#[test]
fn vector2_int_range_rejects_values_outside_the_bounds_on_either_axis() {
    let constraint = NParamConstraints::Vector2IntRange {
        min: (0, 0),
        max: (10, 10)
    };
    assert!(
        constraint
            .validate(&NParamValue::Vector2Int(-1, 5))
            .is_err()
    );
    assert!(
        constraint
            .validate(&NParamValue::Vector2Int(5, 11))
            .is_err()
    );
}

#[test]
fn string_max_length_accepts_short_strings() {
    let constraint = NParamConstraints::StringMaxLength { max_length: 5 };
    assert!(
        constraint
            .validate(&NParamValue::String("abc".to_string()))
            .is_ok()
    );
    assert!(
        constraint
            .validate(&NParamValue::String("abcde".to_string()))
            .is_ok()
    );
}

#[test]
fn string_max_length_rejects_long_strings() {
    let constraint = NParamConstraints::StringMaxLength { max_length: 5 };
    assert!(
        constraint
            .validate(&NParamValue::String("abcdef".to_string()))
            .is_err()
    );
}

#[test]
fn enum_one_of_accepts_listed_options() {
    let constraint = NParamConstraints::EnumOneOf {
        options: vec!["A", "B", "C"]
    };
    assert!(
        constraint
            .validate(&NParamValue::Enum("B".to_string()))
            .is_ok()
    );
}

#[test]
fn enum_one_of_rejects_unlisted_options() {
    let constraint = NParamConstraints::EnumOneOf {
        options: vec!["A", "B", "C"]
    };
    assert!(
        constraint
            .validate(&NParamValue::Enum("D".to_string()))
            .is_err()
    );
}

#[test]
fn custom_constraint_delegates_to_the_provided_function() {
    let constraint = NParamConstraints::Custom(Box::new(|v| match v {
        NParamValue::Int(v) if *v % 2 == 0 => Ok(()),
        NParamValue::Int(v) => Err(format!("{} is not even", v)),
        _ => Err("expected an int".to_string())
    }));
    assert!(constraint.validate(&NParamValue::Int(4)).is_ok());
    assert!(constraint.validate(&NParamValue::Int(3)).is_err());
}

#[test]
fn mismatched_constraint_and_value_types_fail_validation() {
    let constraint = NParamConstraints::IntRange { min: 0, max: 10 };
    assert!(constraint.validate(&NParamValue::Float(5.0)).is_err());
    assert!(
        constraint
            .validate(&NParamValue::String("5".to_string()))
            .is_err()
    );
}

#[test]
fn param_value_equality_ignores_the_action_success_flag() {
    // `Action` only carries UI-facing metadata; two actions compare equal regardless of it
    // because that's what `PartialEq` derives to, and callers rely on that when diffing values.
    assert_eq!(
        NParamValue::Action {
            show_success_message: true
        },
        NParamValue::Action {
            show_success_message: true
        }
    );
    assert_ne!(
        NParamValue::Action {
            show_success_message: true
        },
        NParamValue::Action {
            show_success_message: false
        }
    );
}
