//! Prompt templates and system prompts for spreadsheet AI tasks.
//!
//! Provides system prompts, context formatters, and few-shot examples that describe
//! GridCraft's formula conventions, syntax, and capabilities.

use crate::assistant::WorkbookContext;

/// System prompt describing GridCraft's formula syntax and operational conventions.
pub const SYSTEM_PROMPT: &str = "\
You are GridCraft Assistant, an intelligent spreadsheet copilot running locally and privately.
You assist users with formulas, data analysis, modeling, and spreadsheet workflows.

### Formula Rules:
1. Always use standard Excel-compatible uppercase function names:
   - Aggregations: SUM, AVERAGE, COUNT, COUNTA, COUNTIF, COUNTIFS, SUMIF, SUMIFS, MAX, MIN, MEDIAN
   - Logic: IF, IFS, AND, OR, NOT, IFERROR, SWITCH
   - Lookups: XLOOKUP, VLOOKUP, HLOOKUP, INDEX, MATCH
   - Text: CONCAT, TEXTJOIN, LEFT, RIGHT, MID, LEN, TRIM, UPPER, LOWER, TEXT
   - Date & Time: DATE, TODAY, NOW, YEAR, MONTH, DAY, EDATE, EOMONTH, DATEDIF
   - Math: ROUND, ROUNDUP, ROUNDDOWN, ABS, MOD, POWER, SQRT
2. Cell references follow standard A1 notation:
   - Relative: A1, B2, C1:C100
   - Absolute: $A$1, $B$2:$D$50
   - Full column/row references: A:A, B:D, 1:1, 5:10
   - Sheet qualified: Sheet1!A1, 'Quarterly Sales'!B2:B20
3. When asked to write a formula, start with an equals sign (`=`).
4. Keep formulas concise, efficient, and robust against errors.
5. If the request is ambiguous, provide the most likely formula with a brief explanation.
";

/// A few-shot example for LLM prompt context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromptExample {
    /// Category or task description.
    pub task: &'static str,
    /// Relevant spreadsheet context or column structure.
    pub context: &'static str,
    /// Natural language user question or instruction.
    pub user_query: &'static str,
    /// Expected assistant response.
    pub assistant_response: &'static str,
}

/// Standard few-shot examples for common spreadsheet tasks.
pub const FEW_SHOT_EXAMPLES: &[PromptExample] = &[
    PromptExample {
        task: "Column Aggregation",
        context: "Sheet: Sales | Headers: [Date, Region, Salesperson, Revenue, Units]",
        user_query: "Calculate total revenue",
        assistant_response: "=SUM(D:D)",
    },
    PromptExample {
        task: "Conditional Sum",
        context: "Sheet: Sales | Headers: [Date, Region, Salesperson, Revenue, Units]",
        user_query: "Total revenue for the North region",
        assistant_response: "=SUMIF(B:B, \"North\", D:D)",
    },
    PromptExample {
        task: "Lookup",
        context: "Sheet: Invoices | Headers: [ID, Customer, ProductCode, Total] | Sheet: Products | Headers: [Code, Name, Price]",
        user_query: "Look up product price for ProductCode in C2",
        assistant_response: "=XLOOKUP(C2, Products!A:A, Products!C:C, \"Not Found\")",
    },
    PromptExample {
        task: "Classification Logic",
        context: "Sheet: Grades | Headers: [Student, Score, Status]",
        user_query: "If Score in B2 is 70 or higher set Status to Pass, otherwise Fail",
        assistant_response: "=IF(B2>=70, \"Pass\", \"Fail\")",
    },
    PromptExample {
        task: "Growth Rate",
        context: "Sheet: Financials | Headers: [Quarter, PriorRevenue, CurrentRevenue]",
        user_query: "Percentage growth from prior revenue (B2) to current revenue (C2)",
        assistant_response: "=(C2-B2)/B2*100",
    },
];

