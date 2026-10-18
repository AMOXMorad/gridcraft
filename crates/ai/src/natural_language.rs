//! Natural language to formula translation engine.
//!
//! Provides rule-based offline translation of natural language queries into
//! valid Excel/GridCraft formulas, with full syntax validation.

use gridcraft_core::addr::letters_to_col;
use gridcraft_core::{CellRef, RangeRef};

use crate::AiError;

/// Translates a natural language query into a valid formula string.
///
/// Returns a string starting with `=` that has been validated against GridCraft's formula parser.
///
/// # Examples
/// ```
/// use gridcraft_ai::natural_language::translate;
///
/// assert_eq!(translate("sum of column B").unwrap(), "=SUM(B:B)");
/// assert_eq!(translate("average of A1 to A10").unwrap(), "=AVERAGE(A1:A10)");
/// assert_eq!(translate("if A1 > 10 then High else Low").unwrap(), "=IF(A1>10, \"High\", \"Low\")");
/// ```
pub fn translate(query: &str) -> Result<String, AiError> {
    let cleaned = clean_query(query);
    if cleaned.is_empty() {
        return Err(AiError::Translation("empty natural language query".into()));
    }

    let formula = match match_rules(&cleaned) {
        Some(f) => f,
        None => fallback_heuristic(&cleaned)?,
    };

    // Validate formula syntax with GridCraft's parser
    gridcraft_formula::parse(&formula).map_err(AiError::FormulaParse)?;

    Ok(formula)
}

/// Cleans and normalizes query text: trims whitespace, strips punctuation, conversational filler.
fn clean_query(input: &str) -> String {
    let mut s = input.trim();
    // Strip leading = or +
    s = s.strip_prefix('=').or_else(|| s.strip_prefix('+')).unwrap_or(s).trim();
    // Strip trailing punctuation
    s = s.trim_end_matches(['?', '.', '!', ';']).trim();

    // Strip common conversational prefixes
    let lower = s.to_ascii_lowercase();
    let prefixes = [
        "calculate the ",
        "calculate ",
        "what is the ",
        "what's the ",
        "give me the ",
        "give me ",
        "write a formula for ",
        "write formula for ",
        "formula for ",
        "formula to ",
        "how to calculate ",
        "can you calculate ",
        "please calculate ",
        "please give me ",
    ];

    for prefix in prefixes {
        if lower.starts_with(prefix) {
            let offset = prefix.len();
            s = s.get(offset..).unwrap_or("").trim();
            break;
        }
    }

    s.to_string()
}

/// Tries to match common natural language patterns to formula structures.
fn match_rules(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();

    // IF THEN ELSE patterns
    if lower.starts_with("if ")
        && let Some(formula) = parse_if_statement(s)
    {
        return Some(formula);
    }

    // LOOKUP / VLOOKUP patterns
    if (lower.starts_with("lookup ") || lower.starts_with("vlookup ") || lower.starts_with("find "))
        && let Some(formula) = parse_lookup(s)
    {
        return Some(formula);
    }

    // PERCENTAGE patterns
    if (lower.starts_with("percentage ") || lower.starts_with("percent ") || lower.starts_with("share of "))
        && let Some(formula) = parse_percentage(s)
    {
        return Some(formula);
    }

    // ROUND patterns
    if lower.starts_with("round ")
        && let Some(formula) = parse_round(s)
    {
        return Some(formula);
    }

    // Aggregations: SUM, AVERAGE, COUNT, MAX, MIN, MEDIAN, PRODUCT
    if let Some(formula) = parse_aggregation(s) {
        return Some(formula);
    }

    None
}

