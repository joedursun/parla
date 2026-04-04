# Duo -- Data Architecture & Information Flow

This document describes how data flows through Duo: how lesson plans are generated, how vocabulary is tracked, and how the tutor dynamically adapts during conversations.

## Stack

| Component       | Technology                                   |
|-----------------|----------------------------------------------|
| Language Model   | Gemma 4 26B (GGUF, Apple Silicon, llama.cpp) |
| Speech-to-Text  | Whisper.cpp (local GGUF model)               |
| Text-to-Speech  | Local TTS model (e.g. Kokoro or Piper)       |
| Data Storage     | SQLite (single file, all structured data)    |
| Embeddings       | Not needed for v1 (see appendix)             |

## SQLite Schema

### Core Tables

```sql
-- Single row per learner (multi-learner = multiple rows, but v1 is single-user)
CREATE TABLE learner_profile (
  id              INTEGER PRIMARY KEY DEFAULT 1,
  native_language TEXT NOT NULL,          -- 'en'
  target_language TEXT NOT NULL,          -- 'es'
  cefr_level      TEXT NOT NULL,          -- 'A1', 'A2', 'B1', 'B2'
  goals           TEXT NOT NULL,          -- JSON array: ["travel", "conversation"]
  daily_goal_min  INTEGER DEFAULT 15,     -- target minutes per day
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- The learning path: a sequence of lessons
CREATE TABLE lessons (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  sequence_order    INTEGER NOT NULL,
  cefr_level        TEXT NOT NULL,        -- which CEFR band this lesson targets
  topic             TEXT NOT NULL,         -- 'food_and_dining'
  title             TEXT NOT NULL,         -- 'Ordering Food & Drinks'
  description       TEXT,                  -- 'Practice ordering at a restaurant...'
  scenario          TEXT,                  -- 'You are at a cafe in Madrid...'
  objectives        TEXT NOT NULL,         -- JSON: ["use polite ordering forms", "ask about prices"]
  target_vocabulary TEXT NOT NULL,         -- JSON: ["la cuenta", "me gustaria", "cuanto cuesta"]
  target_grammar    TEXT NOT NULL,         -- JSON: ["costar_conjugation", "polite_conditional"]
  status            TEXT DEFAULT 'planned',-- planned | in_progress | completed
  success_rate      REAL,                  -- 0.0-1.0, computed at completion
  started_at        TEXT,
  completed_at      TEXT,
  created_at        TEXT NOT NULL
);

-- Each conversation session (tied to a lesson, or free-form)
CREATE TABLE conversations (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  lesson_id      INTEGER,                 -- NULL for free conversation
  mode           TEXT NOT NULL,            -- 'lesson' | 'free' | 'listening_practice'
  topic          TEXT,                     -- human-readable topic
  summary        TEXT,                     -- LLM-generated post-conversation summary
  vocab_introduced TEXT,                   -- JSON: vocabulary IDs introduced this session
  grammar_practiced TEXT,                  -- JSON: grammar concept IDs practiced
  error_count    INTEGER DEFAULT 0,
  message_count  INTEGER DEFAULT 0,
  started_at     TEXT NOT NULL,
  ended_at       TEXT,
  created_at     TEXT NOT NULL
);

-- Individual messages within a conversation
CREATE TABLE messages (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  conversation_id INTEGER NOT NULL REFERENCES conversations(id),
  role            TEXT NOT NULL,           -- 'tutor' | 'student' | 'system'
  content         TEXT NOT NULL,           -- the target-language text
  translation     TEXT,                    -- native-language translation
  audio_path      TEXT,                    -- path to audio file if voice was used
  input_method    TEXT DEFAULT 'text',     -- 'text' | 'voice'
  created_at      TEXT NOT NULL
);

-- Corrections the tutor made to student messages
CREATE TABLE corrections (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  message_id        INTEGER NOT NULL REFERENCES messages(id),
  conversation_id   INTEGER NOT NULL REFERENCES conversations(id),
  original_text     TEXT NOT NULL,         -- what the student said
  corrected_text    TEXT NOT NULL,         -- the correct form
  explanation       TEXT NOT NULL,         -- why it's wrong
  error_type        TEXT NOT NULL,         -- 'grammar' | 'vocabulary' | 'pronunciation' | 'usage'
  grammar_concept   TEXT,                  -- links to grammar_concepts.slug if applicable
  created_at        TEXT NOT NULL
);

-- Every vocabulary item the learner has encountered
CREATE TABLE vocabulary (
  id                      INTEGER PRIMARY KEY AUTOINCREMENT,
  target_text             TEXT NOT NULL,     -- 'la cuenta'
  native_text             TEXT NOT NULL,     -- 'the bill'
  pronunciation           TEXT,              -- '/la ˈkwen.ta/'
  part_of_speech          TEXT,              -- 'noun', 'verb', 'phrase'
  gender                  TEXT,              -- 'f', 'm', or NULL
  topic                   TEXT NOT NULL,     -- 'food_and_dining'
  example_sentence_target TEXT,              -- from the conversation where first encountered
  example_sentence_native TEXT,
  first_seen_lesson_id    INTEGER REFERENCES lessons(id),
  first_seen_conversation_id INTEGER REFERENCES conversations(id),
  audio_path              TEXT,              -- path to pronunciation audio
  created_at              TEXT NOT NULL,
  UNIQUE(target_text, topic)                -- prevent exact duplicates per topic
);

-- SRS state for each vocabulary item (one row per card)
CREATE TABLE flashcards (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  vocabulary_id   INTEGER NOT NULL REFERENCES vocabulary(id) UNIQUE,
  card_type       TEXT DEFAULT 'vocabulary', -- 'vocabulary' | 'grammar' | 'phrase'
  status          TEXT DEFAULT 'new',        -- 'new' | 'learning' | 'review' | 'mature'
  ease_factor     REAL DEFAULT 2.5,          -- FSRS/SM-2 ease factor
  interval_days   REAL DEFAULT 0,            -- current interval
  due_date        TEXT NOT NULL,              -- when this card is next due
  review_count    INTEGER DEFAULT 0,
  lapse_count     INTEGER DEFAULT 0,         -- times it went back to "again"
  last_rating     TEXT,                       -- 'again' | 'hard' | 'good' | 'easy'
  last_reviewed   TEXT,
  created_at      TEXT NOT NULL
);

-- Log of every flashcard review (for analytics)
CREATE TABLE flashcard_reviews (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  flashcard_id    INTEGER NOT NULL REFERENCES flashcards(id),
  rating          TEXT NOT NULL,             -- 'again' | 'hard' | 'good' | 'easy'
  response_time_ms INTEGER,                  -- how long they took to answer
  reviewed_at     TEXT NOT NULL
);

-- Grammar concepts the learner is progressing through
CREATE TABLE grammar_concepts (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  slug            TEXT NOT NULL UNIQUE,      -- 'present_tense_regular'
  name            TEXT NOT NULL,             -- 'Present Tense Conjugation'
  description     TEXT,                      -- 'Regular -ar, -er, -ir verbs'
  examples        TEXT,                      -- JSON with example sentences
  cefr_level      TEXT NOT NULL,             -- 'A1'
  status          TEXT DEFAULT 'not_started',-- 'not_started' | 'learning' | 'mastered'
  accuracy_rate   REAL,                      -- rolling accuracy from corrections + reviews
  times_practiced INTEGER DEFAULT 0,
  times_correct   INTEGER DEFAULT 0,
  first_introduced_lesson_id INTEGER REFERENCES lessons(id),
  created_at      TEXT NOT NULL
);

-- Daily stats snapshots for the progress view and streaks
CREATE TABLE daily_stats (
  date                TEXT PRIMARY KEY,      -- '2026-04-04'
  practice_time_min   INTEGER DEFAULT 0,
  conversations_count INTEGER DEFAULT 0,
  messages_sent       INTEGER DEFAULT 0,
  new_vocab_count     INTEGER DEFAULT 0,
  flashcards_reviewed INTEGER DEFAULT 0,
  flashcard_accuracy  REAL,                  -- percent correct (good+easy / total)
  corrections_count   INTEGER DEFAULT 0
);

-- Weak areas surfaced from analysis (recomputed periodically)
CREATE TABLE weak_areas (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  concept        TEXT NOT NULL,              -- 'Verb conjugation (present)'
  concept_type   TEXT NOT NULL,              -- 'grammar' | 'vocabulary' | 'pronunciation'
  accuracy_rate  REAL NOT NULL,              -- 0.0-1.0
  sample_errors  TEXT,                       -- JSON: recent example errors
  suggestion     TEXT,                       -- 'Practice -ar verb endings in conversation'
  last_assessed  TEXT NOT NULL,
  resolved       INTEGER DEFAULT 0           -- 1 when accuracy improves above threshold
);
```

