# monad-composition

**Monad composition patterns for agent pipelines.**

[![crates.io](https://img.shields.io/crates/v/monad-composition.svg)](https://crates.io/crates/monad-composition)
[![docs.rs](https://docs.rs/monad-composition/badge.svg)](https://docs.rs/monad-composition)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> "A monad is just a monoid in the category of endofunctors, what's the problem?"
> — Various attributions

---

## Table of Contents

1. [Overview](#overview)
2. [Why Monads for Agent Pipelines?](#why-monads-for-agent-pipelines)
3. [Theory](#theory)
   - [What is a Monad?](#what-is-a-monad)
   - [The Three Monad Laws](#the-three-monad-laws)
   - [Kleisli Composition](#kleisli-composition)
   - [Do-Notation Analogy](#do-notation-analogy)
4. [Architecture](#architecture)
   - [Monad Stack Diagram](#monad-stack-diagram)
   - [Module Overview](#module-overview)
   - [Design Decisions](#design-decisions)
5. [Modules](#modules)
   - [identity](#identity)
   - [maybe](#maybe)
   - [writer](#writer)
   - [state](#state)
   - [reader](#reader)
   - [composition](#composition)
6. [Examples](#examples)
   - [Example 1: Data Transformation Pipeline](#example-1-data-transformation-pipeline)
   - [Example 2: Resilient Agent with Logging](#example-2-resilient-agent-with-logging)
   - [Example 3: Full Composed Pipeline](#example-3-full-composed-pipeline)
7. [API Reference](#api-reference)
8. [References](#references)
9. [License](#license)

---

## Overview

`monad-composition` provides **concrete monad implementations** for building composable agent processing pipelines in Rust. Each module implements a specific monadic pattern — Identity, Maybe, Writer, State, and Reader — with a composition module that stacks them together.

This is **not** a generic monad trait library. There is no `trait Monad`. Instead, each module provides concrete struct types with `pure` and `bind` methods tailored to a specific use case in agent pipeline construction.

### Key Features

- **6 modules**, each implementing a concrete monad pattern
- **Zero external dependencies** (except `serde` for serialization)
- **62 passing tests** with full coverage of monad laws
- **Composition module** that stacks monads (Maybe+Writer, State+Maybe)
- **AgentPipeline** for composing monadic steps into runnable pipelines
- **Serde support**: all public types derive `Serialize` + `Deserialize`

---

## Why Monads for Agent Pipelines?

Agent pipelines face several recurring challenges:

| Challenge | Monad Pattern |
|---|---|
| Steps always succeed | Identity |
| Steps may fail (optional processing) | Maybe |
| Steps need to log decisions | Writer |
| Steps share mutable state | State |
| Steps read shared configuration | Reader |
| Steps need multiple effects | Composition |

Monads solve these by providing a uniform interface for chaining computations where each step can carry additional effects (logging, state, failure) without explicit plumbing.

Instead of writing:

```rust
// Without monads: manual plumbing everywhere
let (result1, log1) = step1(input);
if let Some(r1) = result1 {
    let (result2, log2) = step2(r1, &mut state, &config);
    if let Some(r2) = result2 {
        let combined_log = [log1, log2].concat();
        // ...
    }
}
```

You write:

```rust
// With monads: the effect handling is implicit
let result = MaybeWriter::writer(input, "start")
    .bind(|x| step1(x))   // Maybe: may fail
    .bind(|x| step2(x))   // Writer: logs decisions
    .bind(|x| step3(x));  // State: reads/writes state
```

---

## Theory

### What is a Monad?

A monad is a structure from category theory that was introduced into programming by Moggi (1991) to model computational effects. Wadler (1995) popularized monads for functional programming, showing how they could elegantly handle I/O, state, exceptions, and more.

Formally, a monad `M` over a type `T` consists of:

1. **A type constructor** `M<T>` that wraps a value of type `T`
2. **A `pure` (return/unit) operation**: `T → M<T>` that lifts a plain value into the monadic context
3. **A `bind` (flatMap/»=) operation**: `M<T> → (T → M<U>) → M<U>` that chains monadic computations

In category-theoretic terms (Awodey 2010), a monad on a category `C` is a functor `T: C → C` equipped with two natural transformations:

- `η: 1_C → T` (unit/pure)
- `μ: T² → T` (join/multiply)

satisfying the coherence conditions (monad laws).

### The Three Monad Laws

Every monad must satisfy three laws. These aren't just mathematical curiosities — they guarantee that monadic code composes predictably.

#### 1. Left Identity

```
pure(a).bind(f)  ≡  f(a)
```

Lifting a value and immediately binding a function is the same as just applying the function directly. `pure` doesn't add any effect.

#### 2. Right Identity

```
m.bind(pure)  ≡  m
```

Binding `pure` to a monadic value returns the original value unchanged. `bind` doesn't add any effect when the function is `pure`.

#### 3. Associativity

```
m.bind(f).bind(g)  ≡  m.bind(|x| f(x).bind(g))
```

The order of nesting binds doesn't matter. You can refactor a chain into a sub-computation without changing the result.

These laws are verified by tests in the `identity` module using concrete values and functions.

### Kleisli Composition

Given two monadic functions:

```
f: A → M<B>
g: B → M<C>
```

Their **Kleisli composition** `f >=> g` produces a new function `A → M<C>`:

```
(f >=> g)(a) = f(a).bind(g)
```

This is the fundamental composition operation. Our `bind` chains correspond directly to Kleisli composition. In the `composition` module, `Pipeline::run_pipeline` performs Kleisli composition over a sequence of steps.

The Kleisli category for monad `M` has:
- **Objects**: types (A, B, C, ...)
- **Morphisms**: Kleisli arrows `A → M<B>`
- **Identity**: `pure`
- **Composition**: `>=>` (Kleisli composition)

### Do-Notation Analogy

Languages like Haskell provide `do`-notation as syntactic sugar for monadic chains:

```haskell
-- Haskell do-notation
do
  x <- step1 input
  y <- step2 x
  z <- step3 y
  return z
```

In Rust, our `bind` chains serve the same purpose:

```rust
// Rust bind chain (equivalent to do-notation)
Identity::pure(input)
    .bind(|x| step1(x))    // x <- step1 input
    .bind(|y| step2(y))    // y <- step2 x
    .bind(|z| step3(z))    // z <- step3 y
    // implicit return z
```

Each `bind` corresponds to a `<-` binding in do-notation. The closure parameter is the bound variable. The final value in the chain is the return value.

---

## Architecture

### Monad Stack Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    Pipeline                              │
│  (composes all monadic steps into a single run_pipeline) │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   State      │  │    Reader    │  │    Maybe     │  │
│  │  (mutable)   │  │  (read-only) │  │ (short-      │  │
│  │  HashMap     │  │  HashMap     │  │  circuit)    │  │
│  │  <Str, Str>  │  │  <Str, Str>  │  │              │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                 │                  │          │
│         ▼                 ▼                  ▼          │
│  ┌─────────────────────────────────────────────────┐   │
│  │              PipelineStep                        │   │
│  │  execute(state, config, input) -> Result<String>│   │
│  └─────────────────────┬───────────────────────────┘   │
│                        │                                │
│         ┌──────────────┼──────────────┐                 │
│         ▼              ▼              ▼                 │
│  ┌────────────┐ ┌────────────┐ ┌──────────────┐        │
│  │  Identity  │ │   Writer   │ │  MaybeWriter │        │
│  │  (plain)   │ │  (logging) │ │ (fail+log)   │        │
│  └────────────┘ └────────────┘ └──────────────┘        │
│                                                          │
├─────────────────────────────────────────────────────────┤
│  Monad Transformers:                                     │
│  • MaybeWriter = Maybe + Writer (logging that can fail) │
│  • StateMaybe  = State + Maybe  (stateful with failure) │
└─────────────────────────────────────────────────────────┘
```

### Module Overview

| Module | Monad | Struct | Effect | Agent Type |
|--------|-------|--------|--------|------------|
| `identity` | Identity | `Identity<T>` | None (pure) | `AgentPipeline` |
| `maybe` | Maybe | `Maybe<T>` | Failure/absence | `AgentMaybe` |
| `writer` | Writer | `Writer<T>` | Logging | `AgentWriter` |
| `state` | State | `State<S, A>` | Mutable state | `AgentState` |
| `reader` | Reader | `Reader<E, A>` | Read-only env | `AgentReader` |
| `composition` | Transformers | `MaybeWriter<T>`, `StateMaybeStep`, `Pipeline` | Stacked effects | `Pipeline` |

### Design Decisions

1. **Concrete structs, no trait** — Rust's type system doesn't support Higher-Kinded Types (HKTs), making a generic `trait Monad` impractical. Instead, each module provides concrete types with `pure` and `bind` methods. This is idiomatic Rust and avoids the complexity of associated type gymnastics.

2. **`fn` pointers, not `Box<dyn Fn>`** — For simplicity and zero allocation overhead, function fields use `fn` pointers rather than boxed closures. This means closures can't capture environment variables, but it keeps the library dependency-free (no `alloc` patterns needed).

3. **`AgentResult<T> = Result<T, String>`** — A single, simple error type. No error enums, no error-chain. For a library focused on composition patterns, string errors are clear and sufficient.

4. **`HashMap<String, String>` for state and config** — Stringly-typed maps are the simplest possible shared state. Real applications would use typed structs, but for demonstrating monad patterns, this is maximally flexible.

5. **`serde` as the sole dependency** — All public types derive `Serialize` and `Deserialize`. This enables pipeline definitions to be serialized (e.g., loaded from JSON/YAML config files) while the function pointers are `#[serde(skip)]`.

6. **62 tests** — Every module has comprehensive tests including monad law verification (identity module), short-circuit behavior (maybe), log accumulation (writer), state threading (state), config access (reader), and stacked effects (composition).

---

## Modules

### identity

The simplest monad: `pure(x) = x`, `bind(x, f) = f(x)`.

```rust
use monad_composition::identity::{Identity, AgentPipeline, PipelineStep};

// Basic usage
let m = Identity::pure(42);
assert_eq!(m.run(), 42);

// Chained binds
let result = Identity::pure(10)
    .bind(|x| Identity::pure(x + 5))
    .bind(|x| Identity::pure(x * 2));
assert_eq!(result.run(), 30);

// AgentPipeline: chain Vec<String> -> Vec<String> transforms
let pipeline = AgentPipeline::new()
    .chain(PipelineStep::new("uppercase", |v| {
        v.into_iter().map(|s| s.to_uppercase()).collect()
    }))
    .chain(PipelineStep::new("sort", |mut v| { v.sort(); v }));

let output = pipeline.run(vec!["cherry".into(), "apple".into()]).unwrap();
assert_eq!(output, vec!["APPLE", "CHERRY"]);
```

The identity module also provides monad law verification functions: `left_identity`, `right_identity`, and `associativity`.

### maybe

Optional values with short-circuiting `bind`.

```rust
use monad_composition::maybe::{Maybe, AgentMaybe, run_maybe_pipeline};

// Some and None
let some = Maybe::pure(42);
let none: Maybe<i32> = Maybe::none();

// Bind short-circuits on None
let result = Maybe::pure(10)
    .bind(|x| if x > 5 { Maybe::pure(x) } else { Maybe::None })
    .bind(|x| Maybe::pure(x * 100));
assert_eq!(result, Maybe::Some(1000));

// Agent pipeline with short-circuiting
let steps = vec![
    AgentMaybe::new("validate", |s| {
        if !s.is_empty() { Maybe::Some(s.to_uppercase()) } else { Maybe::None }
    }),
    AgentMaybe::new("transform", |s| Maybe::Some(format!("{}!", s))),
];
let result = run_maybe_pipeline(&steps, "hello");
assert_eq!(result, Maybe::Some("HELLO!".to_string()));
```

### writer

Computations that produce a value and accumulate a log.

```rust
use monad_composition::writer::{Writer, AgentWriter, run_writer_pipeline};

// Basic writer
let w = Writer::with_log(42, "computed answer");
assert_eq!(w.value, 42);
assert_eq!(w.log, vec!["computed answer"]);

// Chained writers accumulate logs
let result = Writer::with_log(1, "step1")
    .bind(|x| Writer::with_log(x + 1, "step2"))
    .bind(|x| Writer::with_log(x * 10, "step3"));
assert_eq!(result.value, 20);
assert_eq!(result.log, vec!["step1", "step2", "step3"]);

// Agent writer pipeline
let steps = vec![
    AgentWriter::new("analyze", |s| {
        Writer::with_log(s.to_uppercase(), "converted to uppercase")
    }),
];
let result = run_writer_pipeline(&steps, "data").unwrap();
assert_eq!(result.value, "DATA");
assert_eq!(result.log, vec!["converted to uppercase"]);
```

### state

Computations that thread mutable state through a chain.

```rust
use std::collections::HashMap;
use monad_composition::state::{AgentState, run_state_pipeline, state_get, state_put};

let steps = vec![
    AgentState::new("init", |state| {
        state_put(state, "counter", "0");
        Ok("initialized".to_string())
    }),
    AgentState::new("increment", |state| {
        let count: i32 = state_get(state, "counter")?.parse().unwrap();
        state_put(state, "counter", &(count + 1).to_string());
        Ok(format!("count={}", count + 1))
    }),
];

let (outputs, final_state) = run_state_pipeline(&steps, HashMap::new()).unwrap();
assert_eq!(outputs, vec!["initialized", "count=1"]);
assert_eq!(final_state.get("counter").unwrap(), "1");
```

### reader

Read-only access to a shared configuration environment.

```rust
use std::collections::HashMap;
use monad_composition::reader::{AgentReader, run_reader_pipeline, config_get};

let config = HashMap::from([
    ("model".to_string(), "gpt-4".to_string()),
    ("temperature".to_string(), "0.7".to_string()),
]);

let steps = vec![
    AgentReader::new("get_model", |cfg| config_get(cfg, "model")),
    AgentReader::new("get_temp", |cfg| config_get(cfg, "temperature")),
];

let outputs = run_reader_pipeline(&steps, &config).unwrap();
assert_eq!(outputs, vec!["gpt-4", "0.7"]);
```

### composition

Monad transformers and stacked effects.

**MaybeWriter** — logging that can fail:

```rust
use monad_composition::composition::MaybeWriter;

let result = MaybeWriter::with_log(1, "start")
    .bind(|x| MaybeWriter::with_log(x + 1, "incremented"))
    .bind(|x| MaybeWriter::with_log(x * 10, "multiplied"));

let (val, log) = result.run().unwrap();
assert_eq!(val, 20);
assert_eq!(log, vec!["start", "incremented", "multiplied"]);
```

**StateMaybe** — stateful computation with failure:

```rust
use monad_composition::composition::{StateMaybeStep, run_state_maybe_pipeline};

let steps = vec![
    StateMaybeStep::new("set", |state| {
        state.insert("key".to_string(), "value".to_string());
        Ok("set".to_string())
    }),
    StateMaybeStep::new("get", |state| {
        state.get("key").cloned().ok_or("not found".to_string())
    }),
];
let (outputs, state) = run_state_maybe_pipeline(&steps, HashMap::new()).unwrap();
```

**Pipeline** — full composition:

See [Example 3](#example-3-full-composed-pipeline) below.

---

## Examples

### Example 1: Data Transformation Pipeline

A straightforward pipeline that transforms a list of strings through multiple steps.

```rust
use monad_composition::identity::{AgentPipeline, PipelineStep};

fn main() {
    let pipeline = AgentPipeline::new()
        // Step 1: Trim whitespace from each string
        .chain(PipelineStep::new("trim", |v| {
            v.into_iter().map(|s| s.trim().to_string()).collect()
        }))
        // Step 2: Filter out empty strings
        .chain(PipelineStep::new("filter_empty", |v| {
            v.into_iter().filter(|s| !s.is_empty()).collect()
        }))
        // Step 3: Uppercase everything
        .chain(PipelineStep::new("uppercase", |v| {
            v.into_iter().map(|s| s.to_uppercase()).collect()
        }))
        // Step 4: Sort alphabetically
        .chain(PipelineStep::new("sort", |mut v| {
            v.sort();
            v
        }))
        // Step 5: Add index prefix
        .chain(PipelineStep::new("index", |v| {
            v.into_iter()
                .enumerate()
                .map(|(i, s)| format!("{}. {}", i + 1, s))
                .collect()
        }));

    let input = vec![
        "  banana  ".to_string(),
        "".to_string(),
        " apple ".to_string(),
        "cherry".to_string(),
        "   ".to_string(),
        "date".to_string(),
    ];

    let output = pipeline.run(input).unwrap();

    for line in &output {
        println!("{}", line);
    }
    // Output:
    // 1. APPLE
    // 2. BANANA
    // 3. CHERRY
    // 4. DATE
}
```

### Example 2: Resilient Agent with Logging

An agent that processes data with optional steps, logging every decision.

```rust
use monad_composition::composition::{MaybeWriter, MonadType, Pipeline, PipelineStep};
use std::collections::HashMap;

fn main() {
    // Using MaybeWriter for a computation that logs and can fail
    let computation = MaybeWriter::with_log("raw data", "received input")
        .bind(|s| MaybeWriter::with_log(s.to_uppercase(), "normalized case"))
        .bind(|s| {
            if s.contains("ERROR") {
                MaybeWriter::none() // Short-circuit on errors
            } else {
                MaybeWriter::with_log(format!("processed:{}", s), "applied transformation")
            }
        })
        .bind(|s| MaybeWriter::with_log(format!("{} [done]", s), "finalized"));

    match computation.run() {
        Some((result, log)) => {
            println!("Result: {}", result);
            println!("Log:");
            for entry in &log {
                println!("  - {}", entry);
            }
        }
        None => {
            println!("Pipeline failed at some step");
        }
    }
}
```

### Example 3: Full Composed Pipeline

A complete pipeline combining State, Reader, Maybe, and Writer behaviors.

```rust
use monad_composition::composition::{MonadType, Pipeline, PipelineStep};
use std::collections::HashMap;

fn main() {
    let pipeline = Pipeline::new()
        // Step 1: Initialize state (State monad behavior)
        .add_step(PipelineStep::new(
            "init_context",
            MonadType::State,
            |state, _config, input| {
                state.insert("original_input".to_string(), input.to_string());
                state.insert("step_count".to_string(), "0".to_string());
                Ok(input.to_string())
            },
        ))
        // Step 2: Read config for processing mode (Reader monad behavior)
        .add_step(PipelineStep::new(
            "read_mode",
            MonadType::Reader,
            |_state, config, input| {
                let mode = config
                    .get("mode")
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());
                Ok(format!("[{}] {}", mode, input))
            },
        ))
        // Step 3: Transform based on mode (Maybe monad behavior - can fail)
        .add_step(PipelineStep::new(
            "transform",
            MonadType::Maybe,
            |_state, _config, input| {
                if input.contains("[production]") {
                    Ok(input.to_uppercase())
                } else if input.contains("[debug]") {
                    Ok(format!("DEBUG: {}", input))
                } else {
                    Err("Unknown mode - pipeline rejected".to_string())
                }
            },
        ))
        // Step 4: Finalize (Writer monad behavior - always logs)
        .add_step(PipelineStep::new(
            "finalize",
            MonadType::Writer,
            |state, _config, input| {
                let count: i32 = state
                    .get("step_count")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                state.insert("step_count".to_string(), (count + 1).to_string());
                Ok(format!("{} (processed in {} steps)", input, count + 1))
            },
        ));

    let config = HashMap::from([("mode".to_string(), "production".to_string())]);

    let result = pipeline
        .run_pipeline("important data", HashMap::new(), config)
        .unwrap();

    println!("Final output: {}", result.final_output);
    println!("All outputs: {:?}", result.all_outputs);
    println!("Final state: {:?}", result.final_state);
    println!("Execution log:");
    for log in &result.logs {
        println!("  {}", log);
    }

    // Final output: [PRODUCTION] IMPORTANT DATA (processed in 1 steps)
    // All outputs: ["important data", "[production] important data",
    //               "[PRODUCTION] IMPORTANT DATA",
    //               "[PRODUCTION] IMPORTANT DATA (processed in 1 steps)"]
}
```

---

## API Reference

### Core Types

| Type | Module | Description |
|------|--------|-------------|
| `Identity<T>` | `identity` | Identity monad wrapper |
| `AgentPipeline` | `identity` | Chain of `Vec<String> → Vec<String>` functions |
| `PipelineStep` | `identity` | Named step in an identity pipeline |
| `Maybe<T>` | `maybe` | Optional value monad (`Some` / `None`) |
| `AgentMaybe` | `maybe` | Optional processing step |
| `Writer<T>` | `writer` | Value with accumulated log |
| `AgentWriter` | `writer` | Processing step that logs decisions |
| `State<S, A>` | `state` | Stateful computation |
| `AgentState` | `state` | Stateful agent step |
| `Reader<E, A>` | `reader` | Read-only environment computation |
| `AgentReader` | `reader` | Read-only config access step |
| `MaybeWriter<T>` | `composition` | Maybe + Writer stack |
| `StateMaybeStep` | `composition` | State + Maybe step |
| `Pipeline` | `composition` | Full composed pipeline |
| `PipelineStep` | `composition` | Step in composed pipeline |
| `PipelineResult` | `composition` | Result of running a pipeline |
| `MonadType` | `composition` | Tag for monad type of a step |
| `AgentResult<T>` | `lib` | `Result<T, String>` for error handling |

### Common Methods

Every monad type provides:
- `pure(value)` — Lift a value into the monad
- `bind(self, f)` — Chain a monadic computation
- `map(self, f)` — Map over the contained value (where applicable)
- `run(self)` — Extract the result

---

## References

1. **Moggi, E.** (1991). "Notions of Computation and Monads." *Information and Computation*, 93(1), 55–92. — The foundational paper introducing monads as a framework for modeling computational effects in programming languages.

2. **Wadler, P.** (1995. "Monads for Functional Programming." In *Advanced Functional Programming*, Lecture Notes in Computer Science, vol 925. Springer. — Popularized monads for structuring functional programs, demonstrating their use for I/O, state, exceptions, and parsing.

3. **Awodey, S.** (2010). *Category Theory*. Oxford University Press, 2nd edition. — A comprehensive introduction to category theory, providing the mathematical foundations for understanding monads as endofunctors with natural transformations.

4. **Liang, S., Hudak, P., & Jones, M.** (1995). "Monad Transformers and Modular Interpreters." In *Proceedings of the 22nd ACM SIGPLAN-SIGACT Symposium on Principles of Programming Languages* (POPL '95). — Introduced monad transformers for stacking effects, the theoretical basis for our `composition` module.

5. **Pierce, B. C.** (2002). *Types and Programming Languages*. MIT Press. — Covers type systems and their relationship to monadic structures, providing broader context for understanding how monads integrate with typed languages.

6. **Abramsky, S., & Jung, A.** (1994). "Domain Theory." In *Handbook of Logic in Computer Science*, vol. 3. Oxford University Press. — Provides the denotational semantics foundations that underpin monadic interpretations of computation.

7. **Rust Documentation**. "Closures: Anonymous Functions that Capture their Environment." *The Rust Programming Language*. — Practical reference for how Rust closures interact with monadic patterns, particularly the difference between `fn` pointers and `Fn` traits.

---

## License

MIT License. See [LICENSE](LICENSE) for details.
