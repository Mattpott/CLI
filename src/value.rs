use std::error::Error;

use ratatui::widgets::Cell;
use rusqlite::types::Value as RsqValue;

/// Mirror of Rusqlite's value type, but is, importantly, owned by this
/// crate allowing for implementations of traits, functions, etc.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// Fieldless version of [`Value`] for the sake of signaling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Null,
    Integer,
    Real,
    Text,
    Blob,
}

/// Error for unhandled actions
#[derive(Debug, Clone)]
pub struct InvalidValueTypeError {
    origin: String,
}

impl InvalidValueTypeError {
    pub fn new(origin: String) -> Self {
        InvalidValueTypeError { origin }
    }
}

impl Error for InvalidValueTypeError {}

impl std::fmt::Display for InvalidValueTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Trying to get ValueType from invalid string: {}",
            self.origin
        )
    }
}

impl TryFrom<String> for ValueType {
    type Error = InvalidValueTypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ValueType::try_from(value.as_str())
    }
}

impl TryFrom<&str> for ValueType {
    type Error = InvalidValueTypeError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "NULL" => Ok(ValueType::Null),
            "INTEGER" => Ok(ValueType::Integer),
            "REAL" => Ok(ValueType::Real),
            "TEXT" => Ok(ValueType::Text),
            "BLOB" => Ok(ValueType::Blob),
            unknown => Err(InvalidValueTypeError::new(unknown.to_string())),
        }
    }
}

impl Value {
    pub fn parse_column(data_type: &ValueType, text: &str) -> Result<Value, Box<dyn Error>> {
        match data_type {
            ValueType::Null => Ok(Value::Null),
            ValueType::Integer => Ok(Value::Integer(text.parse()?)),
            ValueType::Real => Ok(Value::Real(text.parse()?)),
            ValueType::Text => Ok(Value::Text(text.to_string())),
            ValueType::Blob => Ok(Value::Blob(text.bytes().collect())),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            // The value is a `NULL` value.
            Self::Null => "NULL".to_string(),
            // The value is a signed integer.
            Self::Integer(int) => int.to_string(),
            // The value is a floating point number.
            Self::Real(real) => real.to_string(),
            // The value is a text string.
            Self::Text(text) => text.clone(),
            // The value is a blob of data
            Self::Blob(blob) => {
                if blob.is_empty() {
                    "Empty Blob".to_string()
                } else {
                    // map blob to a single string of bytes
                    blob.iter().fold("Blob data:\t".to_string(), |cur, item| {
                        cur + item.to_string().as_str()
                    })
                }
            }
        };
        write!(f, "{}", data)
    }
}

// Used for taking implementation ownership of the Rusqlite Value in
// so that code can be added as needed
impl From<RsqValue> for Value {
    fn from(value: RsqValue) -> Self {
        match value {
            RsqValue::Null => Self::Null,
            RsqValue::Integer(int) => Self::Integer(int),
            RsqValue::Real(real) => Self::Real(real),
            RsqValue::Text(text) => Self::Text(text),
            RsqValue::Blob(blob) => Self::Blob(blob),
        }
    }
}

// Converts back from our implemented Value type to Rusqlite's one
impl From<Value> for RsqValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Integer(int) => Self::Integer(int),
            Value::Real(real) => Self::Real(real),
            Value::Text(text) => Self::Text(text),
            Value::Blob(blob) => Self::Blob(blob),
        }
    }
}

// Converts back from our implemented Value type to Rusqlite's one
impl From<&Value> for RsqValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Integer(int) => Self::Integer(*int),
            Value::Real(real) => Self::Real(*real),
            Value::Text(text) => Self::Text(text.clone()),
            Value::Blob(blob) => Self::Blob(blob.clone()),
        }
    }
}

/// Consuming conversion from Value to Cell, required for simple creation of
/// Ratatui Rows from Vec<Value>
impl From<Value> for Cell<'_> {
    fn from(value: Value) -> Self {
        Self::from(&value)
    }
}

/// Consuming conversion from Value to Cell, required for simple creation of
/// Ratatui Rows from Vec<Value> without consuming within creation
impl From<&Value> for Cell<'_> {
    fn from(value: &Value) -> Self {
        Self::new(value.to_string())
    }
}