/// Parses aggregation requests: SUM, AVERAGE, COUNT, COUNTA, MAX, MIN, MEDIAN, PRODUCT.
fn parse_aggregation(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();

    // Sum patterns
    let sum_prefixes = ["sum of ", "sum ", "total of ", "total ", "add up ", "add ", "sum up "];
    for p in sum_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=SUM({target})"));
        }
    }

    // Average patterns
    let avg_prefixes = ["average of ", "average ", "mean of ", "mean ", "avg of ", "avg "];
    for p in avg_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=AVERAGE({target})"));
        }
    }

    // Count non-empty patterns
    let counta_prefixes =
        ["count non-empty in ", "count non-empty of ", "count non-blank in ", "count non empty in ", "count all in ", "count text in "];
    for p in counta_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=COUNTA({target})"));
        }
    }

    // Count numeric patterns
    let count_prefixes = ["count of ", "count ", "count numbers in ", "number of items in ", "number of rows in "];
    for p in count_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=COUNT({target})"));
        }
    }

    // Max patterns
    let max_prefixes = ["max of ", "max in ", "maximum of ", "maximum in ", "highest of ", "highest in ", "largest of ", "largest in "];
    for p in max_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=MAX({target})"));
        }
    }

    // Min patterns
    let min_prefixes = ["min of ", "min in ", "minimum of ", "minimum in ", "lowest of ", "lowest in ", "smallest of ", "smallest in "];
    for p in min_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=MIN({target})"));
        }
    }

    // Median patterns
    let median_prefixes = ["median of ", "median in ", "median "];
    for p in median_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=MEDIAN({target})"));
        }
    }

    // Product patterns
    let product_prefixes = ["product of ", "product in ", "multiply "];
    for p in product_prefixes {
        if lower.starts_with(p) {
            let rest = s.get(p.len()..)?.trim();
            let target = normalize_range(rest)?;
            return Some(format!("=PRODUCT({target})"));
        }
    }

    None
}

/// Parses IF conditional logic: "if X then Y else Z", "if X, Y, Z", "if X, then Y, else Z".
fn parse_if_statement(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    let body = s.strip_prefix("if ")?.trim();
    let body_lower = lower.strip_prefix("if ")?.trim();

    let (cond_part, rest) = if let Some(idx) = body_lower.find(" then ") {
        let cond = body.get(..idx)?.trim();
        let rest = body.get(idx + 6..)?.trim();
        (cond, rest)
    } else {
        let idx = body.find(',')?;
        let cond = body.get(..idx)?.trim();
        let rest = body.get(idx + 1..)?.trim();
        (cond, rest)
    };

    let rest_lower = rest.to_ascii_lowercase();
    let (then_part, else_part) = if let Some(idx) = rest_lower.find(" else ") {
        let then_val = rest.get(..idx)?.trim();
        let else_val = rest.get(idx + 6..)?.trim();
        (then_val, else_val)
    } else if let Some(idx) = rest.find(',') {
        let then_val = rest.get(..idx)?.trim();
        let else_val = rest.get(idx + 1..)?.trim();
        (then_val, else_val)
    } else {
        (rest, "\"\"")
    };

    let cond_normalized = normalize_condition(cond_part);
    let then_normalized = normalize_value(then_part);
    let else_normalized = normalize_value(else_part);

    Some(format!("=IF({cond_normalized}, {then_normalized}, {else_normalized})"))
}

/// Normalizes a condition in natural language into a formula comparison expression.
fn normalize_condition(cond: &str) -> String {
    let c = cond.trim();
    let lower = c.to_ascii_lowercase();

    let replacements = [
        (" is greater than or equal to ", ">="),
        (" greater than or equal to ", ">="),
        (" is less than or equal to ", "<="),
        (" less than or equal to ", "<="),
        (" is greater than ", ">"),
        (" greater than ", ">"),
        (" is less than ", "<"),
        (" less than ", "<"),
        (" is equal to ", "="),
        (" equal to ", "="),
        (" equals ", "="),
        (" is not equal to ", "<>"),
        (" not equal to ", "<>"),
        (" is not ", "<>"),
        (" is ", "="),
    ];

    for (phrase, sym) in replacements {
        if let Some(idx) = lower.find(phrase) {
            let left = c.get(..idx).unwrap_or("").trim();
            let right = c.get(idx + phrase.len()..).unwrap_or("").trim();
            let norm_left = normalize_expr(left);
            let norm_right = normalize_value(right);
            return format!("{norm_left}{sym}{norm_right}");
        }
    }

    normalize_expr(c)
}

/// Normalizes a cell or expression operand.
fn normalize_expr(s: &str) -> String {
    let t = s.trim();
    t.replace(' ', "")
}

