//! Streaming parser that watches generated JSON tokens as they arrive and
//! extracts complete sentences of `tutor_message.target_lang` the moment
//! they form, so TTS can be dispatched sentence-by-sentence without waiting
//! for the full response.
//!
//! We also do a final full JSON parse at end-of-generation to recover the
//! structured response (corrections, vocab, etc.) for the UI.

use serde::{Deserialize, Serialize};

/// Sentence-boundary punctuation for Spanish/English/French/etc. Includes
/// the inverted punctuation that starts Spanish exclamations, since those
/// open a new sentence-level unit. A newline also counts.
fn is_sentence_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '…')
}

/// Incremental parser for streamed LLM output. Feed it chunks of raw text;
/// call [`StreamingJsonParser::take_sentences`] to pull any complete
/// sentences of the tutor's target-language message that have become
/// available since the last call.
pub struct StreamingJsonParser {
    /// Everything emitted so far by the LLM.
    buffer: String,
    /// State of our tiny scanner.
    state: State,
    /// Decoded-and-unescaped text of `tutor_message.target_lang` captured so far.
    captured: String,
    /// How many characters of `captured` have already been dispatched as
    /// sentences. We only emit text past this cursor.
    emitted_len: usize,
    /// Length of `buffer` that has been scanned. Characters before this
    /// offset have already been consumed by the state machine.
    scan_pos: usize,
}

/// Where the scanner currently is relative to the JSON being emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Haven't located `"tutor_message"` yet.
    SearchingTutorMessage,
    /// Inside the `tutor_message` object, searching for `"target_lang"`.
    SearchingTargetLang,
    /// Located `"target_lang":` — waiting for the opening quote.
    AwaitingOpenQuote,
    /// Inside the string value. Accumulate characters into `captured`.
    InsideString,
    /// Last char was a backslash inside the string; the next char is escaped.
    InsideStringEscape,
    /// We've seen the closing quote. Done — no more sentences to emit.
    Finished,
}