---

## Information Flow

### 1. Initial Setup (Onboarding)

When the user completes onboarding (selects language, level, goals):

1. Insert `learner_profile` row
2. Seed `grammar_concepts` with the standard progression for the target language + CEFR level
3. Call the LLM to generate the initial learning path (first 8-10 lessons)
4. Insert `lessons` rows

**Prompt: Generate Initial Learning Path**

```
<system>
You are a curriculum designer for language learning. Generate a structured
learning path for a student.

Output a JSON array of lesson objects. Each lesson should build logically
on the previous ones. Prioritize the student's goals.
</system>

<user>
Student profile:
- Native language: English
- Target language: Spanish
- Current level: A1 (Complete Beginner)
- Goals: Travel, Conversation
- Daily practice target: 15 minutes

Generate the first 10 lessons for this student. Each lesson should include:
- topic: a slug identifier (e.g., "greetings_introductions")
- title: display title
- description: one sentence
- scenario: a realistic situation for the conversation
- objectives: 3-5 specific learning objectives
- target_vocabulary: 8-12 key words/phrases to introduce
- target_grammar: 1-3 grammar concepts to practice
- cefr_level: the CEFR sub-level this targets

Prioritize travel and conversation scenarios since those are the student's
goals. Order from simplest to most complex. Each lesson should reuse some
vocabulary from prior lessons while introducing new material.

Respond with only the JSON array, no other text.
</user>
```

