//! Identity monad — the simplest monad that does nothing.
//!
//! `pure(x) = x`, `bind(x, f) = f(x)`.
//! Useful as a baseline and for pipeline steps that pass data through unchanged.

use serde::{Deserialize, Serialize};

use crate::AgentResult;

/// Identity monad wrapper.
///
/// Wraps a value with no additional structure. Every operation is transparent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity<T> {
    pub value: T,
}

impl<T> Identity<T> {
    /// Create a new Identity monad wrapping `value`.
    pub fn pure(value: T) -> Self {
        Identity { value }
    }

    /// Bind (flatMap): apply `f` to the contained value.
    pub fn bind<U, F>(self, f: F) -> Identity<U>
    where
        F: FnOnce(T) -> Identity<U>,
    {
        f(self.value)
    }

    /// Map a function over the contained value.
    pub fn map<U, F>(self, f: F) -> Identity<U>
    where
        F: FnOnce(T) -> U,
    {
        Identity::pure(f(self.value))
    }

    /// Extract the contained value.
    pub fn run(self) -> T {
        self.value
    }
}

// --- Identity monad law checks ---

/// Verify left identity: `pure(a).bind(f) == f(a)`
pub fn left_identity<A, B, F>(a: A, f: F) -> bool
where
    A: Clone + PartialEq,
    B: PartialEq,
    F: Fn(A) -> Identity<B>,
{
    Identity::pure(a.clone()).bind(&f).run() == f(a).run()
}

/// Verify right identity: `m.bind(pure) == m`
pub fn right_identity<A>(m: Identity<A>) -> bool
where
    A: Clone + PartialEq,
{
    m.clone().bind(Identity::pure).run() == m.run()
}

/// Verify associativity: `m.bind(f).bind(g) == m.bind(|x| f(x).bind(g))`
pub fn associativity<A, B, C, F, G>(m: Identity<A>, f: F, g: G) -> bool
where
    A: Clone + PartialEq,
    C: PartialEq,
    F: Fn(A) -> Identity<B>,
    G: Fn(B) -> Identity<C>,
{
    m.clone().bind(&f).bind(&g).run() == m.bind(|x| f(x).bind(g)).run()
}

// --- AgentPipeline ---

/// An agent pipeline that chains `Vec<String> -> Vec<String>` functions.
///
/// Each step transforms a list of strings. The identity monad ensures
/// every step produces exactly one output — no short-circuiting, no logging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPipeline {
    /// Named steps in the pipeline.
    pub steps: Vec<PipelineStep>,
}

/// A named step in an agent pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStep {
    pub name: String,
    #[serde(skip)]
    #[allow(clippy::type_complexity)]
    pub transform: Option<fn(Vec<String>) -> Vec<String>>,
}

impl PipelineStep {
    /// Create a new pipeline step with a name and transform function.
    pub fn new(name: &str, transform: fn(Vec<String>) -> Vec<String>) -> Self {
        PipelineStep {
            name: name.to_string(),
            transform: Some(transform),
        }
    }

    /// A no-op step that passes input through unchanged.
    pub fn noop(name: &str) -> Self {
        PipelineStep {
            name: name.to_string(),
            transform: Some(|x| x),
        }
    }
}

impl Default for AgentPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        AgentPipeline { steps: vec![] }
    }

    /// Add a step to the pipeline.
    pub fn chain(mut self, step: PipelineStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Run the pipeline: feed input through every step in order.
    pub fn run(&self, input: Vec<String>) -> AgentResult<Vec<String>> {
        let mut current = input;
        for step in &self.steps {
            if let Some(f) = step.transform {
                current = f(current);
            }
        }
        Ok(current)
    }

    /// Run using Identity monad composition explicitly.
    pub fn run_monadic(&self, input: Vec<String>) -> AgentResult<Vec<String>> {
        let identity = Identity::pure(input);
        let result = self.steps.iter().fold(identity, |acc, step| {
            acc.bind(|data| {
                if let Some(f) = step.transform {
                    Identity::pure(f(data))
                } else {
                    Identity::pure(data)
                }
            })
        });
        Ok(result.run())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_and_run() {
        let m = Identity::pure(42);
        assert_eq!(m.run(), 42);
    }

    #[test]
    fn test_bind() {
        let m = Identity::pure(10);
        let result = m.bind(|x| Identity::pure(x + 5));
        assert_eq!(result.run(), 15);
    }

    #[test]
    fn test_map() {
        let m = Identity::pure("hello");
        let result = m.map(|s| s.to_uppercase());
        assert_eq!(result.run(), "HELLO");
    }

    #[test]
    fn test_left_identity_law() {
        let f = |x: i32| Identity::pure(x * 2);
        assert!(left_identity(5, f));
    }

    #[test]
    fn test_right_identity_law() {
        let m = Identity::pure(99);
        assert!(right_identity(m));
    }

    #[test]
    fn test_associativity_law() {
        let m = Identity::pure(3);
        let f = |x: i32| Identity::pure(x + 1);
        let g = |x: i32| Identity::pure(x * 10);
        assert!(associativity(m, f, g));
    }

    #[test]
    fn test_pipeline_basic() {
        let pipe = AgentPipeline::new()
            .chain(PipelineStep::new("uppercase", |v| {
                v.into_iter().map(|s| s.to_uppercase()).collect()
            }))
            .chain(PipelineStep::new("sort", |mut v| {
                v.sort();
                v
            }));
        let result = pipe
            .run(vec!["cherry".into(), "apple".into(), "banana".into()])
            .unwrap();
        assert_eq!(result, vec!["APPLE", "BANANA", "CHERRY"]);
    }

    #[test]
    fn test_pipeline_monadic() {
        let pipe = AgentPipeline::new().chain(PipelineStep::new("add_prefix", |v| {
            v.into_iter().map(|s| format!("item:{}", s)).collect()
        }));
        let result = pipe.run_monadic(vec!["a".into(), "b".into()]).unwrap();
        assert_eq!(result, vec!["item:a", "item:b"]);
    }

    #[test]
    fn test_pipeline_empty() {
        let pipe = AgentPipeline::new();
        let result = pipe.run(vec!["x".into()]).unwrap();
        assert_eq!(result, vec!["x"]);
    }

    #[test]
    fn test_identity_chain_multiple_binds() {
        let result = Identity::pure(1)
            .bind(|x| Identity::pure(x + 1))
            .bind(|x| Identity::pure(x * 3))
            .bind(|x| Identity::pure(x - 2));
        assert_eq!(result.run(), 4);
    }
}
