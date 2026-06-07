//! Maybe monad — optional values with short-circuiting bind.
//!
//! `Some(x)` carries a value; `None` represents absence.
//! `bind` short-circuits on `None`, propagating it forward.

use serde::{Deserialize, Serialize};

use crate::AgentResult;

/// Maybe monad representing an optional value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maybe<T> {
    Some(T),
    None,
}

impl<T> Maybe<T> {
    /// Wrap a value in `Some`.
    pub fn pure(value: T) -> Self {
        Maybe::Some(value)
    }

    /// Create a `None`.
    pub fn none() -> Self {
        Maybe::None
    }

    /// Bind (flatMap): apply `f` to the value if `Some`, otherwise propagate `None`.
    pub fn bind<U, F>(self, f: F) -> Maybe<U>
    where
        F: FnOnce(T) -> Maybe<U>,
    {
        match self {
            Maybe::Some(v) => f(v),
            Maybe::None => Maybe::None,
        }
    }

    /// Map a function over the value if `Some`.
    pub fn map<U, F>(self, f: F) -> Maybe<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Maybe::Some(v) => Maybe::Some(f(v)),
            Maybe::None => Maybe::None,
        }
    }

    /// Extract the value or a default.
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Maybe::Some(v) => v,
            Maybe::None => default,
        }
    }

    /// Check if this is `Some`.
    pub fn is_some(&self) -> bool {
        matches!(self, Maybe::Some(_))
    }

    /// Check if this is `None`.
    pub fn is_none(&self) -> bool {
        matches!(self, Maybe::None)
    }
}

// --- AgentMaybe ---

/// An agent processing step that may produce no result.
///
/// Each step takes a string and optionally produces a transformed string.
/// If any step returns `None`, the pipeline short-circuits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMaybe {
    pub name: String,
    #[serde(skip)]
    pub process: Option<fn(&str) -> Maybe<String>>,
}

impl AgentMaybe {
    /// Create a new agent step.
    pub fn new(name: &str, process: fn(&str) -> Maybe<String>) -> Self {
        AgentMaybe {
            name: name.to_string(),
            process: Some(process),
        }
    }

    /// A step that always passes the value through.
    pub fn passthrough(name: &str) -> Self {
        AgentMaybe {
            name: name.to_string(),
            process: Some(|s| Maybe::Some(s.to_string())),
        }
    }

    /// A step that always returns None.
    pub fn blocker(name: &str) -> Self {
        AgentMaybe {
            name: name.to_string(),
            process: Some(|_| Maybe::None),
        }
    }
}

/// Run a sequence of `AgentMaybe` steps over an input string.
///
/// Short-circuits on the first `None`.
pub fn run_maybe_pipeline(steps: &[AgentMaybe], input: &str) -> Maybe<String> {
    let mut current = Maybe::Some(input.to_string());
    for step in steps {
        if let Some(f) = step.process {
            current = current.bind(|s| f(&s));
        }
    }
    current
}

/// Run a maybe pipeline and collect a result, short-circuiting on None.
pub fn run_maybe_to_result(steps: &[AgentMaybe], input: &str) -> AgentResult<String> {
    match run_maybe_pipeline(steps, input) {
        Maybe::Some(v) => Ok(v),
        Maybe::None => Err(format!(
            "Maybe pipeline short-circuited at step '{}'",
            steps
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(" -> ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_and_unwrap() {
        let m = Maybe::pure(42);
        assert_eq!(m.unwrap_or(0), 42);
    }

    #[test]
    fn test_none_unwrap() {
        let m: Maybe<i32> = Maybe::none();
        assert_eq!(m.unwrap_or(99), 99);
    }

    #[test]
    fn test_bind_some() {
        let m = Maybe::pure(10);
        let result = m.bind(|x| Maybe::pure(x + 5));
        assert_eq!(result, Maybe::Some(15));
    }

    #[test]
    fn test_bind_none() {
        let m: Maybe<i32> = Maybe::None;
        let result = m.bind(|x| Maybe::pure(x + 5));
        assert!(result.is_none());
    }

    #[test]
    fn test_bind_short_circuit() {
        let result = Maybe::pure(1)
            .bind(|x| if x > 0 { Maybe::pure(x) } else { Maybe::None })
            .bind(|x| Maybe::pure(x * 100))
            .bind(|_| Maybe::<i32>::None);
        assert!(result.is_none());
    }

    #[test]
    fn test_map_some() {
        let result = Maybe::pure("hello").map(|s| s.len());
        assert_eq!(result, Maybe::Some(5));
    }

    #[test]
    fn test_map_none() {
        let result: Maybe<usize> = Maybe::None.map(|s: &str| s.len());
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_pipeline_all_some() {
        let steps = vec![
            AgentMaybe::new("uppercase", |s| Maybe::Some(s.to_uppercase())),
            AgentMaybe::new("add_suffix", |s| Maybe::Some(format!("{}_processed", s))),
        ];
        let result = run_maybe_pipeline(&steps, "data");
        assert_eq!(result, Maybe::Some("DATA_processed".to_string()));
    }

    #[test]
    fn test_maybe_pipeline_short_circuits() {
        let steps = vec![
            AgentMaybe::new("ok_step", |s| Maybe::Some(s.to_uppercase())),
            AgentMaybe::new("blocker", |_s| Maybe::None),
            AgentMaybe::new("never_reached", |s| Maybe::Some(s.to_string())),
        ];
        let result = run_maybe_pipeline(&steps, "data");
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_to_result_ok() {
        let steps = vec![AgentMaybe::new("double", |s| {
            Maybe::Some(format!("{}{}", s, s))
        })];
        let result = run_maybe_to_result(&steps, "ha");
        assert_eq!(result.unwrap(), "haha");
    }

    #[test]
    fn test_maybe_to_result_err() {
        let steps = vec![AgentMaybe::blocker("gate")];
        let result = run_maybe_to_result(&steps, "input");
        assert!(result.is_err());
    }
}
