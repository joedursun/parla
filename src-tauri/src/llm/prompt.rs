//! Prompt assembly: parameterized system prompt builder and chat message
//! representation. Actual chat-template formatting (Gemma's `<start_of_turn>`
//! markers etc.) is handled by llama-server's `--jinja` flag using the
//! model's embedded Jinja template — we just send role/content pairs.

/// Who said a message. Matches the OpenAI chat roles llama-server accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
        }
    }
}

// ── System prompt builder ────────────────────────────────────────────────

/// A vocabulary item to introduce during the lesson.
pub struct VocabItem<'a> {
    pub target: &'a str,
    pub native: &'a str,
}

/// A grammar point to focus on during the lesson.
pub struct GrammarFocus<'a> {
    pub title: &'a str,
    pub explanation: &'a str,
}

/// Lesson-specific context. When `None`, the tutor runs in free conversation mode.
pub struct LessonContext<'a> {
    pub topic: &'a str,
    pub scenario: &'a str,
    pub objectives: &'a [&'a str],
    pub vocabulary: &'a [VocabItem<'a>],
    pub grammar: &'a [GrammarFocus<'a>],
}

/// Build the system prompt from the learner's profile and optional lesson context.
///
/// When `lesson` is `None`, the tutor enters free conversation mode.
/// Once the DB layer exists (Phase 3+), the caller will populate these
/// parameters from `learner_profile`, `lessons`, and aggregated progress queries.
pub fn build_system_prompt(
    target_language: &str,
    student_name: &str,
    native_language: &str,
    level: &str,
    goals: &[&str],
    lesson: Option<&LessonContext<'_>>,
) -> String {
    let mut prompt = String::with_capacity(4096);

    // ── Tutor identity ───────────────────────────────────────────────
    prompt.push_str(&format!(
        "You are a warm, patient, expert {target_language} tutor having a one-on-one conversation with your student. Your name is Duo.\n\n"
    ));

    // ── Student profile ──────────────────────────────────────────────
    prompt.push_str(&format!(
        "## Your Student\n\
         - Name: {student_name}\n\
         - Native language: {native_language}\n\
         - Current level: {level}\n\
         - Goals: {}\n\n",
        goals.join(", ")
    ));

    // ── Lesson or free conversation ──────────────────────────────────
    if let Some(lesson) = lesson {
        prompt.push_str(&format!(
            "## This Lesson\n\
             - Topic: {}\n\
             - Scenario: {}\n\
             - Objectives:\n",
            lesson.topic, lesson.scenario
        ));
        for (i, obj) in lesson.objectives.iter().enumerate() {
            prompt.push_str(&format!("  {}. {}\n", i + 1, obj));
        }
        prompt.push('\n');

        if !lesson.vocabulary.is_empty() {
            prompt.push_str("## Vocabulary to Introduce\nIntroduce these naturally during conversation (don't dump them all at once):\n");
            for v in lesson.vocabulary {
                prompt.push_str(&format!("- {} ({})\n", v.target, v.native));
            }
            prompt.push('\n');
        }

        if !lesson.grammar.is_empty() {
            prompt.push_str("## Grammar Focus\n");
            for g in lesson.grammar {
                prompt.push_str(&format!("- {}: {}\n", g.title, g.explanation));
            }
            prompt.push('\n');
        }
    } else {
        prompt.push_str(&format!(
            "## Mode: Free Conversation\n\
             This is an open-ended conversation. There is no specific lesson topic.\n\
             Follow the student's lead — talk about whatever they want.\n\
             Still teach naturally: introduce useful vocabulary, correct mistakes gently,\n\
             and note grammar patterns as they come up.\n\n"
        ));
    }

    // ── Language mix ratio (derived from level) ──────────────────────
    let (target_pct, native_pct) = match level.chars().next() {
        Some('A') => (70, 30),
        Some('B') => (85, 15),
        Some('C') => (95, 5),
        _ => (70, 30),
    };

    // ── Conversation rules ───────────────────────────────────────────
    prompt.push_str(&format!(
        "## Conversation Rules\n\n\
         1. STAY IN CHARACTER while teaching. Don't break the scene to lecture.\n\
         2. LANGUAGE MIX: Speak primarily in {target_language}. Keep explanations of new concepts in {native_language}. Aim for ~{target_pct}% {target_language}, {native_pct}% {native_language} at this level.\n\
         3. INTRODUCE VOCABULARY NATURALLY by using the target words in context. When you use a new word for the first time, briefly gloss it.\n\
         4. CORRECT GENTLY. When the student makes an error: acknowledge what they communicated, show the correct form, give a brief (1-sentence) explanation, and continue the conversation. Don't correct every small error.\n\
         5. SCAFFOLD appropriately. If the student seems stuck, offer a suggestion.\n\
         6. PACE YOURSELF. Introduce 2-3 new vocabulary items at a time, not all at once.\n\
         7. Keep tutor_message.target_lang short — 1 to 3 sentences per turn — so the student can respond.\n\n"
    ));

    // ── Response format (language-agnostic) ──────────────────────────
    prompt.push_str(RESPONSE_FORMAT);

    // ── Opening instruction ──────────────────────────────────────────
    if lesson.is_some() {
        prompt.push_str("\nStart by greeting the student warmly, set the scene briefly, and begin the lesson.");
    } else {
        prompt.push_str("\nStart by greeting the student warmly and asking what they'd like to talk about.");
    }

    prompt
}

/// The JSON response format specification — shared across all prompt variants.
const RESPONSE_FORMAT: &str = r#"## Response Format

You MUST respond with a single JSON object and nothing else. No markdown fences, no prose before or after. Exactly this schema:

{
  "tutor_message": {
    "target_lang": "Text in the target language the tutor says (this is what gets spoken aloud)",
    "native_lang": "Translation in the student's native language"
  },
  "correction": null,
  "new_vocabulary": [],
  "grammar_note": null,
  "suggested_responses": [
    {"target_lang": "...", "native_lang": "..."},
    {"target_lang": "...", "native_lang": "..."}
  ],
  "internal_notes": {
    "estimated_comprehension": "high",
    "lesson_progress_pct": 10
  }
}

When the student makes a mistake, populate "correction":
{
  "original": "what the student said wrong",
  "corrected": "the correct form",
  "explanation": "brief 1-sentence explanation"
}

When introducing a new word or phrase, add an entry to "new_vocabulary":
{
  "target_text": "the word or phrase",
  "native_text": "translation",
  "pronunciation": "IPA pronunciation",
  "part_of_speech": "noun/verb/etc",
  "example_target": "Example sentence in target language.",
  "example_native": "Translation of example."
}

When teaching a grammar point, set "grammar_note":
{
  "title": "Grammar concept name",
  "explanation": "Brief explanation of the rule or pattern."
}

"estimated_comprehension" must be one of: "low", "medium", "high".
"lesson_progress_pct" is an integer 0-100 estimating how much of the lesson is complete.
"#;
