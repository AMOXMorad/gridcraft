//! Core AI assistant logic and context management.
//!
//! Coordinates LLM backends and rule-based engines to answer spreadsheet questions,
//! generate formulas, and provide data-driven suggestions.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::AiError;
use crate::natural_language::translate;
use crate::suggest::{Suggestion, analyze_sheet};

/// Configuration for the AI assistant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistantConfig {
    /// Optional path to local GGUF/ONNX model weights.
    pub model_path: Option<PathBuf>,
    /// Context window size in tokens.
    pub context_window_size: usize,
    /// Sampling temperature (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
    /// Optional custom system prompt override.
    pub system_prompt: Option<String>,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        AssistantConfig { model_path: None, context_window_size: 4096, temperature: 0.2, system_prompt: None }
    }
}

/// Context summary for a single sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetContext {
    /// Sheet name.
    pub name: String,
    /// Used range string, e.g. `"A1:E50"`.
    pub used_range: Option<String>,
    /// Header row labels.
    pub headers: Vec<String>,
    /// First few rows of data as strings.
    pub sample_data: Vec<Vec<String>>,
    /// Total row count.
    pub total_rows: u32,
    /// Total column count.
    pub total_cols: u32,
}

impl SheetContext {
    /// Creates a new sheet context with the specified name.
    pub fn new(name: impl Into<String>) -> Self {
        SheetContext { name: name.into(), used_range: None, headers: Vec::new(), sample_data: Vec::new(), total_rows: 0, total_cols: 0 }
    }
}

/// Context summary of the current workbook state passed to the AI assistant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkbookContext {
    /// Active sheet name.
    pub active_sheet: String,
    /// Summaries of sheets in the workbook.
    pub sheets: Vec<SheetContext>,
    /// Current selection address if any (e.g. `"B2:D10"`).
    pub selection: Option<String>,
}

impl WorkbookContext {
    /// Creates a new workbook context with the specified active sheet name.
    pub fn new(active_sheet: impl Into<String>) -> Self {
        WorkbookContext { active_sheet: active_sheet.into(), sheets: Vec::new(), selection: None }
    }

    /// Returns the context for the currently active sheet if present.
    pub fn active_sheet_context(&self) -> Option<&SheetContext> {
        self.sheets.iter().find(|s| s.name == self.active_sheet)
    }
}

/// The response returned by the assistant to an inquiry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Response {
    /// A generated or translated formula (starting with `=`).
    Formula(String),
    /// A text explanation or analytical answer.
    Explanation(String),
    /// An actionable recommendation with an executable command.
    Suggestion(Suggestion),
    /// An error description when a request cannot be fulfilled.
    Error(String),
}

/// A pluggable LLM or inference backend.
pub trait Backend: Send + Sync {
    /// Generates a completion for the given prompt.
    fn complete(&self, prompt: &str) -> Result<String, AiError>;
}

/// A pure offline, rule-based backend requiring no local model weights or GPU.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuleBasedBackend;

impl RuleBasedBackend {
    /// Creates a new rule-based backend.
    pub fn new() -> Self {
        RuleBasedBackend
    }
}

impl Backend for RuleBasedBackend {
    fn complete(&self, prompt: &str) -> Result<String, AiError> {
        if let Some(idx) = prompt.find("Explain how this formula works") {
            let formula_part = prompt.get(idx..).unwrap_or("");
            return Ok(explain_formula_offline(formula_part));
        }

        if let Some(idx) = prompt.find("### User Question:\n") {
            let after = prompt.get(idx + 19..).unwrap_or("");
            let question = after.split("\n\n### Assistant:").next().unwrap_or(after).trim();
            if let Ok(formula) = translate(question) {
                return Ok(formula);
            }
        }

        Ok("Rule-based offline assistant ready. Ask for formulas (e.g. 'sum of column B') or smart suggestions.".into())
    }
}