/// Normalizes a branch value (number, boolean, cell reference, or string literal).
fn normalize_value(s: &str) -> String {
    let mut t = s.trim();
    // If wrapped in then/else leftovers
    t = t.strip_prefix("then ").unwrap_or(t).trim();
    t = t.strip_prefix("else ").unwrap_or(t).trim();

    // Empty string
    if t.is_empty() {
        return "\"\"".into();
    }

    // Number
    if t.parse::<f64>().is_ok() {
        return t.to_string();
    }

    // Boolean
    let lower = t.to_ascii_lowercase();
    if lower == "true" {
        return "TRUE".into();
    }
    if lower == "false" {
        return "FALSE".into();
    }

    // Already quoted
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        let inside = t.get(1..t.len().saturating_sub(1)).unwrap_or("");
        return format!("\"{inside}\"");
    }

    // Valid cell reference or range
    if CellRef::parse(t).is_some() || RangeRef::parse(t).is_some() {
        return t.to_ascii_uppercase();
    }

    // Arithmetic expression with cell references e.g. A1*2, B2+10
    if t.contains(['+', '-', '*', '/', '^']) {
        return t.replace(' ', "");
    }

    // Plain text literal -> quote it
    format!("\"{t}\"")
}

/// Parses VLOOKUP/LOOKUP commands:
/// "lookup A1 in B:C"
/// "lookup A1 in B:D column 3"
/// "find A1 in Sheet2!A:C"
fn parse_lookup(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();

    let prefix_len = if lower.starts_with("lookup ") {
        7
    } else if lower.starts_with("vlookup ") {
        8
    } else if lower.starts_with("find ") {
        5
    } else {
        return None;
    };

    let rest = s.get(prefix_len..)?.trim();
    let rest_lower = rest.to_ascii_lowercase();

    let (lookup_val, target_part) = {
        let idx = rest_lower.find(" in ")?;
        let val = rest.get(..idx)?.trim();
        let target = rest.get(idx + 4..)?.trim();
        (val, target)
    };

    // Check for explicit column index e.g. "column 3", "col 3", "return column 3"
    let target_lower = target_part.to_ascii_lowercase();
    let (table_part, col_idx) = if let Some(idx) = target_lower.find(" column ") {
        let tbl = target_part.get(..idx)?.trim();
        let col = target_part.get(idx + 8..)?.trim().parse::<u32>().unwrap_or(2);
        (tbl, col)
    } else if let Some(idx) = target_lower.find(" col ") {
        let tbl = target_part.get(..idx)?.trim();
        let col = target_part.get(idx + 5..)?.trim().parse::<u32>().unwrap_or(2);
        (tbl, col)
    } else {
        (target_part, 2)
    };

    let norm_val = normalize_lookup_key(lookup_val);
    let norm_table = normalize_range(table_part)?;

    Some(format!("=VLOOKUP({norm_val}, {norm_table}, {col_idx}, FALSE)"))
}

/// Normalizes the lookup search key (cell reference or string literal).
fn normalize_lookup_key(s: &str) -> String {
    let t = s.trim();
    if CellRef::parse(t).is_some() { t.to_ascii_uppercase() } else { normalize_value(t) }
}

/// Parses percentage calculations:
/// "percentage of A1 in A1:A10" -> `=(A1/SUM(A1:A10))*100`
/// "percentage of A1 over B1" -> `=(A1/B1)*100`
/// "percentage of A1" -> `=(A1/SUM(A:A))*100`
fn parse_percentage(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();

    let prefixes = ["percentage of ", "percentage ", "percent of ", "percent ", "share of "];
    let mut remainder = s;
    for p in prefixes {
        if lower.starts_with(p) {
            remainder = s.get(p.len()..)?.trim();
            break;
        }
    }

    let rem_lower = remainder.to_ascii_lowercase();
    let separators = [" in ", " of ", " over ", " against ", " divided by "];

    for sep in separators {
        if let Some(idx) = rem_lower.find(sep) {
            let num = remainder.get(..idx)?.trim();
            let denom = remainder.get(idx + sep.len()..)?.trim();

            let norm_num = normalize_expr(num);
            let norm_denom = normalize_range(denom)?;

            return if norm_denom.contains(':') {
                Some(format!("=({norm_num}/SUM({norm_denom}))*100"))
            } else {
                Some(format!("=({norm_num}/{norm_denom})*100"))
            };
        }
    }

    // Single item: e.g. "percentage of B2"
    let norm = normalize_expr(remainder);
    if let Some(cell) = CellRef::parse(&norm) {
        let col_letter = gridcraft_core::addr::col_to_letters(cell.col);
        Some(format!("=({norm}/SUM({col_letter}:{col_letter}))*100"))
    } else {
        None
    }
}

