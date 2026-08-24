/// Represents the value of a node parameter, which can be of various types.
#[derive(Debug, Clone, PartialEq)]
pub enum NParamValue {
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Enum(String)
}

/// Describes a node parameter, including its name, type, default value, and optional constraints.
pub struct NParamDesc {
    pub key: &'static str,
    pub label: &'static str,
    pub default: NParamValue,
    pub constraints: Option<NParamConstraints>
}

/// Constraints for validating node parameters.
pub enum NParamConstraints {
    /// The value must be an integer within the specified range (inclusive).
    IntRange { min: i32, max: i32 },
    /// The value must be a float within the specified range (inclusive).
    FloatRange { min: f32, max: f32 },
    /// The value must be a string with a maximum length.
    StringMaxLength { max_length: usize },
    /// The value must be one of the specified options.
    EnumOneOf { options: Vec<&'static str> },
    /// Custom validation function. The function should return `Ok(())` if the value is valid, or an `Err(String)` with an error message if invalid.
    Custom(NParamValidator)
}
/// Custom validation function for a node parameter.
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
            (NParamConstraints::Custom(f), _) => f(value),
            _ => Err("Type mismatch between constraint and value".to_string())
        }
    }
}
