/// Conversation history compression — shared between Ollama and Claude backends.
///
/// Strategy:
/// - Text messages: keep newest MAX_RECENT_TEXT in full, compress older ones to <15 word briefs
/// - Tool rounds: keep newest MAX_TOOL_ROUNDS_KEPT in full, drop older rounds entirely
/// - A "tool round" = 1 assistant message (with tool_calls) + all its tool result messages

/// Maximum number of recent text messages to keep in full.
pub const MAX_RECENT_TEXT: usize = 6;

/// Maximum number of tool-calling rounds to keep in the current invocation's tool history.
pub const MAX_TOOL_ROUNDS_KEPT: usize = 3;

// ── Brief extraction ─────────────────────────────────────────────────────────

/// Extract the BRIEF value from a `---MEMORY---` block in LLM output.
/// Returns None if no valid BRIEF is found.
pub fn extract_brief(content: &str) -> Option<String> {
    let idx = content.find("---MEMORY---")?;
    let tail = &content[idx + 12..];
    for line in tail.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("BRIEF:") {
            let brief = rest.trim();
            if !brief.is_empty() {
                return Some(brief.to_string());
            }
        }
    }
    None
}

/// Strip the `---MEMORY---` block from content, returning only the main text.
pub fn strip_memory_block(content: &str) -> String {
    if let Some(idx) = content.find("---MEMORY---") {
        content[..idx].trim_end().to_string()
    } else {
        content.to_string()
    }
}

// ── Text history compression ─────────────────────────────────────────────────

/// Simplified message representation for compression analysis.
pub struct CompressMsg {
    pub role: String,
    pub content: String,
    pub brief: Option<String>,
}

/// Result of text history compression.
pub struct CompressedTextHistory {
    /// Summary block for older exchanges (to inject into system prompt).
    pub older_summary: Option<String>,
    /// Index from which to keep messages in full (0-based, inclusive).
    pub keep_from: usize,
}

/// Truncate text to approximately `max_words` words.
fn truncate_words(text: &str, max_words: usize) -> String {
    let clean = text.replace('\n', " ");
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.len() <= max_words {
        words.join(" ")
    } else {
        format!("{}…", words[..max_words].join(" "))
    }
}

/// Compress older text messages into a summary block.
///
/// Messages from index `keep_from` onward are kept in full.
/// Older assistant messages are summarized using their `brief` field.
/// If no brief exists (backward compat), falls back to word truncation.
pub fn compress_text_history(messages: &[CompressMsg]) -> CompressedTextHistory {
    if messages.len() <= MAX_RECENT_TEXT {
        return CompressedTextHistory { older_summary: None, keep_from: 0 };
    }

    let split = messages.len() - MAX_RECENT_TEXT;
    let older = &messages[..split];

    let mut summary = String::from("[EARLIER EXCHANGES]\nMessages older than the current window, compressed:\n");
    let mut i = 0;
    while i < older.len() {
        if older[i].role == "user" {
            let user_brief = truncate_words(&older[i].content, 20);
            i += 1;
            if i < older.len() && older[i].role == "assistant" {
                let asst_brief = match older[i].brief.as_deref() {
                    Some(b) if !b.is_empty() => b.to_string(),
                    _ => truncate_words(&strip_memory_block(&older[i].content), 15),
                };
                summary.push_str(&format!("- User: \"{}\" → You: \"{}\"\n", user_brief, asst_brief));
                i += 1;
            } else {
                summary.push_str(&format!("- User: \"{}\"\n", user_brief));
            }
        } else if older[i].role == "assistant" {
            let asst_brief = match older[i].brief.as_deref() {
                Some(b) if !b.is_empty() => b.to_string(),
                _ => truncate_words(&strip_memory_block(&older[i].content), 15),
            };
            summary.push_str(&format!("- You: \"{}\"\n", asst_brief));
            i += 1;
        } else {
            i += 1; // skip tool/system messages
        }
    }

    CompressedTextHistory {
        older_summary: Some(summary),
        keep_from: split,
    }
}

// ── Tool round trimming ─────────────────────────────────────────────────────

/// Trim tool history to keep only the newest MAX_TOOL_ROUNDS_KEPT rounds.
///
/// A "round" starts at each message where `is_round_start` returns true,
/// and includes all subsequent messages until the next round start.
///
/// Returns the start index from which to slice the history.
/// If total rounds <= MAX_TOOL_ROUNDS_KEPT, returns 0 (keep all).
pub fn trim_tool_start_index<T>(history: &[T], is_round_start: impl Fn(&T) -> bool) -> usize {
    let round_starts: Vec<usize> = history
        .iter()
        .enumerate()
        .filter(|(_, m)| is_round_start(m))
        .map(|(i, _)| i)
        .collect();

    if round_starts.len() <= MAX_TOOL_ROUNDS_KEPT {
        return 0;
    }

    let keep_from_round = round_starts.len() - MAX_TOOL_ROUNDS_KEPT;
    round_starts[keep_from_round]
}

// ── Memory instruction ───────────────────────────────────────────────────────

/// Build the MEMORY INSTRUCTION block for the system prompt.
///
/// - `include_importance`: true for character chat (generates IMPORTANCE score for
///   the character memory system), false for normal chat (brief only).
pub fn memory_instruction(include_importance: bool) -> String {
    if include_importance {
        "[MEMORY INSTRUCTION]\nAfter EVERY reply, always append this block on new lines:\n---MEMORY---\nBRIEF:<1-2 sentence first-person summary of THIS specific response — what you just said, not the whole conversation. Under 15 words>\nIMPORTANCE:<0-100>\nGuide: 0-30 trivial small talk, 31-60 notable exchange, 61-79 significant moment, 80+ permanent (e.g. user shared something major).".to_string()
    } else {
        "[MEMORY INSTRUCTION]\nAfter EVERY reply, always append this block on new lines:\n---MEMORY---\nBRIEF:<1-2 sentence summary of THIS specific response. Under 15 words>".to_string()
    }
}