/// Parses round expressions: "round A1 to 2 decimals", "round A1".
fn parse_round(s: &str) -> Option<String> {
    let rest = s.strip_prefix("round ")?.trim();
    let lower = rest.to_ascii_lowercase();

    let (val_part, decimals) = if let Some(idx) = lower.find(" to ") {
        let val = rest.get(..idx)?.trim();
        let dec_part = rest.get(idx + 4..)?.trim();
        let num_str = dec_part.split_whitespace().next().unwrap_or("0").trim_end_matches(['d', 'e', 'c', 'i', 'm', 'a', 'l', 's', 'p', 'l', 'a']);
        let digits = num_str.parse::<i32>().unwrap_or(0);
        (val, digits)
    } else {
        (rest, 0)
    };

    let norm_val = normalize_expr(val_part);
    Some(format!("=ROUND({norm_val}, {decimals})"))
}

/// Normalizes range text into standard A1 range notation (e.g. "B:B", "A1:B10").
pub fn normalize_range(raw: &str) -> Option<String> {
    let s = raw.trim();
    let lower = s.to_ascii_lowercase();

    // Sheet qualified: e.g. "Sheet1!A1:B10"
    if let Some((sheet, body)) = s.split_once('!') {
        let norm_body = normalize_range(body)?;
        return Some(format!("{sheet}!{norm_body}"));
    }

    // Column patterns: "column B", "col B", "columns B:D", "column B to D"
    if lower.starts_with("column ") || lower.starts_with("col ") {
        let prefix_len = if lower.starts_with("column ") { 7 } else { 4 };
        let body = s.get(prefix_len..)?.trim();
        return parse_col_range(body);
    }
    if lower.starts_with("columns ") || lower.starts_with("cols ") {
        let prefix_len = if lower.starts_with("columns ") { 8 } else { 5 };
        let body = s.get(prefix_len..)?.trim();
        return parse_col_range(body);
    }

    // Row patterns: "row 5", "rows 2 to 10"
    if lower.starts_with("row ") {
        let row_str = s.get(4..)?.trim();
        if let Ok(n) = row_str.parse::<u32>() {
            return Some(format!("{n}:{n}"));
        }
    }
    if lower.starts_with("rows ") {
        let row_str = s.get(5..)?.trim();
        return parse_row_range(row_str);
    }

    // Range patterns: "A1 to A10", "A1 through A10", "A1..A10", "A1:A10"
    let range_connectors = [" to ", " through ", " until ", "..", ":", " - "];
    for conn in range_connectors {
        if let Some(idx) = lower.find(conn) {
            let start = s.get(..idx)?.trim();
            let end = s.get(idx + conn.len()..)?.trim();
            let norm_start = start.to_ascii_uppercase();
            let norm_end = end.to_ascii_uppercase();

            // Check if both are cells
            if CellRef::parse(&norm_start).is_some() && CellRef::parse(&norm_end).is_some() {
                return Some(format!("{norm_start}:{norm_end}"));
            }

            // Check if both are columns (letters)
            if letters_to_col(&norm_start).is_some() && letters_to_col(&norm_end).is_some() {
                return Some(format!("{norm_start}:{norm_end}"));
            }

            // Check if both are numbers (rows)
            if norm_start.parse::<u32>().is_ok() && norm_end.parse::<u32>().is_ok() {
                return Some(format!("{norm_start}:{norm_end}"));
            }
        }
    }

    // Check if directly a valid RangeRef
    let upper = s.to_ascii_uppercase();
    if RangeRef::parse(&upper).is_some() {
        return Some(upper);
    }

    // Check if directly a single column letter (e.g. "B" or "AA")
    if letters_to_col(&upper).is_some() {
        return Some(format!("{upper}:{upper}"));
    }

    // Check if directly a single cell
    if CellRef::parse(&upper).is_some() {
        return Some(upper);
    }

    None
}

/// Parses column specs like "B", "B:D", "B to D", "B through D".
fn parse_col_range(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    for conn in [" to ", " through ", ":", "..", " - "] {
        if let Some(idx) = lower.find(conn) {
            let a = s.get(..idx)?.trim().to_ascii_uppercase();
            let b = s.get(idx + conn.len()..)?.trim().to_ascii_uppercase();
            if letters_to_col(&a).is_some() && letters_to_col(&b).is_some() {
                return Some(format!("{a}:{b}"));
            }
        }
    }

    let upper = s.trim().to_ascii_uppercase();
    if letters_to_col(&upper).is_some() {
        return Some(format!("{upper}:{upper}"));
    }

    None
}