/// Helper that generates rule-based formula explanations offline.
fn explain_formula_offline(text: &str) -> String {
    let upper = text.to_ascii_uppercase();
    let mut parts = Vec::new();

    if upper.contains("SUM(") {
        parts.push("SUM: Calculates the total arithmetic sum of the values in the range.");
    }
    if upper.contains("AVERAGE(") {
        parts.push("AVERAGE: Calculates the arithmetic mean of the numeric values in the range.");
    }
    if upper.contains("COUNT(") {
        parts.push("COUNT: Counts the number of cells in the range containing numbers.");
    }
    if upper.contains("COUNTA(") {
        parts.push("COUNTA: Counts the number of non-empty cells in the range.");
    }
    if upper.contains("MAX(") {
        parts.push("MAX: Finds the maximum numerical value in the range.");
    }
    if upper.contains("MIN(") {
        parts.push("MIN: Finds the minimum numerical value in the range.");
    }
    if upper.contains("IF(") {
        parts.push("IF: Evaluates a logical condition and branches to different outputs.");
    }
    if upper.contains("VLOOKUP(") {
        parts.push("VLOOKUP: Searches the first column of a table and retrieves a matching column value.");
    }

    if parts.is_empty() { "This formula computes values based on standard spreadsheet expressions.".into() } else { parts.join("\n") }
}

/// The main AI Assistant façade.
pub struct Assistant {
    config: AssistantConfig,
    backend: Box<dyn Backend>,
}

impl Assistant {
    /// Creates a new assistant with default configuration and the offline rule-based backend.
    pub fn new(config: AssistantConfig) -> Self {
        Assistant { config, backend: Box::new(RuleBasedBackend::new()) }
    }

    /// Creates an assistant with a custom LLM backend.
    pub fn with_backend(config: AssistantConfig, backend: Box<dyn Backend>) -> Self {
        Assistant { config, backend }
    }

    /// Returns the assistant's configuration.
    pub fn config(&self) -> &AssistantConfig {
        &self.config
    }

    /// Asks the assistant a question with workbook context.
    pub fn ask(&self, question: &str, context: &WorkbookContext) -> Result<Response, AiError> {
        let q = question.trim();
        if q.is_empty() {
            return Ok(Response::Error("Question is empty".into()));
        }

        let q_lower = q.to_ascii_lowercase();

        // 1. Check if the user is asking for smart suggestions or analysis
        if q_lower.starts_with("suggest")
            || q_lower.starts_with("recommend")
            || q_lower.contains("what should i do")
            || q_lower.contains("analyze sheet")
            || q_lower.contains("give me suggestions")
        {
            if let Some(sheet) = context.active_sheet_context().or_else(|| context.sheets.first()) {
                let suggestions = analyze_sheet(sheet);
                if let Some(top) = suggestions.into_iter().next() {
                    return Ok(Response::Suggestion(top));
                }
            }
            return Ok(Response::Explanation("No specific suggestions found for the current data structure.".into()));
        }

        // 2. Direct natural language to formula translation
        if let Ok(formula) = translate(q) {
            return Ok(Response::Formula(formula));
        }

        // 3. Query LLM backend
        let prompt = crate::prompt::build_prompt(q, context);
        let raw = self.backend.complete(&prompt)?;
        let trimmed = raw.trim();

        if trimmed.starts_with('=') && gridcraft_formula::parse(trimmed).is_ok() {
            return Ok(Response::Formula(trimmed.to_string()));
        }

        Ok(Response::Explanation(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockBackend(String);

    impl Backend for MockBackend {
        fn complete(&self, _: &str) -> Result<String, AiError> {
            Ok(self.0.clone())
        }
    }

    #[test]
    fn assistant_translates_sum_query() {
        let assistant = Assistant::new(AssistantConfig::default());
        let ctx = WorkbookContext::new("Sheet1");
        let resp = assistant.ask("sum of column B", &ctx).unwrap();
        assert_eq!(resp, Response::Formula("=SUM(B:B)".into()));
    }

    #[test]
    fn assistant_provides_suggestion() {
        let assistant = Assistant::new(AssistantConfig::default());
        let mut ctx = WorkbookContext::new("Sales");
        ctx.sheets.push(SheetContext {
            name: "Sales".into(),
            used_range: Some("A1:B10".into()),
            headers: vec!["Region".into(), "Revenue".into()],
            sample_data: vec![vec!["North".into(), "100".into()]],
            total_rows: 10,
            total_cols: 2,
        });

        let resp = assistant.ask("suggest something", &ctx).unwrap();
        assert!(matches!(resp, Response::Suggestion(_)));
    }

    #[test]
    fn assistant_with_custom_backend() {
        let backend = Box::new(MockBackend("=AVERAGE(A1:A10)".into()));
        let assistant = Assistant::with_backend(AssistantConfig::default(), backend);
        let ctx = WorkbookContext::new("Sheet1");
        let resp = assistant.ask("what is the average formula?", &ctx).unwrap();
        assert_eq!(resp, Response::Formula("=AVERAGE(A1:A10)".into()));
    }
}
