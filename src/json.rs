use std::str;

#[derive(Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
}

impl Value {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        if bytes[0] == b'"' {
            Self::parse_string(&bytes[1..])
        } else if (bytes[0] >= b'0' && bytes[0] <= b'9') || bytes[0] == b'-' {
            Self::parse_number(bytes)
        } else if bytes.len() >= 4 && bytes == b"null" {
            Some(Value::Null)
        } else if bytes.len() >= 4 && bytes == b"true" {
            Some(Value::Bool(true))
        } else if bytes.len() >= 5 && bytes == b"false" {
            Some(Value::Bool(false))
        } else {
            None
        }
    }

    fn parse_number(bytes: &[u8]) -> Option<Self> {
        let delimiter = bytes.iter().position(|&ch| ch == b',' || ch == b'}')?;
        let string = str::from_utf8(&bytes[..delimiter]).ok()?;
        string.parse().map(Value::Number).ok()
    }

    fn parse_string(bytes: &[u8]) -> Option<Self> {
        let delimiter = bytes.iter().position(|&ch| ch == b'"')?;
        str::from_utf8(&bytes[..delimiter])
            .map(|s| s.to_string())
            .map(Value::String)
            .ok()
    }
}
