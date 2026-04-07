pub mod grammar_seeds;

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
                "SELECT id, mode, topic
                 FROM conversations ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(ConversationRow {
                    id: row.get(0)?,
                    mode: row.get(1)?,
                    topic: row.get(2)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }

    pub fn update_conversation_topic(
        &self,
        conversation_id: i64,
        topic: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET topic = ?1 WHERE id = ?2",
            params![topic, conversation_id],
        )
        .map_err(|e| format!("update topic: {e}"))?;
        Ok(())
    }

    /// Delete a conversation and its associated messages and corrections.
    /// Nulls out vocabulary references but keeps vocab/flashcard rows.
    pub fn delete_conversation(&self, conversation_id: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        // Delete corrections (references both messages and conversations).
        conn.execute(
            "DELETE FROM corrections WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("delete corrections: {e}"))?;
        // Delete messages.
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("delete messages: {e}"))?;
        // Null out vocabulary back-references (vocab is shared, don't delete it).
        conn.execute(
            "UPDATE vocabulary SET first_seen_conversation_id = NULL WHERE first_seen_conversation_id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("null vocab refs: {e}"))?;
        // Delete the conversation itself.
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![conversation_id],
        )
        .map_err(|e| format!("delete conversation: {e}"))?;
        Ok(())
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

    /// Fetch all messages for a given conversation, ordered chronologically.
    pub fn get_messages_by_conversation(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<MessageRow>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT role, content, translation
                 FROM messages
                 WHERE conversation_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![conversation_id], |row| {
                Ok(MessageRow {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    translation: row.get(2)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
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

    /// Fetch all flashcards joined with their vocabulary data.
    pub fn get_all_flashcards(&self) -> Result<Vec<FlashcardListRow>, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        let mut stmt = conn
            .prepare(
                "SELECT f.id, v.target_text, v.native_text, v.pronunciation,
                        v.example_sentence_target, v.example_sentence_native,
                        f.status, f.due_date, f.review_count, v.topic
                 FROM flashcards f
                 JOIN vocabulary v ON v.id = f.vocabulary_id
                 ORDER BY f.created_at DESC",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FlashcardListRowRaw {
                    id: row.get(0)?,
                    word: row.get(1)?,
                    meaning: row.get(2)?,
                    pronunciation: row.get(3)?,
                    example_target: row.get(4)?,
                    example_native: row.get(5)?,
                    status: row.get(6)?,
                    due_date: row.get(7)?,
                    review_count: row.get(8)?,
                    topic: row.get(9)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            let raw = r.map_err(|e| format!("row: {e}"))?;
            let is_due = raw.due_date.as_str() <= now.as_str();
            let display_status = if raw.status == "new" {
                "New"
            } else if is_due {
                "Due"
            } else if raw.status == "mature" {
                "Mature"
            } else {
                "Learning"
            };
            out.push(FlashcardListRow {
                id: raw.id,
                word: raw.word,
                meaning: raw.meaning,
                pronunciation: raw.pronunciation,
                example_target: raw.example_target,
                example_native: raw.example_native,
                status: display_status.to_string(),
                due_date: raw.due_date,
                review_count: raw.review_count,
                topic: raw.topic,
            });
        }
        Ok(out)
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

    // ── Grammar concepts ─────────────────────────────────────────────────

    /// Bulk-insert grammar concepts (used during onboarding seeding).
    pub fn insert_grammar_concepts(&self, concepts: &[NewGrammarConcept]) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        for c in concepts {
            conn.execute(
                "INSERT OR IGNORE INTO grammar_concepts (slug, name, description, cefr_level, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![c.slug, c.name, c.description, c.cefr_level, now],
            )
            .map_err(|e| format!("insert grammar concept: {e}"))?;
        }
        Ok(())
    }

    /// Fetch all grammar concepts, ordered by CEFR level then slug.
    pub fn get_grammar_concepts(&self) -> Result<Vec<GrammarConceptRow>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, slug, name, description, cefr_level, status, accuracy_rate, times_practiced
                 FROM grammar_concepts
                 ORDER BY cefr_level ASC, id ASC",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(GrammarConceptRow {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    cefr_level: row.get(4)?,
                    status: row.get(5)?,
                    accuracy_rate: row.get(6)?,
                    times_practiced: row.get(7)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }

    // ── Lessons ──────────────────────────────────────────────────────────

    /// Bulk-insert lessons generated by the LLM.
    pub fn insert_lessons(&self, lessons: &[NewLesson]) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        for l in lessons {
            conn.execute(
                "INSERT INTO lessons
                    (sequence_order, cefr_level, topic, title, description, scenario,
                     objectives, target_vocabulary, target_grammar, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'planned', ?10)",
                params![
                    l.sequence_order,
                    l.cefr_level,
                    l.topic,
                    l.title,
                    l.description,
                    l.scenario,
                    l.objectives_json,
                    l.target_vocabulary_json,
                    l.target_grammar_json,
                    now,
                ],
            )
            .map_err(|e| format!("insert lesson: {e}"))?;
        }
        Ok(())
    }

    /// Fetch all lessons ordered by sequence.
    pub fn get_lessons(&self) -> Result<Vec<LessonRow>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, sequence_order, cefr_level, topic, title, description,
                        scenario, objectives, target_vocabulary, target_grammar,
                        status, success_rate
                 FROM lessons
                 ORDER BY sequence_order ASC",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(LessonRow {
                    id: row.get(0)?,
                    sequence_order: row.get(1)?,
                    cefr_level: row.get(2)?,
                    topic: row.get(3)?,
                    title: row.get(4)?,
                    description: row.get(5)?,
                    scenario: row.get(6)?,
                    objectives_json: row.get(7)?,
                    target_vocabulary_json: row.get(8)?,
                    target_grammar_json: row.get(9)?,
                    status: row.get(10)?,
                    success_rate: row.get(11)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }

    /// Get a single lesson by id.
    pub fn get_lesson(&self, lesson_id: i64) -> Result<Option<LessonRow>, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, sequence_order, cefr_level, topic, title, description,
                    scenario, objectives, target_vocabulary, target_grammar,
                    status, success_rate
             FROM lessons WHERE id = ?1",
            params![lesson_id],
            |row| {
                Ok(LessonRow {
                    id: row.get(0)?,
                    sequence_order: row.get(1)?,
                    cefr_level: row.get(2)?,
                    topic: row.get(3)?,
                    title: row.get(4)?,
                    description: row.get(5)?,
                    scenario: row.get(6)?,
                    objectives_json: row.get(7)?,
                    target_vocabulary_json: row.get(8)?,
                    target_grammar_json: row.get(9)?,
                    status: row.get(10)?,
                    success_rate: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(|e| format!("get lesson: {e}"))
    }

    /// Update a lesson's status and optionally its success rate.
    pub fn update_lesson_status(
        &self,
        lesson_id: i64,
        status: &str,
        success_rate: Option<f64>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        match status {
            "in_progress" => {
                conn.execute(
                    "UPDATE lessons SET status = 'in_progress', started_at = ?1 WHERE id = ?2",
                    params![now, lesson_id],
                )
                .map_err(|e| format!("update lesson: {e}"))?;
            }
            "completed" => {
                conn.execute(
                    "UPDATE lessons SET status = 'completed', success_rate = ?1, completed_at = ?2 WHERE id = ?3",
                    params![success_rate, now, lesson_id],
                )
                .map_err(|e| format!("update lesson: {e}"))?;
            }
            _ => {
                conn.execute(
                    "UPDATE lessons SET status = ?1 WHERE id = ?2",
                    params![status, lesson_id],
                )
                .map_err(|e| format!("update lesson: {e}"))?;
            }
        }
        Ok(())
    }

    /// Create a conversation linked to a lesson.
    pub fn create_lesson_conversation(
        &self,
        lesson_id: i64,
        topic: &str,
    ) -> Result<i64, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        conn.execute(
            "INSERT INTO conversations (lesson_id, mode, topic, started_at, created_at)
             VALUES (?1, 'lesson', ?2, ?3, ?3)",
            params![lesson_id, topic, now],
        )
        .map_err(|e| format!("insert lesson conversation: {e}"))?;
        Ok(conn.last_insert_rowid())
    }

    // ── Flashcard review (SRS) ──────────────────────────────────────────

    /// Fetch due flashcards for review, ordered by priority.
    pub fn get_due_flashcards(&self, limit: usize) -> Result<Vec<DueFlashcardRow>, String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.vocabulary_id, v.target_text, v.native_text,
                        v.pronunciation, v.example_sentence_target, v.example_sentence_native,
                        f.status, f.ease_factor, f.interval_days, f.review_count
                 FROM flashcards f
                 JOIN vocabulary v ON v.id = f.vocabulary_id
                 WHERE f.due_date <= ?1
                 ORDER BY
                    CASE f.status
                        WHEN 'learning' THEN 0
                        WHEN 'new' THEN 1
                        WHEN 'review' THEN 2
                        ELSE 3
                    END,
                    f.due_date ASC
                 LIMIT ?2",
            )
            .map_err(|e| format!("prepare: {e}"))?;
        let rows = stmt
            .query_map(params![now, limit], |row| {
                Ok(DueFlashcardRow {
                    id: row.get(0)?,
                    vocabulary_id: row.get(1)?,
                    target_text: row.get(2)?,
                    native_text: row.get(3)?,
                    pronunciation: row.get(4)?,
                    example_target: row.get(5)?,
                    example_native: row.get(6)?,
                    status: row.get(7)?,
                    ease_factor: row.get(8)?,
                    interval_days: row.get(9)?,
                    review_count: row.get(10)?,
                })
            })
            .map_err(|e| format!("query: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| format!("row: {e}"))?);
        }
        Ok(out)
    }

    /// Record a flashcard review and update SRS state (SM-2 algorithm).
    pub fn review_flashcard(
        &self,
        flashcard_id: i64,
        rating: &str,
        response_time_ms: Option<i64>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let now = now_iso();

        // Get current SRS state.
        let (ease, interval, review_count, lapse_count): (f64, f64, i32, i32) = conn
            .query_row(
                "SELECT ease_factor, interval_days, review_count, lapse_count FROM flashcards WHERE id = ?1",
                params![flashcard_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|e| format!("get flashcard: {e}"))?;

        // Compute new SRS state based on rating.
        let (new_ease, new_interval, new_status, new_lapse_count) = match rating {
            "again" => {
                // Reset: short interval, decrease ease, mark as learning.
                let e = (ease - 0.2_f64).max(1.3);
                (e, 1.0 / 1440.0, "learning", lapse_count + 1) // ~1 minute
            }
            "hard" => {
                let e = (ease - 0.15_f64).max(1.3);
                let i = (interval * 1.2).max(1.0 / 1440.0);
                let status = if i > 21.0 { "mature" } else if i > 1.0 { "review" } else { "learning" };
                (e, i, status, lapse_count)
            }
            "good" => {
                let i = if interval < 1.0 / 1440.0 { 1.0 } else { interval * ease };
                let status = if i > 21.0 { "mature" } else if i > 1.0 { "review" } else { "learning" };
                (ease, i, status, lapse_count)
            }
            "easy" => {
                let e = ease + 0.15;
                let i = if interval < 1.0 / 1440.0 { 4.0 } else { interval * ease * 1.3 };
                let status = if i > 21.0 { "mature" } else { "review" };
                (e, i, status, lapse_count)
            }
            _ => return Err(format!("invalid rating: {rating}")),
        };

        // Compute new due date.
        let due_date = add_days_to_now(new_interval);

        // Update flashcard.
        conn.execute(
            "UPDATE flashcards
             SET ease_factor = ?1, interval_days = ?2, status = ?3,
                 due_date = ?4, review_count = ?5, lapse_count = ?6,
                 last_rating = ?7, last_reviewed = ?8
             WHERE id = ?9",
            params![
                new_ease,
                new_interval,
                new_status,
                due_date,
                review_count + 1,
                new_lapse_count,
                rating,
                now,
                flashcard_id,
            ],
        )
        .map_err(|e| format!("update flashcard: {e}"))?;

        // Insert review log.
        conn.execute(
            "INSERT INTO flashcard_reviews (flashcard_id, rating, response_time_ms, reviewed_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![flashcard_id, rating, response_time_ms, now],
        )
        .map_err(|e| format!("insert review: {e}"))?;

        Ok(())
    }

    // ── Daily stats ─────────────────────────────────────────────────────

    /// Upsert today's daily stats (increment counters).
    pub fn update_daily_stats(
        &self,
        new_vocab: i32,
        corrections: i32,
        messages: i32,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let today = today_iso();
        conn.execute(
            "INSERT INTO daily_stats (date, new_vocab_count, corrections_count, messages_sent, conversations_count)
             VALUES (?1, ?2, ?3, ?4, 1)
             ON CONFLICT(date) DO UPDATE SET
                new_vocab_count = daily_stats.new_vocab_count + excluded.new_vocab_count,
                corrections_count = daily_stats.corrections_count + excluded.corrections_count,
                messages_sent = daily_stats.messages_sent + excluded.messages_sent",
            params![today, new_vocab, corrections, messages],
        )
        .map_err(|e| format!("update daily stats: {e}"))?;
        Ok(())
    }

    /// Get the current streak (consecutive days with practice).
    pub fn get_streak(&self) -> Result<i32, String> {
        let conn = self.conn.lock().unwrap();
        let today = today_iso();
        // Check if there's activity today or yesterday to start counting.
        let streak: i32 = conn
            .query_row(
                "WITH RECURSIVE streak(d, n) AS (
                    SELECT ?1, 0
                    UNION ALL
                    SELECT date(d, '-1 day'), n + 1
                    FROM streak
                    WHERE EXISTS (
                        SELECT 1 FROM daily_stats
                        WHERE date = date(d, '-1 day')
                        AND (practice_time_min > 0 OR messages_sent > 0)
                    )
                 )
                 SELECT CASE
                    WHEN EXISTS (SELECT 1 FROM daily_stats WHERE date = ?1 AND (practice_time_min > 0 OR messages_sent > 0))
                    THEN (SELECT MAX(n) + 1 FROM streak)
                    ELSE (SELECT MAX(n) FROM streak)
                 END",
                params![today],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(streak)
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

pub struct MessageRow {
    pub role: String,
    pub content: String,
    pub translation: Option<String>,
}

pub struct ConversationRow {
    pub id: i64,
    pub mode: String,
    pub topic: Option<String>,
}

pub struct VocabRow {
    pub target_text: String,
    pub native_text: String,
    pub status: String,
}

/// Internal raw row before status computation.
struct FlashcardListRowRaw {
    id: i64,
    word: String,
    meaning: String,
    pronunciation: Option<String>,
    example_target: Option<String>,
    example_native: Option<String>,
    status: String,
    due_date: String,
    review_count: i32,
    topic: String,
}

pub struct FlashcardListRow {
    pub id: i64,
    pub word: String,
    pub meaning: String,
    pub pronunciation: Option<String>,
    pub example_target: Option<String>,
    pub example_native: Option<String>,
    pub status: String,
    pub due_date: String,
    pub review_count: i32,
    pub topic: String,
}

pub struct NewGrammarConcept {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub cefr_level: String,
}

pub struct GrammarConceptRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub cefr_level: String,
    pub status: String,
    pub accuracy_rate: Option<f64>,
    pub times_practiced: i32,
}

pub struct NewLesson {
    pub sequence_order: i32,
    pub cefr_level: String,
    pub topic: String,
    pub title: String,
    pub description: String,
    pub scenario: String,
    pub objectives_json: String,
    pub target_vocabulary_json: String,
    pub target_grammar_json: String,
}

pub struct LessonRow {
    pub id: i64,
    pub sequence_order: i32,
    pub cefr_level: String,
    pub topic: String,
    pub title: String,
    pub description: Option<String>,
    pub scenario: Option<String>,
    pub objectives_json: String,
    pub target_vocabulary_json: String,
    pub target_grammar_json: String,
    pub status: String,
    pub success_rate: Option<f64>,
}

pub struct DueFlashcardRow {
    pub id: i64,
    pub vocabulary_id: i64,
    pub target_text: String,
    pub native_text: String,
    pub pronunciation: Option<String>,
    pub example_target: Option<String>,
    pub example_native: Option<String>,
    pub status: String,
    pub ease_factor: f64,
    pub interval_days: f64,
    pub review_count: i32,
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

fn today_iso() -> String {
    let full = now_iso();
    full[..10].to_string() // "2026-04-06"
}

fn add_days_to_now(days: f64) -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let future_secs = secs + (days * 86400.0) as u64;
    let secs_per_day = 86400u64;
    let total_days = future_secs / secs_per_day;
    let rem = future_secs % secs_per_day;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, day) = epoch_days_to_ymd(total_days);
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
