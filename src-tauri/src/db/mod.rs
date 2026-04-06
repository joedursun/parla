use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Thread-safe database handle, managed as Tauri state.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    /// Open (or create) the database at the given path and run migrations.
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn =
            Connection::open(path).map_err(|e| format!("failed to open database: {e}"))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| format!("failed to set pragmas: {e}"))?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(SCHEMA).map_err(|e| format!("migration failed: {e}"))
    }

    // ── Learner profile ──────────────────────────────────────────────────

    pub fn create_profile(&self, profile: &NewProfile) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        let goals_json =
            serde_json::to_string(&profile.goals).map_err(|e| format!("json: {e}"))?;
        conn.execute(
            "INSERT OR REPLACE INTO learner_profile
                (id, native_language, target_language, cefr_level, goals, daily_goal_min, created_at, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, 15, ?5, ?5)",
            params![
                profile.native_language,
                profile.target_language,
                profile.cefr_level,
                goals_json,
                now,
            ],
        )
        .map_err(|e| format!("insert profile: {e}"))?;
        Ok(())
    }

    pub fn get_profile(&self) -> Result<Option<ProfileRow>, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT native_language, target_language, cefr_level, goals FROM learner_profile WHERE id = 1",
            [],
            |row| {
                Ok(ProfileRow {
                    native_language: row.get(0)?,
                    target_language: row.get(1)?,
                    cefr_level: row.get(2)?,
                    goals_json: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|e| format!("get profile: {e}"))
    }

    // ── Conversations ────────────────────────────────────────────────────

    pub fn create_conversation(&self, mode: &str, topic: Option<&str>) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO conversations (mode, topic, started_at, created_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![mode, topic, now],
        )
        .map_err(|e| format!("insert conversation: {e}"))?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_recent_conversations(&self, limit: usize) -> Result<Vec<ConversationRow>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, mode, topic, message_count, started_at
                 FROM conversations ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    mode: row.get(1)?,
                    topic: row.get(2)?,
                    message_count: row.get(3)?,
                    started_at: row.get(4)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }

    pub fn update_conversation_counts(
        &self,
        conversation_id: i64,
        message_count: i32,
        error_count: i32,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET message_count = ?1, error_count = ?2 WHERE id = ?3",
            params![message_count, error_count, conversation_id],
        )
        .map_err(|e| format!("update conversation: {e}"))?;
        Ok(())
    }

    // ── Messages ─────────────────────────────────────────────────────────

    pub fn insert_message(&self, msg: &NewMessage) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO messages (conversation_id, role, content, translation, input_method, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.conversation_id,
                msg.role,
                msg.content,
                msg.translation,
                msg.input_method,
                now,
            ],
        )
        .map_err(|e| format!("insert message: {e}"))?;
        Ok(conn.last_insert_rowid())
    }

    // ── Corrections ──────────────────────────────────────────────────────

    pub fn insert_correction(&self, c: &NewCorrection) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO corrections (message_id, conversation_id, original_text, corrected_text, explanation, error_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                c.message_id,
                c.conversation_id,
                c.original_text,
                c.corrected_text,
                c.explanation,
                c.error_type,
                now,
            ],
        )
        .map_err(|e| format!("insert correction: {e}"))?;
        Ok(conn.last_insert_rowid())
    }

    // ── Vocabulary ───────────────────────────────────────────────────────

    /// Upsert a vocabulary item. Returns the vocabulary row id.
    pub fn upsert_vocabulary(&self, v: &NewVocabulary) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO vocabulary
                (target_text, native_text, pronunciation, part_of_speech, topic,
                 example_sentence_target, example_sentence_native,
                 first_seen_conversation_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(target_text, topic) DO UPDATE SET
                native_text = excluded.native_text,
                pronunciation = COALESCE(excluded.pronunciation, vocabulary.pronunciation),
                example_sentence_target = COALESCE(excluded.example_sentence_target, vocabulary.example_sentence_target),
                example_sentence_native = COALESCE(excluded.example_sentence_native, vocabulary.example_sentence_native)",
            params![
                v.target_text,
                v.native_text,
                v.pronunciation,
                v.part_of_speech,
                v.topic,
                v.example_target,
                v.example_native,
                v.conversation_id,
                now,
            ],
        )
        .map_err(|e| format!("upsert vocabulary: {e}"))?;

        // Get the id of the inserted/updated row.
        let id: i64 = conn
            .query_row(
                "SELECT id FROM vocabulary WHERE target_text = ?1 AND topic = ?2",
                params![v.target_text, v.topic],
                |row| row.get(0),
            )
            .map_err(|e| format!("get vocab id: {e}"))?;
        Ok(id)
    }

    // ── Flashcards ───────────────────────────────────────────────────────

    /// Create a flashcard for a vocabulary item if one doesn't already exist.
    pub fn ensure_flashcard(&self, vocabulary_id: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT OR IGNORE INTO flashcards (vocabulary_id, due_date, created_at)
             VALUES (?1, ?2, ?2)",
            params![vocabulary_id, now],
        )
        .map_err(|e| format!("insert flashcard: {e}"))?;
        Ok(())
    }

    pub fn flashcards_due_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.query_row(
            "SELECT COUNT(*) FROM flashcards WHERE due_date <= ?1",
            params![now],
            |row| row.get(0),
        )
        .map_err(|e| format!("count due: {e}"))
    }

    // ── Recent vocabulary ────────────────────────────────────────────────

    pub fn recent_vocabulary(&self, limit: usize) -> Result<Vec<VocabRow>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT v.target_text, v.native_text, COALESCE(f.status, 'new') as fstatus
                 FROM vocabulary v
                 LEFT JOIN flashcards f ON f.vocabulary_id = v.id
                 ORDER BY v.created_at DESC
                 LIMIT ?1",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(VocabRow {
                    target_text: row.get(0)?,
                    native_text: row.get(1)?,
                    status: row.get(2)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }
}