### 2. Lesson Plan Generation (Before Each Lesson)

Before starting a new lesson, we check if the next planned lesson still makes sense given learner progress, and generate the detailed conversation setup.

**When this runs:** When the user taps "Continue Lesson" or starts the next lesson in the path.

**Data gathered from SQLite:**

```sql
-- Learner context
SELECT * FROM learner_profile WHERE id = 1;

-- Last 5 completed lessons with success rates
SELECT id, title, topic, success_rate, target_grammar, target_vocabulary
FROM lessons WHERE status = 'completed'
ORDER BY completed_at DESC LIMIT 5;

-- Vocabulary mastery summary
SELECT topic,
  COUNT(*) as total,
  SUM(CASE WHEN f.status = 'mature' THEN 1 ELSE 0 END) as mastered,
  SUM(CASE WHEN f.status = 'learning' THEN 1 ELSE 0 END) as learning,
  SUM(CASE WHEN f.status = 'new' THEN 1 ELSE 0 END) as new_count
FROM vocabulary v
JOIN flashcards f ON f.vocabulary_id = v.id
GROUP BY topic;

-- Grammar concepts status
SELECT slug, name, status, accuracy_rate
FROM grammar_concepts WHERE cefr_level = 'A1'
ORDER BY CASE status
  WHEN 'mastered' THEN 0
  WHEN 'learning' THEN 1
  WHEN 'not_started' THEN 2
END;

-- Current weak areas
SELECT concept, concept_type, accuracy_rate, suggestion
FROM weak_areas WHERE resolved = 0
ORDER BY accuracy_rate ASC;

-- Recent errors (last 20)
SELECT c.original_text, c.corrected_text, c.error_type, c.grammar_concept
FROM corrections c
ORDER BY c.created_at DESC LIMIT 20;
```

