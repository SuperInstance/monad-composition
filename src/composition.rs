//! Monad transformers and composition — stacking monads for richer semantics.
//!
//! Provides `MaybeWriter` (Maybe + Writer), `StateMaybe` (State + Maybe),
//! and a general-purpose `Pipeline` for composing monadic agent steps.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::AgentResult;
use crate::maybe::Maybe;

// --- MaybeWriter: logging that can fail ---

/// MaybeWriter monad: a computation that may fail (Maybe) and logs decisions (Writer).
///
/// Stack: Maybe<Writer> — if the computation succeeds, you get a value + log;
/// if it fails at any step, you get `None` and partial logs are lost.
///
/// In practice we model this as `Maybe<(T, Vec<String>)>` for simplicity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaybeWriter<T> {
    pub inner: Maybe<(T, Vec<String>)>,
}

impl<T> MaybeWriter<T> {
    /// Create a successful computation with a value and empty log.
    pub fn pure(value: T) -> Self {
        MaybeWriter {
            inner: Maybe::Some((value, vec![])),
        }
    }

    /// Create a successful computation with a log entry.
    pub fn writer(value: T, msg: &str) -> Self {
        MaybeWriter {
            inner: Maybe::Some((value, vec![msg.to_string()])),
        }
    }

    /// Create a failed computation.
    pub fn none() -> Self {
        MaybeWriter { inner: Maybe::None }
    }

    /// Bind: chain MaybeWriter computations. Short-circuits on failure.
    pub fn bind<U, F>(self, f: F) -> MaybeWriter<U>
    where
        F: FnOnce(T) -> MaybeWriter<U>,
    {
        match self.inner {
            Maybe::Some((value, mut log)) => {
                let next = f(value);
                match next.inner {
                    Maybe::Some((next_val, mut next_log)) => {
                        log.append(&mut next_log);
                        MaybeWriter {
                            inner: Maybe::Some((next_val, log)),
                        }
                    }
                    Maybe::None => MaybeWriter::none(),
                }
            }
            Maybe::None => MaybeWriter::none(),
        }
    }

    /// Extract the result. Returns None if the computation failed.
    pub fn run(self) -> Option<(T, Vec<String>)> {
        match self.inner {
            Maybe::Some((v, log)) => Some((v, log)),
            Maybe::None => None,
        }
    }

    /// Check if this computation succeeded.
    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    /// Check if this computation failed.
    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }
}

// --- StateMaybe: stateful computations that can fail ---

/// A stateful computation step that can fail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMaybeStep {
    pub name: String,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub process: Option<fn(&mut HashMap<String, String>) -> AgentResult<String>>,
}

impl StateMaybeStep {
    /// Create a new StateMaybe step.
    pub fn new(
        name: &str,
        process: fn(&mut HashMap<String, String>) -> AgentResult<String>,
    ) -> Self {
        StateMaybeStep {
            name: name.to_string(),
            process: Some(process),
        }
    }
}

/// Run a stateful pipeline where any step can fail.
///
/// Returns `Ok((outputs, final_state))` on success, or `Err` with the
/// step name and error message on failure. State is preserved up to the failure point.
pub fn run_state_maybe_pipeline(
    steps: &[StateMaybeStep],
    initial_state: HashMap<String, String>,
) -> AgentResult<(Vec<String>, HashMap<String, String>)> {
    let mut state = initial_state;
    let mut outputs = vec![];
    for step in steps {
        if let Some(f) = step.process {
            let result = f(&mut state)
                .map_err(|e| format!("StateMaybe pipeline failed at '{}': {}", step.name, e))?;
            outputs.push(result);
        }
    }
    Ok((outputs, state))
}

// --- Pipeline: general monadic agent pipeline ---

/// A pipeline step that can do anything: transform data, read config, modify state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    pub monad_type: MonadType,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub execute: Option<
        fn(&mut HashMap<String, String>, &HashMap<String, String>, &str) -> AgentResult<String>,
    >,
}

/// What kind of monadic behavior a pipeline step has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonadType {
    /// Identity: always succeeds, no logging.
    Identity,
    /// Maybe: can short-circuit.
    Maybe,
    /// Writer: logs decisions.
    Writer,
    /// State: reads and writes shared state.
    State,
    /// Reader: read-only config access.
    Reader,
}

