use std::hash::Hash;

#[derive(Debug, Clone, PartialEq)]
pub enum NParamValue {
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Enum(String),
    /// Edited as a pair of X/Y sliders.
    Vector2(f32, f32),
    /// Edited as a pair of X/Y sliders.
    Vector2Int(i32, i32),
    /// Rendered as a button in the properties panel.
    Action {
        show_success_message: bool
    }
}
impl Hash for NParamValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            NParamValue::Int(v) => v.hash(state),
            NParamValue::Float(v) => v.to_bits().hash(state),
            NParamValue::Bool(v) => v.hash(state),
            NParamValue::String(v) => v.hash(state),
            NParamValue::Enum(v) => v.hash(state),
            NParamValue::Vector2(x, y) => {
                x.to_bits().hash(state);
                y.to_bits().hash(state);
            }
            NParamValue::Vector2Int(x, y) => {
                x.hash(state);
                y.hash(state);
            }
            NParamValue::Action {
                show_success_message
            } => show_success_message.hash(state)
        }
    }
}

pub struct NParamDesc {
    pub key: &'static str,
    pub label: &'static str,
    /// Purely presentational (e.g. `"Noise"`, `"Simulation"`); has no effect on processing.
    pub category: &'static str,
    pub default: NParamValue,
    pub constraints: Option<NParamConstraints>
}
impl Hash for NParamDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
        self.label.hash(state);
        self.default.hash(state);
    }
}

pub enum NParamConstraints {
    /// Inclusive.
    IntRange {
        min: i32,
        max: i32
    },
    /// Inclusive.
    FloatRange {
        min: f32,
        max: f32
    },
    StringMaxLength {
        max_length: usize
    },
    EnumOneOf {
        options: Vec<&'static str>
    },
    /// Inclusive, independently per axis.
    Vector2Range {
        min: (f32, f32),
        max: (f32, f32)
    },
    /// Inclusive, independently per axis.
    Vector2IntRange {
        min: (i32, i32),
        max: (i32, i32)
    },
    Custom(NParamValidator)
}
pub type NParamValidator = Box<dyn Fn(&NParamValue) -> Result<(), String> + Send + Sync>;
impl NParamConstraints {
    pub fn validate(&self, value: &NParamValue) -> Result<(), String> {
        match (self, value) {
            (NParamConstraints::IntRange { min, max }, NParamValue::Int(v)) => {
                if *v < *min || *v > *max {
                    Err(format!("Value {} is out of range [{}, {}]", v, min, max))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::FloatRange { min, max }, NParamValue::Float(v)) => {
                if *v < *min || *v > *max {
                    Err(format!("Value {} is out of range [{}, {}]", v, min, max))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::StringMaxLength { max_length }, NParamValue::String(s)) => {
                if s.len() > *max_length {
                    Err(format!(
                        "String length {} exceeds maximum length {}",
                        s.len(),
                        max_length
                    ))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::EnumOneOf { options }, NParamValue::Enum(s)) => {
                if !options.contains(&s.as_str()) {
                    Err(format!(
                        "Value '{}' is not one of the allowed options: {:?}",
                        s, options
                    ))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::Vector2Range { min, max }, NParamValue::Vector2(x, y)) => {
                if *x < min.0 || *x > max.0 || *y < min.1 || *y > max.1 {
                    Err(format!(
                        "Value ({}, {}) is out of range [{:?}, {:?}]",
                        x, y, min, max
                    ))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::Vector2IntRange { min, max }, NParamValue::Vector2Int(x, y)) => {
                if *x < min.0 || *x > max.0 || *y < min.1 || *y > max.1 {
                    Err(format!(
                        "Value ({}, {}) is out of range [{:?}, {:?}]",
                        x, y, min, max
                    ))
                } else {
                    Ok(())
                }
            }
            (NParamConstraints::Custom(f), _) => f(value),
            _ => Err("Type mismatch between constraint and value".to_string())
        }
    }
}