**Decision: Adapt or proceed?**

If weak areas exist with accuracy < 40%, the system may decide to insert a review lesson targeting those areas before proceeding to new material. This decision is made by the LLM:

```
<system>
You are a language tutoring system deciding what to teach next.
Given the student's progress data, decide whether to:
A) Proceed with the planned next lesson
B) Insert a review lesson targeting weak areas before moving on

Respond with JSON: {"decision": "proceed" | "review", "reason": "...", "adjustments": [...]}
If "review", include which weak areas to target.
If "proceed", include any adjustments to the planned lesson (e.g., "reinforce
gender agreement since accuracy is low").
</system>
```

### 3. The Conversation Loop (Core Teaching Flow)

This is where the tutor teaches. Each conversation turn follows this pipeline:

```
Student speaks/types
        |
        v
  [STT if voice] -- Whisper transcribes audio to text
        |
        v
  [LLM: Analyze + Respond] -- Single call that does everything
        |
        v
  [Parse structured response]
        |
        +---> Store message in DB
        +---> Store any corrections
        +---> Upsert new vocabulary
        +---> Update grammar concept stats
        +---> Update context panel (vocab, grammar notes, suggestions)
        |
        v
  [TTS] -- Generate audio for tutor response
        |
        v
  Display to student
```

**The Conversation System Prompt**

This is the most critical prompt in the system. It's assembled fresh for each conversation from the DB, and stays in context for the full conversation.