/// Formats a [`WorkbookContext`] into a concise markdown text summary for inclusion in prompts.
pub fn format_context(context: &WorkbookContext) -> String {
    let mut out = String::new();
    out.push_str("### Current Workbook Context:\n");
    out.push_str(&format!("- Active Sheet: {}\n", context.active_sheet));

    if let Some(ref sel) = context.selection {
        out.push_str(&format!("- Current Selection: {}\n", sel));
    }

    out.push_str("\n### Sheets:\n");
    for sheet in &context.sheets {
        out.push_str(&format!("#### Sheet: `{}`\n", sheet.name));
        if let Some(ref range) = sheet.used_range {
            out.push_str(&format!("- Used Range: {}\n", range));
        }
        if sheet.total_rows > 0 || sheet.total_cols > 0 {
            out.push_str(&format!("- Dimensions: {} rows x {} columns\n", sheet.total_rows, sheet.total_cols));
        }
        if !sheet.headers.is_empty() {
            out.push_str(&format!("- Headers: [{}]\n", sheet.headers.join(", ")));
        }
        if !sheet.sample_data.is_empty() {
            out.push_str("- Sample Data (first rows):\n");
            for row in sheet.sample_data.iter().take(5) {
                out.push_str(&format!("  | {} |\n", row.join(" | ")));
            }
        }
        out.push('\n');
    }

    out
}

/// Builds a complete LLM prompt combining system instructions, workbook context, few-shot examples, and the user question.
pub fn build_prompt(question: &str, context: &WorkbookContext) -> String {
    let mut prompt = String::new();
    prompt.push_str(SYSTEM_PROMPT);
    prompt.push_str("\n---\n\n");
    prompt.push_str(&format_context(context));
    prompt.push_str("\n### Examples:\n");
    for ex in FEW_SHOT_EXAMPLES {
        prompt.push_str(&format!("Context: {}\nUser: {}\nAssistant: {}\n\n", ex.context, ex.user_query, ex.assistant_response));
    }
    prompt.push_str("### User Question:\n");
    prompt.push_str(question);
    prompt.push_str("\n\n### Assistant:\n");
    prompt
}

/// Builds a targeted prompt for natural language to formula translation.
pub fn build_formula_prompt(request: &str, context: &WorkbookContext) -> String {
    let mut prompt = String::new();
    prompt.push_str(SYSTEM_PROMPT);
    prompt.push_str("\nTask: Translate the following natural language request into a valid Excel/GridCraft formula.\n");
    prompt.push_str("Respond ONLY with the formula starting with `=`, and optionally a one-line explanation below it.\n\n");
    prompt.push_str(&format_context(context));
    prompt.push_str(&format!("\nRequest: {}\nFormula:\n", request));
    prompt
}

/// Builds a prompt requesting an explanation of an existing formula.
pub fn build_explain_prompt(formula: &str) -> String {
    format!(
        "{}\n\nTask: Explain how this formula works step-by-step in clear, accessible language:\n\n`{}`\n\nExplanation:\n",
        SYSTEM_PROMPT, formula
    )
}

/// Builds a prompt requesting smart suggestions and anomaly detection on the workbook.
pub fn build_analysis_prompt(context: &WorkbookContext) -> String {
    let mut prompt = String::new();
    prompt.push_str(SYSTEM_PROMPT);
    prompt.push_str("\nTask: Analyze the spreadsheet structure and data. Suggest useful summaries, charts, formatting, and identify any potential anomalies or missing patterns.\n\n");
    prompt.push_str(&format_context(context));
    prompt.push_str("\nSuggestions:\n");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::SheetContext;

    #[test]
    fn system_prompt_is_valid() {
        assert!(SYSTEM_PROMPT.contains("GridCraft Assistant"));
        assert!(SYSTEM_PROMPT.contains("SUM"));
    }

    #[test]
    fn format_context_includes_sheet_info() {
        let mut ctx = WorkbookContext::new("Sales");
        ctx.selection = Some("B2:D10".into());
        ctx.sheets.push(SheetContext {
            name: "Sales".into(),
            used_range: Some("A1:E50".into()),
            headers: vec!["Date".into(), "Region".into(), "Revenue".into()],
            sample_data: vec![vec!["2026-01-01".into(), "North".into(), "1500".into()]],
            total_rows: 50,
            total_cols: 3,
        });

        let formatted = format_context(&ctx);
        assert!(formatted.contains("Active Sheet: Sales"));
        assert!(formatted.contains("Current Selection: B2:D10"));
        assert!(formatted.contains("Headers: [Date, Region, Revenue]"));
        assert!(formatted.contains("2026-01-01 | North | 1500"));
    }

    #[test]
    fn build_prompt_contains_question_and_context() {
        let ctx = WorkbookContext::new("Sheet1");
        let prompt = build_prompt("Sum column B", &ctx);
        assert!(prompt.contains("Sum column B"));
        assert!(prompt.contains("Active Sheet: Sheet1"));
    }
}