/// A composed agent pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub steps: Vec<PipelineStep>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Pipeline { steps: vec![] }
    }

    /// Add a step to the pipeline.
    pub fn add_step(mut self, step: PipelineStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Run the pipeline with shared state, read-only config, and initial input.
    ///
    /// Each step receives:
    /// - `&mut state` — mutable shared state (State monad behavior)
    /// - `&config` — read-only config (Reader monad behavior)
    /// - `&input` — the current data string (Identity/Maybe/Writer behavior)
    ///
    /// Steps that fail return an error, which short-circuits the pipeline.
    /// All outputs are collected.
    pub fn run_pipeline(
        &self,
        initial_input: &str,
        initial_state: HashMap<String, String>,
        config: HashMap<String, String>,
    ) -> AgentResult<PipelineResult> {
        let mut state = initial_state;
        let mut current_input = initial_input.to_string();
        let mut outputs = vec![];
        let mut logs = vec![];

        for step in &self.steps {
            if let Some(f) = step.execute {
                let result = f(&mut state, &config, &current_input)
                    .map_err(|e| format!("Pipeline failed at step '{}': {}", step.name, e))?;
                logs.push(format!(
                    "[{}] {} -> {}",
                    step.monad_label(),
                    step.name,
                    result
                ));
                current_input = result.clone();
                outputs.push(result);
            }
        }

        Ok(PipelineResult {
            final_output: current_input,
            all_outputs: outputs,
            final_state: state,
            logs,
        })
    }
}

impl PipelineStep {
    /// Create a new pipeline step.
    #[allow(clippy::type_complexity)]
    pub fn new(
        name: &str,
        monad_type: MonadType,
        execute: fn(
            &mut HashMap<String, String>,
            &HashMap<String, String>,
            &str,
        ) -> AgentResult<String>,
    ) -> Self {
        PipelineStep {
            name: name.to_string(),
            monad_type,
            execute: Some(execute),
        }
    }

    fn monad_label(&self) -> &'static str {
        match self.monad_type {
            MonadType::Identity => "Identity",
            MonadType::Maybe => "Maybe",
            MonadType::Writer => "Writer",
            MonadType::State => "State",
            MonadType::Reader => "Reader",
        }
    }
}

