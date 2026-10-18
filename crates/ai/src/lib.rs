//! GridCraft AI assistant: local, offline, privacy-first intelligence for spreadsheets.
//!
//! This crate provides:
//! - Natural language → formula translation
//! - Smart suggestions based on data patterns
//! - Data analysis and anomaly detection
//!
//! All processing runs locally. No data leaves the machine.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod assistant;
pub mod natural_language;
pub mod prompt;
pub mod suggest;

pub use assistant::{Assistant, AssistantConfig, Backend, Response, RuleBasedBackend, SheetContext, WorkbookContext};
pub use natural_language::translate;
pub use suggest::{ColumnAnalysis, ColumnDataType, Suggestion, SuggestionKind, analyze_columns, analyze_sheet};

/// Error type for the GridCraft AI assistant crate.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// Failed to translate a natural language string into a valid formula.
    #[error("failed to translate natural language to formula: {0}")]
    Translation(String),
    /// Generated or input formula has invalid syntax.
    #[error("formula syntax error: {0}")]
    FormulaParse(#[from] gridcraft_formula::ParseError),
    /// Inference or backend execution error.
    #[error("backend error: {0}")]
    Backend(String),
    /// Invalid or malformed workbook context.
    #[error("invalid context: {0}")]
    InvalidContext(String),
    /// No actionable suggestion could be determined.
    #[error("no suggestion available: {0}")]
    NoSuggestion(String),
    /// Serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Miscellaneous error.
    #[error("other error: {0}")]
    Other(String),
}