/// Parses row specs like "2 to 10", "2:10".
fn parse_row_range(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    for conn in [" to ", " through ", ":", "..", " - "] {
        if let Some(idx) = lower.find(conn) {
            let a = s.get(..idx)?.trim();
            let b = s.get(idx + conn.len()..)?.trim();
            if let (Ok(na), Ok(nb)) = (a.parse::<u32>(), b.parse::<u32>()) {
                return Some(format!("{na}:{nb}"));
            }
        }
    }
    None
}

/// Fallback heuristic for unhandled patterns.
fn fallback_heuristic(s: &str) -> Result<String, AiError> {
    // If user provided a formula directly without '='
    let upper = s.to_ascii_uppercase();
    if (upper.starts_with("SUM(")
        || upper.starts_with("AVERAGE(")
        || upper.starts_with("COUNT(")
        || upper.starts_with("IF(")
        || upper.starts_with("VLOOKUP("))
        && upper.ends_with(')')
    {
        return Ok(format!("={upper}"));
    }

    Err(AiError::Translation(format!("could not determine formula pattern for query: `{s}`")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_sum() {
        assert_eq!(translate("sum of column B").unwrap(), "=SUM(B:B)");
        assert_eq!(translate("sum of B").unwrap(), "=SUM(B:B)");
        assert_eq!(translate("sum of A1 to A10").unwrap(), "=SUM(A1:A10)");
        assert_eq!(translate("total of B2:B20").unwrap(), "=SUM(B2:B20)");
        assert_eq!(translate("calculate the total of column C").unwrap(), "=SUM(C:C)");
    }

    #[test]
    fn translates_average() {
        assert_eq!(translate("average of column C").unwrap(), "=AVERAGE(C:C)");
        assert_eq!(translate("average of C").unwrap(), "=AVERAGE(C:C)");
        assert_eq!(translate("mean of C1 to C100").unwrap(), "=AVERAGE(C1:C100)");
        assert_eq!(translate("avg of column D").unwrap(), "=AVERAGE(D:D)");
    }

    #[test]
    fn translates_count() {
        assert_eq!(translate("count of column A").unwrap(), "=COUNT(A:A)");
        assert_eq!(translate("count non-empty in A1 to A50").unwrap(), "=COUNTA(A1:A50)");
        assert_eq!(translate("count all in column B").unwrap(), "=COUNTA(B:B)");
    }

    #[test]
    fn translates_max_and_min() {
        assert_eq!(translate("max of column D").unwrap(), "=MAX(D:D)");
        assert_eq!(translate("maximum of D1:D50").unwrap(), "=MAX(D1:D50)");
        assert_eq!(translate("min of column D").unwrap(), "=MIN(D:D)");
        assert_eq!(translate("lowest in D1:D50").unwrap(), "=MIN(D1:D50)");
    }

    #[test]
    fn translates_if_conditions() {
        assert_eq!(translate("if A1 > 10 then High else Low").unwrap(), "=IF(A1>10, \"High\", \"Low\")");
        assert_eq!(translate("if A1 is greater than 10 then 1 else 0").unwrap(), "=IF(A1>10, 1, 0)");
        assert_eq!(translate("if B2 >= 50, Pass, Fail").unwrap(), "=IF(B2>=50, \"Pass\", \"Fail\")");
    }

    #[test]
    fn translates_vlookup() {
        assert_eq!(translate("lookup A1 in B:C").unwrap(), "=VLOOKUP(A1, B:C, 2, FALSE)");
        assert_eq!(translate("lookup A2 in D1:F100 column 3").unwrap(), "=VLOOKUP(A2, D1:F100, 3, FALSE)");
    }

    #[test]
    fn translates_percentage() {
        assert_eq!(translate("percentage of A1 in A1:A10").unwrap(), "=(A1/SUM(A1:A10))*100");
        assert_eq!(translate("percentage of A1 over A10").unwrap(), "=(A1/A10)*100");
        assert_eq!(translate("percentage of B2").unwrap(), "=(B2/SUM(B:B))*100");
    }

    #[test]
    fn translates_round() {
        assert_eq!(translate("round A1 to 2 decimals").unwrap(), "=ROUND(A1, 2)");
        assert_eq!(translate("round A1").unwrap(), "=ROUND(A1, 0)");
    }

    #[test]
    fn rejects_nonsense() {
        assert!(translate("unintelligible words without any data").is_err());
    }
}