/// The result of running a composed pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    /// The output of the last step.
    pub final_output: String,
    /// Outputs from all steps, in order.
    pub all_outputs: Vec<String>,
    /// The final state after all State steps.
    pub final_state: HashMap<String, String>,
    /// Execution log from all steps.
    pub logs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- MaybeWriter tests ---

    #[test]
    fn test_maybe_writer_pure() {
        let mw = MaybeWriter::pure(42);
        let (val, log) = mw.run().unwrap();
        assert_eq!(val, 42);
        assert!(log.is_empty());
    }

    #[test]
    fn test_maybe_writer_with_log() {
        let mw = MaybeWriter::writer(10, "created");
        let (val, log) = mw.run().unwrap();
        assert_eq!(val, 10);
        assert_eq!(log, vec!["created"]);
    }

    #[test]
    fn test_maybe_writer_none() {
        let mw: MaybeWriter<i32> = MaybeWriter::none();
        assert!(mw.run().is_none());
    }

    #[test]
    fn test_maybe_writer_bind_success() {
        let result = MaybeWriter::writer(1, "step1")
            .bind(|x| MaybeWriter::writer(x + 1, "step2"))
            .bind(|x| MaybeWriter::writer(x * 10, "step3"));
        let (val, log) = result.run().unwrap();
        assert_eq!(val, 20);
        assert_eq!(log, vec!["step1", "step2", "step3"]);
    }

    #[test]
    fn test_maybe_writer_bind_short_circuit() {
        let result = MaybeWriter::writer(1, "step1")
            .bind(|_: i32| MaybeWriter::none())
            .bind(|x: i32| MaybeWriter::writer(x, "never"));
        assert!(result.is_none());
    }

    #[test]
    fn test_maybe_writer_is_some_none() {
        assert!(MaybeWriter::pure(1).is_some());
        assert!(MaybeWriter::<i32>::none().is_none());
    }

    // --- StateMaybe tests ---

    #[test]
    fn test_state_maybe_pipeline_success() {
        let steps = vec![
            StateMaybeStep::new("set", |state| {
                state.insert("key".to_string(), "value".to_string());
                Ok("set".to_string())
            }),
            StateMaybeStep::new("get", |state| {
                state.get("key").cloned().ok_or("not found".to_string())
            }),
        ];
        let (outputs, final_state) = run_state_maybe_pipeline(&steps, HashMap::new()).unwrap();
        assert_eq!(outputs, vec!["set", "value"]);
        assert_eq!(final_state.get("key").unwrap(), "value");
    }

    #[test]
    fn test_state_maybe_pipeline_failure() {
        let steps = vec![StateMaybeStep::new("fail", |_state| {
            Err("intentional failure".to_string())
        })];
        let result = run_state_maybe_pipeline(&steps, HashMap::new());
        assert!(result.is_err());
    }

    // --- Pipeline tests ---

    #[test]
    fn test_pipeline_basic() {
        let pipe = Pipeline::new().add_step(PipelineStep::new(
            "identity_uppercase",
            MonadType::Identity,
            |_state, _config, input| Ok(input.to_uppercase()),
        ));
        let result = pipe
            .run_pipeline("hello", HashMap::new(), HashMap::new())
            .unwrap();
        assert_eq!(result.final_output, "HELLO");
        assert_eq!(result.all_outputs, vec!["HELLO"]);
    }

    #[test]
    fn test_pipeline_multi_step() {
        let pipe = Pipeline::new()
            .add_step(PipelineStep::new(
                "state_init",
                MonadType::State,
                |state, _config, _input| {
                    state.insert("count".to_string(), "1".to_string());
                    Ok("initialized".to_string())
                },
            ))
            .add_step(PipelineStep::new(
                "reader_check",
                MonadType::Reader,
                |_state, config, _input| {
                    let mode = config
                        .get("mode")
                        .cloned()
                        .unwrap_or_else(|| "default".to_string());
                    Ok(format!("mode={}", mode))
                },
            ))
            .add_step(PipelineStep::new(
                "maybe_validate",
                MonadType::Maybe,
                |_state, _config, input| {
                    if input.contains("mode=") {
                        Ok(format!("validated:{}", input))
                    } else {
                        Err("validation failed".to_string())
                    }
                },
            ));
        let mut config = HashMap::new();
        config.insert("mode".to_string(), "production".to_string());
        let result = pipe.run_pipeline("start", HashMap::new(), config).unwrap();
        assert_eq!(result.final_output, "validated:mode=production");
        assert_eq!(result.all_outputs.len(), 3);
        assert_eq!(result.final_state.get("count").unwrap(), "1");
    }

    #[test]
    fn test_pipeline_failure() {
        let pipe = Pipeline::new().add_step(PipelineStep::new(
            "fail_step",
            MonadType::Maybe,
            |_state, _config, _input| Err("boom".to_string()),
        ));
        let result = pipe.run_pipeline("x", HashMap::new(), HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fail_step"));
    }

    #[test]
    fn test_pipeline_empty() {
        let pipe = Pipeline::new();
        let result = pipe
            .run_pipeline("input", HashMap::new(), HashMap::new())
            .unwrap();
        assert_eq!(result.final_output, "input");
        assert!(result.all_outputs.is_empty());
    }

    #[test]
    fn test_pipeline_result_logs() {
        let pipe = Pipeline::new()
            .add_step(PipelineStep::new(
                "step1",
                MonadType::Identity,
                |_s, _c, input| Ok(format!("{}_1", input)),
            ))
            .add_step(PipelineStep::new(
                "step2",
                MonadType::Identity,
                |_s, _c, input| Ok(format!("{}_2", input)),
            ));
        let result = pipe
            .run_pipeline("x", HashMap::new(), HashMap::new())
            .unwrap();
        assert_eq!(result.logs.len(), 2);
        assert!(result.logs[0].contains("step1"));
        assert!(result.logs[1].contains("step2"));
    }
}
