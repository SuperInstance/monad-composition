//! Writer monad — computations that produce a value and accumulate a log.
//!
//! `pure(x) = (x, [])`, `bind((v, log), f)` runs `f(v)` and concatenates logs.
//! Useful for tracking decisions and audit trails in agent pipelines.

use serde::{Deserialize, Serialize};

use crate::AgentResult;

/// Writer monad: a value paired with an accumulated log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Writer<T> {
    pub value: T,
    pub log: Vec<String>,
}

impl<T> Writer<T> {
    /// Create a writer with a value and empty log.
    pub fn pure(value: T) -> Self {
        Writer { value, log: vec![] }
    }

    /// Create a writer with a value and a single log entry.
    pub fn with_log(value: T, msg: &str) -> Self {
        Writer {
            value,
            log: vec![msg.to_string()],
        }
    }

    /// Bind: run a computation, concatenating logs.
    pub fn bind<U, F>(self, f: F) -> Writer<U>
    where
        F: FnOnce(T) -> Writer<U>,
    {
        let Writer { value, log } = self;
        let next = f(value);
        let mut combined = log;
        combined.extend(next.log);
        Writer {
            value: next.value,
            log: combined,
        }
    }

    /// Map over the value, leaving the log unchanged.
    pub fn map<U, F>(self, f: F) -> Writer<U>
    where
        F: FnOnce(T) -> U,
    {
        Writer {
            value: f(self.value),
            log: self.log,
        }
    }

    /// Extract the value and log.
    pub fn run(self) -> (T, Vec<String>) {
        (self.value, self.log)
    }

    /// Get a reference to the log.
    pub fn read_log(&self) -> &[String] {
        &self.log
    }
}

/// Combine two writers, running them sequentially and merging logs.
pub fn sequence<T>(writers: Vec<Writer<T>>) -> Writer<Vec<T>> {
    let mut values = vec![];
    let mut all_logs: Vec<String> = vec![];
    for w in writers {
        values.push(w.value);
        all_logs.extend(w.log);
    }
    Writer {
        value: values,
        log: all_logs,
    }
}

// --- AgentWriter ---

/// An agent processing step that logs its decisions.
///
/// Takes a string input, produces a transformed string, and logs what it did.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWriter {
    pub name: String,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub process: Option<fn(&str) -> Writer<String>>,
}

impl AgentWriter {
    /// Create a new agent writer step.
    pub fn new(name: &str, process: fn(&str) -> Writer<String>) -> Self {
        AgentWriter {
            name: name.to_string(),
            process: Some(process),
        }
    }

    /// A step that passes through input and logs it.
    pub fn passthrough(name: &str) -> Self {
        AgentWriter {
            name: name.to_string(),
            process: Some(|s: &str| Writer::with_log(s.to_string(), "passed through")),
        }
    }
}

/// Run a writer pipeline, accumulating logs from all steps.
pub fn run_writer_pipeline(steps: &[AgentWriter], input: &str) -> AgentResult<Writer<String>> {
    let mut current = Writer::pure(input.to_string());
    for step in steps {
        if let Some(f) = step.process {
            current = current.bind(|s| f(&s));
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure() {
        let w = Writer::pure(42);
        assert_eq!(w.value, 42);
        assert!(w.log.is_empty());
    }

    #[test]
    fn test_writer_with_log() {
        let w = Writer::with_log(10, "created");
        assert_eq!(w.value, 10);
        assert_eq!(w.log, vec!["created"]);
    }

    #[test]
    fn test_bind_accumulates_logs() {
        let w = Writer::with_log(1, "step1")
            .bind(|x| Writer::with_log(x + 1, "step2"))
            .bind(|x| Writer::with_log(x * 10, "step3"));
        assert_eq!(w.value, 20);
        assert_eq!(w.log, vec!["step1", "step2", "step3"]);
    }

    #[test]
    fn test_map_preserves_log() {
        let w = Writer::with_log(5, "logged");
        let mapped = w.map(|x| x * 2);
        assert_eq!(mapped.value, 10);
        assert_eq!(mapped.log, vec!["logged"]);
    }

    #[test]
    fn test_run_extracts() {
        let w = Writer::with_log("result", "entry");
        let (val, log) = w.run();
        assert_eq!(val, "result");
        assert_eq!(log, vec!["entry"]);
    }

    #[test]
    fn test_sequence() {
        let ws = vec![
            Writer::with_log(1, "a"),
            Writer::with_log(2, "b"),
            Writer::with_log(3, "c"),
        ];
        let combined = sequence(ws);
        assert_eq!(combined.value, vec![1, 2, 3]);
        assert_eq!(combined.log, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_writer_pipeline() {
        let steps = vec![
            AgentWriter::new("uppercase", |s| {
                Writer::with_log(s.to_uppercase(), "converted to uppercase")
            }),
            AgentWriter::new("reverse", |s| {
                let reversed: String = s.chars().rev().collect();
                Writer::with_log(reversed, "reversed the string")
            }),
        ];
        let result = run_writer_pipeline(&steps, "hello").unwrap();
        assert_eq!(result.value, "OLLEH");
        assert_eq!(
            result.log,
            vec!["converted to uppercase", "reversed the string"]
        );
    }

    #[test]
    fn test_writer_pipeline_empty() {
        let result = run_writer_pipeline(&[], "data").unwrap();
        assert_eq!(result.value, "data");
        assert!(result.log.is_empty());
    }

    #[test]
    fn test_writer_chain_order() {
        let w = Writer::pure(0)
            .bind(|_| Writer::with_log(1, "first"))
            .bind(|_| Writer::with_log(2, "second"))
            .bind(|_| Writer::with_log(3, "third"));
        assert_eq!(w.log, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_read_log() {
        let w = Writer::with_log(42, "logged");
        assert_eq!(w.read_log(), &["logged".to_string()]);
    }
}