impl StreamingJsonParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            state: State::SearchingTutorMessage,
            captured: String::new(),
            emitted_len: 0,
            scan_pos: 0,
        }
    }

    /// Feed a chunk of streamed text into the parser.
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        self.scan();
    }

    /// Pull any newly-complete sentences of `tutor_message.target_lang`.
    /// Each returned string is one sentence (stripped of leading/trailing
    /// whitespace), ready to be handed to TTS.
    pub fn take_sentences(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let remaining = &self.captured[self.emitted_len..];

        // Walk `remaining` and cut at sentence terminators.
        let mut last_cut = 0usize;
        let mut chars = remaining.char_indices().peekable();
        while let Some((i, c)) = chars.next() {
            if is_sentence_terminator(c) {
                // Include the terminator and any immediately following
                // punctuation (e.g. `?!`).
                let mut end = i + c.len_utf8();
                while let Some(&(_, nc)) = chars.peek() {
                    if is_sentence_terminator(nc) {
                        end += nc.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Require at least one whitespace/newline/end-of-buffer after
                // the terminator before we commit — otherwise "3.14" would be
                // cut. But if we're in `Finished` state, commit immediately.
                let after = &remaining[end..];
                let committed = after
                    .chars()
                    .next()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(self.state == State::Finished);
                if committed {
                    let sent = remaining[last_cut..end].trim();
                    if !sent.is_empty() {
                        out.push(sent.to_string());
                    }
                    // Advance past terminator AND the whitespace run.
                    let mut adv = end;
                    while adv < remaining.len() {
                        let c = remaining[adv..].chars().next().unwrap();
                        if c.is_whitespace() {
                            adv += c.len_utf8();
                        } else {
                            break;
                        }
                    }
                    last_cut = adv;
                }
            }
        }

        // If the string was closed, flush any remainder as a final sentence
        // even if there was no terminator.
        if self.state == State::Finished && last_cut < remaining.len() {
            let tail = remaining[last_cut..].trim();
            if !tail.is_empty() {
                out.push(tail.to_string());
            }
            last_cut = remaining.len();
        }

        self.emitted_len += last_cut;
        out
    }

    /// True if we've finished capturing the target_lang string.
    #[allow(dead_code)]
    pub fn is_finished(&self) -> bool {
        self.state == State::Finished
    }

    /// Return everything captured so far (unescaped) from target_lang.
    pub fn captured(&self) -> &str {
        &self.captured
    }

    fn scan(&mut self) {
        // Work on a byte-index basis; all the markers we look for are ASCII.
        let bytes = self.buffer.as_bytes();

        while self.scan_pos < bytes.len() {
            match self.state {
                State::SearchingTutorMessage => {
                    if let Some(idx) = find_from(bytes, self.scan_pos, b"\"tutor_message\"") {
                        self.scan_pos = idx + b"\"tutor_message\"".len();
                        self.state = State::SearchingTargetLang;
                    } else {
                        return; // wait for more input
                    }
                }
                State::SearchingTargetLang => {
                    if let Some(idx) = find_from(bytes, self.scan_pos, b"\"target_lang\"") {
                        self.scan_pos = idx + b"\"target_lang\"".len();
                        self.state = State::AwaitingOpenQuote;
                    } else {
                        return;
                    }
                }
                State::AwaitingOpenQuote => {
                    // Skip whitespace and the `:` separator until we hit `"`.
                    while self.scan_pos < bytes.len() {
                        let b = bytes[self.scan_pos];
                        self.scan_pos += 1;
                        if b == b'"' {
                            self.state = State::InsideString;
                            break;
                        }
                        // Anything other than whitespace/colon is unexpected;
                        // just skip and keep looking.
                    }
                    if self.state != State::InsideString {
                        return;
                    }
                }
                State::InsideString => {
                    // Walk characters (UTF-8 safe) until closing quote or escape.
                    let tail = &self.buffer[self.scan_pos..];
                    let mut advanced = 0usize;
                    let mut closed = false;
                    let mut escape = false;
                    for (i, c) in tail.char_indices() {
                        if c == '\\' {
                            escape = true;
                            advanced = i + 1;
                            break;
                        } else if c == '"' {
                            closed = true;
                            advanced = i + 1;
                            break;
                        } else {
                            self.captured.push(c);
                        }
                    }
                    if !closed && !escape {
                        // Consumed the whole tail as plain chars.
                        self.scan_pos = bytes.len();
                        return;
                    }
                    self.scan_pos += advanced;
                    if escape {
                        self.state = State::InsideStringEscape;
                    } else if closed {
                        self.state = State::Finished;
                        return;
                    }
                }
                State::InsideStringEscape => {
                    if self.scan_pos >= bytes.len() {
                        return;
                    }
                    let tail = &self.buffer[self.scan_pos..];
                    let first = tail.chars().next().unwrap();
                    let consumed;
                    match first {
                        'n' => {
                            self.captured.push('\n');
                            consumed = first.len_utf8();
                        }
                        't' => {
                            self.captured.push('\t');
                            consumed = first.len_utf8();
                        }
                        'r' => {
                            self.captured.push('\r');
                            consumed = first.len_utf8();
                        }
                        '"' => {
                            self.captured.push('"');
                            consumed = first.len_utf8();
                        }
                        '\\' => {
                            self.captured.push('\\');
                            consumed = first.len_utf8();
                        }
                        '/' => {
                            self.captured.push('/');
                            consumed = first.len_utf8();
                        }
                        'u' => {
                            // \uXXXX — need 4 more hex chars.
                            if tail.len() < 1 + 4 {
                                return; // wait for more input
                            }
                            let hex = &tail[1..5];
                            if let Ok(code) = u32::from_str_radix(hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    self.captured.push(ch);
                                }
                            }
                            consumed = 5;
                        }
                        other => {
                            // Unknown escape — keep the char verbatim.
                            self.captured.push(other);
                            consumed = first.len_utf8();
                        }
                    }
                    self.scan_pos += consumed;
                    self.state = State::InsideString;
                }
                State::Finished => return,
            }
        }
    }
}

