/// Maximum context characters to show around error position
const ERROR_CONTEXT_SIZE: usize = 30;

/// Helper function to extract context around an error position safely
pub fn extract_context(full_text: &str, pos: usize) -> String {
    // Find char boundary at or before start
    let start_byte = full_text
        .char_indices()
        .filter(|&(i, _)| i <= pos.saturating_sub(ERROR_CONTEXT_SIZE))
        .map(|(i, _)| i)
        .next_back()
        .unwrap_or(0);

    // Find char boundary at or after end
    let end_limit = pos + ERROR_CONTEXT_SIZE;
    let end_byte = full_text
        .char_indices()
        .filter(|&(i, _)| i <= end_limit)
        .map(|(i, c)| i + c.len_utf8())
        .next_back()
        .unwrap_or(full_text.len());

    let mut context = full_text[start_byte..end_byte].to_string();
    if start_byte > 0 {
        context.insert_str(0, "...");
    }
    if end_byte < full_text.len() {
        context.push_str("...");
    }
    context
}

/// Format an error with context and suggestions
pub fn format_parse_error(
    expression: &str,
    error_pos: Option<usize>,
    description: &str,
    suggestion: Option<&str>,
) -> String {
    let mut msg = format!("Parse error: {}", description);

    if let Some(pos) = error_pos {
        let context = extract_context(expression, pos);
        msg.push_str(&format!("\n  Near: \"{}\"", context));
    }

    if let Some(hint) = suggestion {
        msg.push_str(&format!("\n  Hint: {}", hint));
    }

    msg
}
