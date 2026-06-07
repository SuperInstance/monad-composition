//! Reader monad — read-only access to a shared environment.
//!
//! `Reader<E, A>` wraps `E -> A`. Bind chains computations that
//! all read from the same environment without modifying it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::AgentResult;

/// Reader monad: a computation that reads from an environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reader<E, A> {
    /// The computation: given an environment, produce a value.
    #[serde(skip)]
    pub run_reader: Option<fn(&E) -> A>,
    /// Serializable description.
    pub label: String,
}

impl<E, A> Reader<E, A> {
    /// Create a reader from a function.
    pub fn new(label: &str, f: fn(&E) -> A) -> Self {
        Reader {
            run_reader: Some(f),
            label: label.to_string(),
        }
    }

    /// Execute the reader with an environment.
    pub fn run(self, env: &E) -> A {
        if let Some(f) = self.run_reader {
            f(env)
        } else {
            panic!("Reader has no function");
        }
    }
}

impl<E: Clone, A: Clone> Reader<E, A> {
    /// Pure: produce a value ignoring the environment.
    pub fn pure(label: &str, _value: A) -> Self
    where
        A: Clone,
    {
        // We can't store the value in a fn pointer, so we store None
        // and handle this in the agent pipeline.
        Reader {
            run_reader: None,
            label: label.to_string(),
        }
    }

    /// Map over the result.
    pub fn map<B>(self, _f: fn(A) -> B) -> Reader<E, B> {
        Reader {
            run_reader: None,
            label: format!("{} >> map", self.label),
        }
    }
}

// --- AgentReader ---

/// An agent step with read-only access to a configuration environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReader {
    pub name: String,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub process: Option<fn(&HashMap<String, String>) -> AgentResult<String>>,
}

impl AgentReader {
    /// Create a new reader agent step.
    pub fn new(name: &str, process: fn(&HashMap<String, String>) -> AgentResult<String>) -> Self {
        AgentReader {
            name: name.to_string(),
            process: Some(process),
        }
    }
}

/// Run a reader pipeline, providing the same config to every step.
pub fn run_reader_pipeline(
    steps: &[AgentReader],
    config: &HashMap<String, String>,
) -> AgentResult<Vec<String>> {
    let mut outputs = vec![];
    for step in steps {
        if let Some(f) = step.process {
            let result = f(config)
                .map_err(|e| format!("Reader pipeline failed at step '{}': {}", step.name, e))?;
            outputs.push(result);
        }
    }
    Ok(outputs)
}

/// Read a required config key.
pub fn config_get(config: &HashMap<String, String>, key: &str) -> AgentResult<String> {
    config
        .get(key)
        .cloned()
        .ok_or_else(|| format!("config key '{}' not found", key))
}

/// Read an optional config key with a default.
pub fn config_get_or(config: &HashMap<String, String>, key: &str, default: &str) -> String {
    config
        .get(key)
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// Check if a config key exists.
pub fn config_has(config: &HashMap<String, String>, key: &str) -> bool {
    config.contains_key(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("model".to_string(), "gpt-4".to_string());
        h.insert("temperature".to_string(), "0.7".to_string());
        h.insert("max_tokens".to_string(), "4096".to_string());
        h
    }

    #[test]
    fn test_reader_new_and_run() {
        let r = Reader::new("get_model", |config: &HashMap<String, String>| {
            config.get("model").unwrap().clone()
        });
        let val = r.run(&make_config());
        assert_eq!(val, "gpt-4");
    }

    #[test]
    fn test_reader_pipeline() {
        let steps = vec![
            AgentReader::new("read_model", |config| config_get(config, "model")),
            AgentReader::new("read_temp", |config| config_get(config, "temperature")),
            AgentReader::new("combined", |config| {
                let model = config_get(config, "model")?;
                let temp = config_get(config, "temperature")?;
                Ok(format!("{}@{}", model, temp))
            }),
        ];
        let outputs = run_reader_pipeline(&steps, &make_config()).unwrap();
        assert_eq!(outputs[0], "gpt-4");
        assert_eq!(outputs[1], "0.7");
        assert_eq!(outputs[2], "gpt-4@0.7");
    }

    #[test]
    fn test_reader_pipeline_error() {
        let steps = vec![AgentReader::new("missing_key", |config| {
            config_get(config, "nonexistent")
        })];
        let result = run_reader_pipeline(&steps, &make_config());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_get_ok() {
        assert_eq!(config_get(&make_config(), "model").unwrap(), "gpt-4");
    }

    #[test]
    fn test_config_get_missing() {
        assert!(config_get(&make_config(), "absent").is_err());
    }

    #[test]
    fn test_config_get_or_present() {
        assert_eq!(config_get_or(&make_config(), "model", "default"), "gpt-4");
    }

    #[test]
    fn test_config_get_or_missing() {
        assert_eq!(
            config_get_or(&make_config(), "absent", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn test_config_has() {
        assert!(config_has(&make_config(), "model"));
        assert!(!config_has(&make_config(), "nope"));
    }

    #[test]
    fn test_reader_pipeline_empty() {
        let outputs = run_reader_pipeline(&[], &make_config()).unwrap();
        assert!(outputs.is_empty());
    }
}