```
<system>
You are a warm, patient, expert Spanish tutor having a one-on-one
conversation with your student. Your name is Duo.

## Your Student
- Name: Joe
- Native language: English
- Current level: A1 (Beginner)
- Goals: Travel, Conversation
- They've completed 3 lessons so far

## This Lesson
- Topic: Ordering Food & Drinks
- Scenario: You and the student are at a cafe in Madrid. You play the
  waiter (el mesero). The student practices ordering.
- Objectives:
  1. Use "me gustaria" for polite ordering
  2. Ask about prices with "cuanto cuesta/cuestan"
  3. Request the bill with "la cuenta, por favor"
  4. Name common food and drink items
  5. Use numbers for prices

## Vocabulary to Introduce
Introduce these naturally during conversation (don't dump them all at once):
- me gustaria (I would like)
- la cuenta (the bill)
- cuanto cuesta (how much does it cost)
- el plato (the dish)
- la bebida (the drink)
- el postre (dessert)
- la propina (the tip)
- algo mas (anything else)

## Grammar Focus
- "costar" conjugation: cuesta (singular) vs cuestan (plural)
- Polite conditional: "me gustaria" vs "quiero"

## Known Weak Areas
- Verb conjugation (present): 42% accuracy -- gently reinforce when natural
- Gender agreement: 51% accuracy -- note when student gets it right or wrong

## Already Mastered Vocabulary (can use freely, don't re-teach)
- hola, buenos dias, buenas tardes
- por favor, gracias
- si, no
- uno, dos, tres, cuatro, cinco
- donde esta, a la izquierda, a la derecha

## Conversation Rules

1. STAY IN CHARACTER as the waiter while teaching. Don't break the scene
   to lecture.

2. LANGUAGE MIX: Speak primarily in Spanish. Keep explanations of new
   concepts in English. Aim for ~70% Spanish, 30% English at this level.

3. INTRODUCE VOCABULARY NATURALLY by using the target words in context.
   When you use a new word for the first time, briefly gloss it.
   Example: "Tenemos churros, tostadas, y tortilla espanola -- that's
   a Spanish omelette made with potato and egg."

4. CORRECT GENTLY. When the student makes an error:
   - Acknowledge what they communicated successfully
   - Show the correct form
   - Give a brief (1-sentence) explanation
   - Continue the conversation naturally
   Don't correct every small error -- prioritize errors related to
   lesson objectives and known weak areas.

5. SCAFFOLD appropriately. If the student seems stuck (short responses,
   mixing in English, long pauses), offer a suggestion in the form of
   "You could say: [Spanish] -- [English translation]".

6. TRACK PROGRESS mentally. Note which target vocabulary and grammar
   the student has successfully used unprompted. When all key objectives
   have been practiced, begin wrapping up the scenario naturally.

7. PACE YOURSELF. Introduce 2-3 new vocabulary items at a time, not all
   at once. Wait until the student has used or acknowledged them before
   introducing more.

## Response Format

Respond with JSON (the app will parse and render this):

{
  "tutor_message": {
    "target_lang": "Spanish text the tutor says",
    "native_lang": "English translation"
  },
  "correction": null | {
    "original": "what student said wrong",
    "corrected": "the correct form",
    "explanation": "brief explanation why"
  },
  "new_vocabulary": [] | [
    {
      "target_text": "tortilla espanola",
      "native_text": "Spanish omelette",
      "pronunciation": "/tor.ˈti.ʎa es.pa.ˈɲo.la/",
      "part_of_speech": "noun",
      "gender": "f",
      "example_target": "Tenemos tortilla espanola hoy.",
      "example_native": "We have Spanish omelette today."
    }
  ],
  "grammar_note": null | {
    "concept": "costar_conjugation",
    "title": "Costar (to cost)",
    "explanation": "Use cuesta for singular, cuestan for plural."
  },
  "suggested_responses": [
    {"target_lang": "No, eso es todo.", "native_lang": "No, that's all."},
    {"target_lang": "Tienen algun postre?", "native_lang": "Do you have desserts?"}
  ],
  "internal_notes": {
    "vocabulary_used_correctly": ["me gustaria", "por favor"],
    "vocabulary_attempted_incorrectly": [],
    "grammar_demonstrated": ["polite_conditional"],
    "grammar_errors": ["costar_conjugation"],
    "estimated_comprehension": "high",
    "lesson_progress_pct": 60
  }
}
</system>
```

**Why a single structured call instead of separate analysis + response?**

- Fewer LLM calls = lower latency (critical for conversational feel)
- The analysis informs the response naturally (e.g., the tutor corrects because it detected an error)
- Gemma 26B can handle structured JSON output reliably
- The `internal_notes` field gives us machine-readable signals without a second call

**Processing the Response**

After each LLM response, the app parses the JSON and:

1. **Displays** `tutor_message` as a chat bubble, `correction` as an inline correction card, `new_vocabulary` as vocab cards, `grammar_note` in the context panel, `suggested_responses` as clickable chips.

2. **Stores** the tutor message and student message in `messages`. Stores any `correction` in `corrections`. Upserts `new_vocabulary` items into `vocabulary` and creates `flashcards` rows.

3. **Updates state** from `internal_notes`:
   - Increment `times_practiced` / `times_correct` on relevant `grammar_concepts`
   - Update the conversation's running `error_count`
   - Track lesson progress percentage

### 4. Post-Conversation Processing

When the user ends a conversation (or it reaches a natural conclusion):

**Trigger:** User clicks "End Lesson" or the tutor signals lesson is complete (progress reaches 100%).

**Steps:**

1. **Summarize** -- Call the LLM to generate a conversation summary:

