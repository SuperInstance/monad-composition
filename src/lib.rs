//! # monad-composition
//!
//! Monad composition patterns for agent pipelines.
//!
//! This crate provides concrete monad implementations for building composable
//! agent processing pipelines. Each module implements a specific monadic pattern
//! with laws verified by tests.

pub mod composition;
pub mod identity;
pub mod maybe;
pub mod reader;
pub mod state;
pub mod writer;

/// Agent result type for error handling throughout the crate.
pub type AgentResult<T> = Result<T, String>;
