//! HyperTalk runtime values. HyperTalk is famously string-centric: every value can be
//! read as text, and numbers/booleans are coercions of that text. We keep a small enum
//! and coerce on demand, matching that "everything is a string" feel.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    Text(String),
    Number(f64),
    Bool(bool),
    #[default]
    Empty,
}

impl Value {
    pub fn from_text(s: impl Into<String>) -> Value {
        Value::Text(s.into())
    }

    /// Render as the text a HyperTalk script would see.
    pub fn as_text(&self) -> String {
        match self {
            Value::Text(s) => s.clone(),
            Value::Number(n) => fmt_number(*n),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Empty => String::new(),
        }
    }

    /// Coerce to a number, HyperTalk-style: empty is 0, text is parsed (trimmed).
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(_) => None,
            Value::Empty => Some(0.0),
            Value::Text(s) => {
                let t = s.trim();
                if t.is_empty() {
                    Some(0.0)
                } else {
                    t.parse::<f64>().ok()
                }
            }
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Empty => false,
            Value::Number(n) => *n != 0.0,
            Value::Text(s) => s.eq_ignore_ascii_case("true"),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty) || matches!(self, Value::Text(s) if s.is_empty())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_text())
    }
}

/// Format a number without a trailing ".0" for integral values, like HyperTalk.
pub fn fmt_number(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{}", n);
        s
    }
}