```
<system>
Summarize this language lesson conversation. Include:
- What vocabulary was introduced and whether the student used it
- What grammar was practiced
- Key errors the student made
- Overall assessment of how the lesson went
- Suggested focus areas for next time

Output JSON:
{
  "summary": "...",
  "vocabulary_mastery": {"me gustaria": "demonstrated", "cuanto cuesta": "needs_practice"},
  "grammar_mastery": {"costar_conjugation": "introduced_with_errors", "polite_conditional": "demonstrated"},
  "success_rate": 0.75,
  "suggested_next_focus": ["costar conjugation", "plural price questions"]
}
</system>

<user>
[Full conversation transcript inserted here]
</user>
```

2. **Update lesson record:**
   ```sql
   UPDATE lessons
   SET status = 'completed', success_rate = 0.75, completed_at = '...'
   WHERE id = ?;
   ```

3. **Update grammar concepts:**
   ```sql
   UPDATE grammar_concepts
   SET times_practiced = times_practiced + 1,
       times_correct = CASE WHEN ? THEN times_correct + 1 ELSE times_correct END,
       accuracy_rate = CAST(times_correct AS REAL) / times_practiced,
       status = CASE
         WHEN accuracy_rate > 0.85 AND times_practiced >= 3 THEN 'mastered'
         WHEN times_practiced > 0 THEN 'learning'
         ELSE status
       END
   WHERE slug = ?;
   ```

4. **Recompute weak areas:**
   ```sql
   -- Find grammar concepts with low accuracy
   INSERT OR REPLACE INTO weak_areas (concept, concept_type, accuracy_rate, last_assessed)
   SELECT name, 'grammar', accuracy_rate, datetime('now')
   FROM grammar_concepts
   WHERE status = 'learning' AND accuracy_rate < 0.6 AND times_practiced >= 2;

   -- Mark resolved areas
   UPDATE weak_areas SET resolved = 1
   WHERE accuracy_rate > 0.7;
   ```

5. **Update daily stats:**
   ```sql
   INSERT INTO daily_stats (date, practice_time_min, conversations_count, ...)
   VALUES (date('now'), ?, 1, ...)
   ON CONFLICT(date) DO UPDATE SET
     practice_time_min = practice_time_min + excluded.practice_time_min,
     conversations_count = conversations_count + 1;
   ```

6. **Plan next lesson** (if current lesson completed and next is still "planned"):
   Run the lesson adaptation check from section 2.

### 5. Flashcard Review Flow

The SRS implementation uses a simplified FSRS algorithm:

**Rating -> New Interval:**

| Rating | Action |
|--------|--------|
| Again  | Reset interval to 1 minute. Increment `lapse_count`. Set status = 'learning'. |
| Hard   | Multiply interval by 1.2. Decrease ease by 0.15 (min 1.3). |
| Good   | Multiply interval by ease_factor. |
| Easy   | Multiply interval by ease_factor * 1.3. Increase ease by 0.15. |

**Status Transitions:**
- `new` -> `learning` (after first review)
- `learning` -> `review` (after interval > 1 day)
- `review` -> `mature` (after interval > 21 days)
- Any -> `learning` (on "Again" rating, a lapse)

**Fetching due cards:**
```sql
SELECT f.*, v.target_text, v.native_text, v.pronunciation,
       v.example_sentence_target, v.example_sentence_native, v.audio_path,
       l.title as lesson_title
FROM flashcards f
JOIN vocabulary v ON v.id = f.vocabulary_id
LEFT JOIN lessons l ON l.id = v.first_seen_lesson_id
WHERE f.due_date <= datetime('now')
ORDER BY
  CASE f.status
    WHEN 'learning' THEN 0  -- learning cards first (short intervals)
    WHEN 'new' THEN 1
    WHEN 'review' THEN 2
  END,
  f.due_date ASC
LIMIT 20;
```

### 6. Dynamic Adaptation During Conversation

This is what makes Duo feel like a real tutor rather than a chatbot. The key mechanisms:

#### a) Real-Time Error Tracking

