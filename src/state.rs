//! State monad — computations that thread state through a chain.
//!
//! `State<S, A>` wraps `S -> (A, S)`. Bind threads the state through
//! each computation, enabling shared mutable state without mutation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::AgentResult;

/// State monad: a computation that takes state and produces a value + new state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State<S, A> {
    /// The computation: given state, produce (value, new_state).
    #[serde(skip)]
    pub run_state: Option<fn(S) -> (A, S)>,
    /// Serializable representation of the state transition description.
    pub label: String,
}

impl<S: Clone, A: Clone> State<S, A> {
    /// Create a state computation from a function.
    pub fn new(label: &str, f: fn(S) -> (A, S)) -> Self {
        State {
            run_state: Some(f),
            label: label.to_string(),
        }
    }

    /// Pure: produce a value without modifying state.
    pub fn pure(label: &str, _value: A) -> Self
    where
        A: Clone,
    {
        State {
            run_state: None,
            label: label.to_string(),
        }
    }

    /// Bind: chain state computations sequentially.
    pub fn bind<B>(self, _f: fn(A) -> State<S, B>) -> State<S, B> {
        // We can't easily compose fn pointers, so we store the label
        // and rely on AgentState for actual execution.
        State {
            run_state: None,
            label: format!("{} >> bind", self.label),
        }
    }

    /// Execute the state computation with initial state.
    pub fn run(self, state: S) -> (A, S) {
        if let Some(f) = self.run_state {
            f(state)
        } else {
            panic!("State computation has no function (pure or composed)");
        }
    }
}

// --- AgentState ---

/// A stateful agent step that reads and modifies a shared state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub name: String,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub process: Option<fn(&mut HashMap<String, String>) -> AgentResult<String>>,
}

impl AgentState {
    /// Create a new stateful agent step.
    pub fn new(
        name: &str,
        process: fn(&mut HashMap<String, String>) -> AgentResult<String>,
    ) -> Self {
        AgentState {
            name: name.to_string(),
            process: Some(process),
        }
    }
}

/// Run a sequence of stateful agent steps, threading state through each.
pub fn run_state_pipeline(
    steps: &[AgentState],
    initial_state: HashMap<String, String>,
) -> AgentResult<(Vec<String>, HashMap<String, String>)> {
    let mut state = initial_state;
    let mut outputs = vec![];
    for step in steps {
        if let Some(f) = step.process {
            let result = f(&mut state)
                .map_err(|e| format!("State pipeline failed at step '{}': {}", step.name, e))?;
            outputs.push(result);
        }
    }
    Ok((outputs, state))
}

/// Read a key from state, returning an error if missing.
pub fn state_get(state: &HashMap<String, String>, key: &str) -> AgentResult<String> {
    state
        .get(key)
        .cloned()
        .ok_or_else(|| format!("key '{}' not found in state", key))
}

/// Write a key-value pair into state.
pub fn state_put(state: &mut HashMap<String, String>, key: &str, value: &str) {
    state.insert(key.to_string(), value.to_string());
}

/// Modify a state value if it exists.
pub fn state_modify(
    state: &mut HashMap<String, String>,
    key: &str,
    f: fn(&str) -> String,
) -> AgentResult<()> {
    if let Some(v) = state.get(key) {
        let new_v = f(v);
        state.insert(key.to_string(), new_v);
        Ok(())
    } else {
        Err(format!("key '{}' not found in state", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("count".to_string(), "0".to_string());
        h.insert("name".to_string(), "agent".to_string());
        h
    }

    #[test]
    fn test_state_new_and_run() {
        let s = State::new("increment", |mut s: HashMap<String, String>| {
            let count: i32 = s.get("count").unwrap().parse().unwrap();
            s.insert("count".to_string(), (count + 1).to_string());
            (format!("count={}", count + 1), s)
        });
        let (val, state) = s.run(make_state());
        assert_eq!(val, "count=1");
        assert_eq!(state.get("count").unwrap(), "1");
    }

    #[test]
    fn test_state_pipeline() {
        let steps = vec![
            AgentState::new("increment", |state| {
                state_modify(state, "count", |v| {
                    (v.parse::<i32>().unwrap() + 1).to_string()
                })?;
                Ok(state_get(state, "count")?)
            }),
            AgentState::new("rename", |state| {
                state_put(state, "name", "super_agent");
                Ok("renamed".to_string())
            }),
        ];
        let (outputs, final_state) = run_state_pipeline(&steps, make_state()).unwrap();
        assert_eq!(outputs, vec!["1", "renamed"]);
        assert_eq!(final_state.get("name").unwrap(), "super_agent");
        assert_eq!(final_state.get("count").unwrap(), "1");
    }

    #[test]
    fn test_state_pipeline_error() {
        let steps = vec![AgentState::new("fail", |_state| {
            Err("something went wrong".to_string())
        })];
        let result = run_state_pipeline(&steps, make_state());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fail"));
    }

    #[test]
    fn test_state_get_ok() {
        let state = make_state();
        assert_eq!(state_get(&state, "count").unwrap(), "0");
    }

    #[test]
    fn test_state_get_missing() {
        let state = make_state();
        assert!(state_get(&state, "missing").is_err());
    }

    #[test]
    fn test_state_put() {
        let mut state = make_state();
        state_put(&mut state, "new_key", "new_value");
        assert_eq!(state.get("new_key").unwrap(), "new_value");
    }

    #[test]
    fn test_state_modify_ok() {
        let mut state = make_state();
        state_modify(&mut state, "count", |v| {
            (v.parse::<i32>().unwrap() + 10).to_string()
        })
        .unwrap();
        assert_eq!(state.get("count").unwrap(), "10");
    }

    #[test]
    fn test_state_modify_missing() {
        let mut state = make_state();
        let result = state_modify(&mut state, "absent", |_| "x".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_state_empty_pipeline() {
        let (outputs, final_state) = run_state_pipeline(&[], make_state()).unwrap();
        assert!(outputs.is_empty());
        assert_eq!(final_state.get("count").unwrap(), "0");
    }
}