// ── Row / param types ────────────────────────────────────────────────────

pub struct NewProfile {
    pub native_language: String,
    pub target_language: String,
    pub cefr_level: String,
    pub goals: Vec<String>,
}

pub struct ProfileRow {
    pub native_language: String,
    pub target_language: String,
    pub cefr_level: String,
    pub goals_json: String,
}

pub struct NewMessage {
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub translation: Option<String>,
    pub input_method: String,
}

pub struct NewCorrection {
    pub message_id: i64,
    pub conversation_id: i64,
    pub original_text: String,
    pub corrected_text: String,
    pub explanation: String,
    pub error_type: String,
}

pub struct NewVocabulary {
    pub target_text: String,
    pub native_text: String,
    pub pronunciation: Option<String>,
    pub part_of_speech: Option<String>,
    pub topic: String,
    pub example_target: Option<String>,
    pub example_native: Option<String>,
    pub conversation_id: Option<i64>,
}

pub struct ConversationRow {
    pub id: i64,
    pub mode: String,
    pub topic: Option<String>,
    pub message_count: i32,
    pub started_at: String,
}

pub struct VocabRow {
    pub target_text: String,
    pub native_text: String,
    pub status: String,
}

fn now_iso() -> String {
    // Use a simple UTC timestamp without pulling in chrono.
    // Format: 2026-04-06T12:34:56Z
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs_per_day = 86400u64;
    let days = d / secs_per_day;
    let rem = d % secs_per_day;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;

    // Days since epoch to Y-M-D (simplified leap year calculation).
    let (y, mo, day) = epoch_days_to_ymd(days);
    format!("{y:04}-{mo:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ── Schema ───────────────────────────────────────────────────────────────

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS learner_profile (
    id              INTEGER PRIMARY KEY DEFAULT 1,
    native_language TEXT NOT NULL,
    target_language TEXT NOT NULL,
    cefr_level      TEXT NOT NULL,
    goals           TEXT NOT NULL,
    daily_goal_min  INTEGER DEFAULT 15,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS lessons (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence_order    INTEGER NOT NULL,
    cefr_level        TEXT NOT NULL,
    topic             TEXT NOT NULL,
    title             TEXT NOT NULL,
    description       TEXT,
    scenario          TEXT,
    objectives        TEXT NOT NULL,
    target_vocabulary TEXT NOT NULL,
    target_grammar    TEXT NOT NULL,
    status            TEXT DEFAULT 'planned',
    success_rate      REAL,
    started_at        TEXT,
    completed_at      TEXT,
    created_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    lesson_id      INTEGER,
    mode           TEXT NOT NULL,
    topic          TEXT,
    summary        TEXT,
    vocab_introduced TEXT,
    grammar_practiced TEXT,
    error_count    INTEGER DEFAULT 0,
    message_count  INTEGER DEFAULT 0,
    started_at     TEXT NOT NULL,
    ended_at       TEXT,
    created_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id),
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    translation     TEXT,
    audio_path      TEXT,
    input_method    TEXT DEFAULT 'text',
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS corrections (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id        INTEGER NOT NULL REFERENCES messages(id),
    conversation_id   INTEGER NOT NULL REFERENCES conversations(id),
    original_text     TEXT NOT NULL,
    corrected_text    TEXT NOT NULL,
    explanation       TEXT NOT NULL,
    error_type        TEXT NOT NULL,
    grammar_concept   TEXT,
    created_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS vocabulary (
    id                      INTEGER PRIMARY KEY AUTOINCREMENT,
    target_text             TEXT NOT NULL,
    native_text             TEXT NOT NULL,
    pronunciation           TEXT,
    part_of_speech          TEXT,
    gender                  TEXT,
    topic                   TEXT NOT NULL,
    example_sentence_target TEXT,
    example_sentence_native TEXT,
    first_seen_lesson_id    INTEGER REFERENCES lessons(id),
    first_seen_conversation_id INTEGER REFERENCES conversations(id),
    audio_path              TEXT,
    created_at              TEXT NOT NULL,
    UNIQUE(target_text, topic)
);

CREATE TABLE IF NOT EXISTS flashcards (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    vocabulary_id   INTEGER NOT NULL REFERENCES vocabulary(id) UNIQUE,
    card_type       TEXT DEFAULT 'vocabulary',
    status          TEXT DEFAULT 'new',
    ease_factor     REAL DEFAULT 2.5,
    interval_days   REAL DEFAULT 0,
    due_date        TEXT NOT NULL,
    review_count    INTEGER DEFAULT 0,
    lapse_count     INTEGER DEFAULT 0,
    last_rating     TEXT,
    last_reviewed   TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS flashcard_reviews (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    flashcard_id    INTEGER NOT NULL REFERENCES flashcards(id),
    rating          TEXT NOT NULL,
    response_time_ms INTEGER,
    reviewed_at     TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS grammar_concepts (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    slug            TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    description     TEXT,
    examples        TEXT,
    cefr_level      TEXT NOT NULL,
    status          TEXT DEFAULT 'not_started',
    accuracy_rate   REAL,
    times_practiced INTEGER DEFAULT 0,
    times_correct   INTEGER DEFAULT 0,
    first_introduced_lesson_id INTEGER REFERENCES lessons(id),
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS daily_stats (
    date                TEXT PRIMARY KEY,
    practice_time_min   INTEGER DEFAULT 0,
    conversations_count INTEGER DEFAULT 0,
    messages_sent       INTEGER DEFAULT 0,
    new_vocab_count     INTEGER DEFAULT 0,
    flashcards_reviewed INTEGER DEFAULT 0,
    flashcard_accuracy  REAL,
    corrections_count   INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS weak_areas (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    concept        TEXT NOT NULL,
    concept_type   TEXT NOT NULL,
    accuracy_rate  REAL NOT NULL,
    sample_errors  TEXT,
    suggestion     TEXT,
    last_assessed  TEXT NOT NULL,
    resolved       INTEGER DEFAULT 0
);
";