Every correction gets stored with its `error_type` and `grammar_concept`. During the conversation, the system prompt includes the running context. But the real power is **across conversations**: before generating a lesson plan, we query error patterns:

```sql
-- What does the student struggle with most?
SELECT grammar_concept, COUNT(*) as error_count,
       GROUP_CONCAT(original_text, ' | ') as examples
FROM corrections
WHERE created_at > datetime('now', '-14 days')
  AND grammar_concept IS NOT NULL
GROUP BY grammar_concept
ORDER BY error_count DESC
LIMIT 5;
```

This feeds into the lesson system prompt so the tutor knows to pay extra attention to certain areas.

#### b) Conversation-Level Adaptation

The LLM's `internal_notes.estimated_comprehension` field tracks how well the student is following. The app uses this to adjust mid-conversation:

- **High comprehension:** Reduce scaffolding. Stop showing suggested responses. Increase target-language ratio.
- **Low comprehension:** Offer more suggestions. Increase English explanations. Slow down vocabulary introduction.

This is implemented by injecting a mid-conversation guidance message to the LLM context:

```
<system_update>
The student appears to be struggling. For the next few turns:
- Offer 3 suggested responses instead of 2
- Keep sentences shorter
- Repeat key vocabulary more frequently
- Increase English in your explanations to ~50%
</system_update>
```

Or for high comprehension:

```
<system_update>
The student is doing very well. Adjust:
- Stop offering suggested responses
- Introduce vocabulary slightly faster
- Use more complex sentence structures
- Reduce English to ~15% of your speech
</system_update>
```

#### c) Cross-Session Memory

Before each conversation, the system prompt includes a "learner model" section built from aggregated DB data. This gives the LLM a picture of the student that evolves over time:

```
## Learner Model (auto-generated from data)
- Overall accuracy: 78%
- Strongest areas: Greetings (92%), Numbers (85%)
- Weakest areas: Verb conjugation (42%), Gender agreement (51%)
- Learning style signals: Uses voice input 60% of the time. Average
  response length is short (4-6 words). Tends to use "quiero" instead
  of "me gustaria" -- may need more polite-form reinforcement.
- Vocabulary retention: 98 mastered, 54 learning, 35 new.
  Flashcard lapse rate on food vocabulary is 30% (above average).
- Recent progress: Completed "Asking for Directions" with 85% success.
  Struggled with "cuanto cuesta" vs "cuanto es" in last session.
```

This is assembled purely from SQL queries -- no embeddings needed.

### 7. Progress Computation

The progress view pulls from multiple tables:

**CEFR level progress:**
```sql
-- Percentage of lessons completed at current CEFR level
SELECT
  COUNT(CASE WHEN status = 'completed' THEN 1 END) * 100.0 /
  COUNT(*) as pct_complete
FROM lessons
WHERE cefr_level = (SELECT cefr_level FROM learner_profile WHERE id = 1);
```

**Skill breakdown** (reading/listening/writing/speaking):
These are estimated from activity types:
- Reading: Accuracy on text-based flashcard reviews + corrections in text conversations
- Listening: Usage of audio playback + voice-input comprehension
- Writing: Text input accuracy in conversations
- Speaking: Voice input frequency + pronunciation corrections

```sql
-- Speaking score: ratio of voice messages without pronunciation corrections
SELECT
  COUNT(CASE WHEN m.input_method = 'voice' AND c.id IS NULL THEN 1 END) * 100.0 /
  NULLIF(COUNT(CASE WHEN m.input_method = 'voice' THEN 1 END), 0) as speaking_accuracy
FROM messages m
LEFT JOIN corrections c ON c.message_id = m.id AND c.error_type = 'pronunciation'
WHERE m.role = 'student'
  AND m.created_at > datetime('now', '-30 days');
```