/// Byte-level substring search starting from `from`.
fn find_from(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= haystack.len() || needle.is_empty() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

// ── Final (non-streaming) structured parse ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTutorResponse {
    pub tutor_message: TutorMessage,
    #[serde(default)]
    pub correction: Option<Correction>,
    #[serde(default)]
    pub new_vocabulary: Vec<NewVocabulary>,
    #[serde(default)]
    pub grammar_note: Option<GrammarNote>,
    #[serde(default)]
    pub suggested_responses: Vec<SuggestedResponse>,
    #[serde(default)]
    pub internal_notes: Option<InternalNotes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorMessage {
    pub target_lang: String,
    #[serde(default)]
    pub native_lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub original: String,
    pub corrected: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewVocabulary {
    pub target_text: String,
    pub native_text: String,
    #[serde(default)]
    pub pronunciation: Option<String>,
    #[serde(default)]
    pub part_of_speech: Option<String>,
    #[serde(default)]
    pub example_target: Option<String>,
    #[serde(default)]
    pub example_native: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrammarNote {
    pub title: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedResponse {
    pub target_lang: String,
    #[serde(default)]
    pub native_lang: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalNotes {
    #[serde(default)]
    pub estimated_comprehension: Option<String>,
    #[serde(default)]
    pub lesson_progress_pct: Option<i32>,
}

impl ParsedTutorResponse {
    /// Parse a full streamed response. Tolerant to leading/trailing garbage
    /// (e.g. a stray code fence) by extracting the first balanced `{...}`.
    pub fn from_streamed(raw: &str) -> Result<Self, String> {
        let json = extract_json_object(raw).ok_or_else(|| {
            format!("no JSON object found in response (len={})", raw.len())
        })?;
        serde_json::from_str(json).map_err(|e| format!("json parse failed: {e}"))
    }
}

/// Find the first top-level JSON object in `s` and return a substring slice.
/// Walks the string tracking brace depth while respecting string escapes.
fn extract_json_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut start: Option<usize> = None;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(st) = start {
                        return Some(&s[st..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_extracts_sentences_progressively() {
        let mut p = StreamingJsonParser::new();
        // First chunk: opens the string and contains a complete first sentence
        // "¡Hola!" (followed by a space, so it's committed).
        p.push(r#"{"tutor_message":{"target_lang":"¡Hola! Bienvenido"#);
        assert_eq!(p.take_sentences(), vec!["¡Hola!"]);

        // Second chunk: extends with " al café." + next sentence start
        p.push(r#" al café. ¿Qué te"#);
        assert_eq!(p.take_sentences(), vec!["Bienvenido al café."]);

        // Third chunk closes the string — final sentence flushes out.
        p.push(r#" gustaría ordenar?","native_lang":"Hello"}}"#);
        assert_eq!(p.take_sentences(), vec!["¿Qué te gustaría ordenar?"]);
        assert!(p.is_finished());
    }

    #[test]
    fn streaming_handles_escaped_quote() {
        let mut p = StreamingJsonParser::new();
        p.push(r#"{"tutor_message":{"target_lang":"He said \"hola\" to me. Great."}}"#);
        let _ = p.take_sentences();
        assert!(p.is_finished());
        assert_eq!(p.captured(), r#"He said "hola" to me. Great."#);
    }

    #[test]
    fn final_parse_extracts_structured_fields() {
        let raw = r#"Here you go:
        {
          "tutor_message": {"target_lang": "¡Hola!", "native_lang": "Hi!"},
          "correction": null,
          "new_vocabulary": [{"target_text": "hola", "native_text": "hi"}],
          "grammar_note": null,
          "suggested_responses": [],
          "internal_notes": {"estimated_comprehension": "high", "lesson_progress_pct": 10}
        }
        "#;
        let p = ParsedTutorResponse::from_streamed(raw).unwrap();
        assert_eq!(p.tutor_message.target_lang, "¡Hola!");
        assert_eq!(p.new_vocabulary.len(), 1);
        assert_eq!(p.new_vocabulary[0].target_text, "hola");
    }
}