**Streak calculation:**
```sql
-- Count consecutive days with practice, going backward from today
WITH RECURSIVE dates AS (
  SELECT date('now') as d, 1 as streak
  UNION ALL
  SELECT date(d, '-1 day'), streak + 1
  FROM dates
  WHERE EXISTS (
    SELECT 1 FROM daily_stats
    WHERE date = date(d, '-1 day') AND practice_time_min > 0
  )
)
SELECT MAX(streak) FROM dates;
```

---

## Free Conversation Mode

When the user starts a "Free Conversation" (no lesson), the system prompt is adapted:

- No specific vocabulary targets or lesson objectives
- The tutor follows the student's lead on topic
- Still tracks errors, introduces vocabulary organically, and stores everything
- Post-conversation processing creates flashcards from any new vocabulary encountered

```
<system>
You are a friendly Spanish conversation partner. The student wants to
practice speaking freely -- there is no specific lesson plan.

## Your approach:
- Let the student choose the topic. Ask what they'd like to talk about.
- Speak at a level appropriate for A1. Use simple sentences.
- When you naturally use a word the student might not know, briefly
  explain it.
- Correct errors gently, focusing on the most impactful ones.
- Aim for a natural, enjoyable conversation.

## Student's current level and context:
[...same learner model as structured lessons...]

Use the same JSON response format as structured lessons.
</system>
```

---

## Audio Pipeline

### Voice Input (Student -> App)
1. User holds mic button, audio captured as WAV/PCM
2. Whisper.cpp transcribes to text (target language)
3. Transcription displayed in chat bubble
4. Text sent to LLM as the student's message (same as typed input from here)

### Voice Output (App -> Student)
1. LLM response `tutor_message.target_lang` sent to TTS model
2. TTS generates audio (WAV)
3. Audio plays automatically; text displays simultaneously
4. Audio saved to `messages.audio_path` for replay

### Pronunciation Feedback
For v1, pronunciation assessment comes indirectly through the STT model: if Whisper transcribes the student's speech as something different from what they intended, that's a signal of pronunciation issues. The tutor can pick up on this:

```
Student says "la quenta" (mispronounced)
Whisper transcribes: "la quenta"
Tutor sees "la quenta" and corrects to "la cuenta", noting pronunciation
```

---

## Appendix: Why Not Embeddings for V1

Embeddings would enable semantic search across conversations (e.g., "find lessons where the student talked about food"). But for v1, this isn't needed because:

1. **Lesson planning** uses structured data (success rates, error counts, grammar concept status) -- more reliable than semantic similarity for curriculum decisions.
2. **Vocabulary lookup** is exact-match against the `vocabulary` table with topic categorization.
3. **Cross-conversation context** is handled by the aggregated learner model built from SQL queries.
4. **Free conversation** doesn't need to find "similar past conversations" -- the tutor just needs the learner model.

If we later want features like "find all conversations where the student practiced subjunctive mood" or "recommend lessons similar to ones the student enjoyed," embeddings (via sqlite-vec or Qdrant) would help. But structured queries cover all v1 use cases.

## Appendix: Model Context Window Budget

Gemma 4 26B with GGUF quantization supports context windows up to 8K-32K tokens depending on quantization. Here's how we budget it:

| Section | Est. Tokens |
|---------|------------|
| System prompt (tutor instructions) | ~800 |
| Learner model | ~300 |
| Lesson objectives + vocabulary | ~400 |
| Conversation history (last 20 turns) | ~2,000 |
| **Total** | **~3,500** |

This leaves ample room within an 8K context. For longer conversations, we can summarize older messages:

```
<conversation_summary>
Earlier in this conversation, the student ordered a coffee with milk
(cafe con leche) and churros. They used "me gustaria" correctly and
were corrected on "cuanto es" -> "cuanto cuesta/cuestan". Vocabulary
introduced so far: me gustaria, la cuenta, tortilla espanola, churros.
</conversation_summary>

[Recent messages continue from here...]
```

The summarization can be done by the same LLM model, triggered when conversation history exceeds ~15 turns.
