mod audio;
mod db;
mod llm;
mod stt;
mod tts;
mod vad;

use audio::pipeline::AudioState;
use db::Db;
use llm::parser::{ParsedTutorResponse, StreamingJsonParser};
use llm::prompt::{build_lesson_generation_prompt, build_system_prompt, ChatMessage};
use llm::{GenChunk, LlmState};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, RunEvent, State};
use tts::{detect_language_from_text, language_name_to_code};

/// The user's target language code (e.g. "ko", "es"), shared across threads.
#[derive(Clone)]
struct TargetLanguage(Arc<Mutex<String>>);

impl TargetLanguage {
    fn new(code: &str) -> Self {
        Self(Arc::new(Mutex::new(code.to_string())))
    }
    fn get(&self) -> String {
        self.0.lock().unwrap().clone()
    }
    fn set(&self, code: &str) {
        *self.0.lock().unwrap() = code.to_string();
    }
}

/// Resolve the models directory: <app_data_dir>/models/
fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to get app data dir: {e}"))?;
    let dir = data_dir.join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create models dir: {e}"))?;
    Ok(dir)
}

/// Find a Gemma GGUF model file in the given directory.
/// Accepts any file matching `gemma*.gguf` (Q4_K_M, Q4_0, etc).
fn find_gemma_model(dir: &Path) -> Result<PathBuf, String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        // Prefer Q4_K_M > Q5_K_M > Q4_0, but accept any gemma .gguf.
        let mut candidates: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
                name.contains("gemma") && name.ends_with(".gguf")
            })
            .collect();
        candidates.sort_by_key(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
            // Lower score = preferred.
            if n.contains("q4_k_m") { 0 }
            else if n.contains("q5_k_m") { 1 }
            else if n.contains("q4_0") { 2 }
            else { 3 }
        });
        if let Some(p) = candidates.into_iter().next() {
            return Ok(p);
        }
    }
    Err(format!(
        "No Gemma .gguf model found in {}. Run ./setup.sh to download.",
        dir.display()
    ))
}

/// Find a Whisper model file in the given directory.
fn find_whisper_model(dir: &Path) -> Result<PathBuf, String> {
    // Prefer multilingual models over .en (English-only) variants since we
    // need to transcribe in the student's target language.
    let patterns = [
        "ggml-large-v3-turbo.bin",
        "ggml-large-v3.bin",
        "ggml-medium.bin",
        "ggml-small.bin",
        "ggml-base.bin",
        "ggml-tiny.bin",
        // English-only fallbacks (will only transcribe English)
        "ggml-small.en.bin",
        "ggml-base.en.bin",
        "ggml-medium.en.bin",
    ];
    for name in patterns {
        let path = dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("ggml-") && name_str.ends_with(".bin") {
                return Ok(entry.path());
            }
        }
    }
    Err(format!(
        "No Whisper model found in {}. Run ./setup.sh to download models.",
        dir.display()
    ))
}

// ── Tauri commands ─────────────────────────────────────────────────────────
// All commands that do real work are async to avoid blocking the main thread.

#[tauri::command]
async fn start_recording(state: State<'_, AudioState>) -> Result<(), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.start_recording())
        .await
        .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn stop_recording(state: State<'_, AudioState>) -> Result<StopRecordingResult, String> {
    let state = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || state.stop_recording())
        .await
        .map_err(|e| format!("{e}"))??;
    Ok(make_recording_result(&result))
}

#[tauri::command]
async fn stop_recording_and_transcribe(
    state: State<'_, AudioState>,
    target_lang: State<'_, TargetLanguage>,
) -> Result<TranscriptionResult, String> {
    let state = state.inner().clone();
    let lang_code = target_lang.get();
    // Map our language code to Whisper's expected code
    let whisper_lang = whisper_lang_code(&lang_code);

    let result = tokio::task::spawn_blocking(move || {
        let rec = state.stop_recording()?;
        let segments: Vec<(usize, usize)> = rec
            .speech_segments
            .iter()
            .map(|s| (s.start_sample, s.end_sample))
            .collect();
        // Transcribe in the target language
        let transcription = state
            .transcribe(&rec.samples, &segments, Some(&whisper_lang))
            .unwrap_or_default();
        // Also translate to English for the subtitle
        let translation = if whisper_lang != "en" {
            state.translate(&rec.samples, &segments).unwrap_or_default()
        } else {
            String::new()
        };
        Ok::<_, String>((rec, transcription, translation))
    })
    .await
    .map_err(|e| format!("{e}"))??;

    let (rec, transcription, translation) = result;
    let recording = make_recording_result(&rec);
    Ok(TranscriptionResult {
        duration_ms: recording.duration_ms,
        sample_count: recording.sample_count,
        speech_segments: recording.speech_segments,
        transcription,
        translation,
    })
}

/// Map our internal language codes to Whisper language codes.
fn whisper_lang_code(lang: &str) -> String {
    match lang {
        "zh" => "zh".to_string(),
        "ko" => "ko".to_string(),
        "ja" => "ja".to_string(),
        "en" => "en".to_string(),
        "es" => "es".to_string(),
        "fr" => "fr".to_string(),
        "de" => "de".to_string(),
        "tr" => "tr".to_string(),
        "it" => "it".to_string(),
        "pt" => "pt".to_string(),
        other => other.to_string(),
    }
}

#[tauri::command]
async fn loopback_test(state: State<'_, AudioState>) -> Result<StopRecordingResult, String> {
    let state = state.inner().clone();
    let result = tokio::task::spawn_blocking(move || {
        let rec = state.stop_recording()?;
        let recording = make_recording_result(&rec);
        state.play_audio(rec.samples, 16000)?;
        Ok::<_, String>(recording)
    })
    .await
    .map_err(|e| format!("{e}"))?;
    result
}

#[tauri::command]
async fn speak_text(
    state: State<'_, AudioState>,
    target_lang: State<'_, TargetLanguage>,
    text: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lang = detect_language_from_text(&text)
        .map(|s| s.to_string())
        .unwrap_or_else(|| target_lang.get());
    tokio::task::spawn_blocking(move || {
        let output = state.synthesize(&text, &lang)?;
        if !output.samples.is_empty() {
            state.play_audio(output.samples, output.sample_rate)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn stop_playback(state: State<'_, AudioState>) -> Result<(), String> {
    state.stop_playback();
    Ok(())
}

#[tauri::command]
async fn audio_status(state: State<'_, AudioState>) -> Result<AudioStatusResult, String> {
    let status = state.status();
    Ok(AudioStatusResult {
        is_recording: status.is_recording,
        is_playing: status.is_playing,
        vad_active: status.vad_active,
        stt_ready: status.stt_ready,
        tts_ready: status.tts_ready,
        speech_detected: status.speech_detected,
    })
}

#[tauri::command]
async fn check_models(app: tauri::AppHandle) -> Result<ModelStatus, String> {
    let dir = models_dir(&app)?;
    Ok(ModelStatus {
        vad: dir.join("silero_vad.onnx").exists(),
        stt: find_whisper_model(&dir).is_ok(),
        llm: find_gemma_model(&dir).is_ok(),
        models_dir: dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
async fn llm_status(state: State<'_, LlmState>) -> Result<LlmStatusResult, String> {
    let s = state.inner().clone();
    let status = tokio::task::spawn_blocking(move || s.status())
        .await
        .map_err(|e| format!("{e}"))?;
    Ok(LlmStatusResult {
        loaded: status.loaded,
    })
}

// ── Profile commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn get_profile(db: State<'_, Db>) -> Result<Option<ProfileResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let row = db.get_profile()?;
        match row {
            None => Ok(None),
            Some(r) => {
                let goals: Vec<String> =
                    serde_json::from_str(&r.goals_json).unwrap_or_default();
                Ok(Some(ProfileResult {
                    native_language: r.native_language,
                    target_language: r.target_language,
                    cefr_level: r.cefr_level,
                    goals,
                }))
            }
        }
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn create_profile(
    db: State<'_, Db>,
    history: State<'_, ConversationHistory>,
    target_lang_state: State<'_, TargetLanguage>,
    llm: State<'_, LlmState>,
    app: tauri::AppHandle,
    native_language: String,
    target_language: String,
    cefr_level: String,
    goals: Vec<String>,
) -> Result<ProfileResult, String> {
    // Update the TTS target language immediately.
    target_lang_state.set(language_name_to_code(&target_language));

    let db = db.inner().clone();
    let history = history.inner().clone();
    let tl = target_language.clone();
    let nl = native_language.clone();
    let cl = cefr_level.clone();
    let g = goals.clone();
    let db2 = db.clone();
    let result: ProfileResult = tokio::task::spawn_blocking(move || -> Result<ProfileResult, String> {
        db.create_profile(&db::NewProfile {
            native_language: nl.clone(),
            target_language: tl.clone(),
            cefr_level: cl.clone(),
            goals: g.clone(),
        })?;

        // Seed grammar concepts for the selected language + level.
        let concepts = db::grammar_seeds::grammar_concepts_for(&tl, &cl);
        db.insert_grammar_concepts(&concepts)?;
        println!("[parla] seeded {} grammar concepts for {} {}", concepts.len(), tl, cl);

        // Update the system prompt for the current session.
        let goal_refs: Vec<&str> = g.iter().map(|s| s.as_str()).collect();
        let prompt = build_system_prompt(&tl, "Learner", &nl, &cl, &goal_refs, None);
        history.set_system_prompt(prompt);
        Ok(ProfileResult {
            native_language: nl,
            target_language: tl,
            cefr_level: cl,
            goals: g,
        })
    })
    .await
    .map_err(|e| format!("{e}"))??;

    // Trigger lesson generation in background (non-blocking).
    let llm = llm.inner().clone();
    tokio::task::spawn_blocking(move || {
        let goal_refs: Vec<&str> = goals.iter().map(|s| s.as_str()).collect();
        if let Err(e) = generate_initial_lessons(
            &llm,
            &db2,
            &app,
            &target_language,
            &native_language,
            &cefr_level,
            &goal_refs,
        ) {
            eprintln!("[parla] lesson generation failed: {e}");
        }
    });

    Ok(result)
}

// ── Conversation list commands ────────────────────────────────────────────

#[tauri::command]
async fn get_recent_conversations(
    db: State<'_, Db>,
) -> Result<Vec<RecentConversationResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let rows = db.get_recent_conversations(20)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let title = r
                    .topic
                    .unwrap_or_else(|| format!("{} conversation", r.mode));
                RecentConversationResult {
                    id: r.id.to_string(),
                    title,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn rename_conversation(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    conversation_id: i64,
    title: String,
) -> Result<(), String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        db.update_conversation_topic(conversation_id, &title)?;
        emit_recent_conversations(&db, &app);
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

#[tauri::command]
async fn delete_conversation(
    db: State<'_, Db>,
    history: State<'_, ConversationHistory>,
    app: tauri::AppHandle,
    conversation_id: i64,
) -> Result<(), String> {
    let db = db.inner().clone();
    let history = history.inner().clone();
    tokio::task::spawn_blocking(move || {
        db.delete_conversation(conversation_id)?;
        if history.get_conversation_id() == Some(conversation_id) {
            history.clear();
        }
        emit_recent_conversations(&db, &app);
        let due = db.flashcards_due_count().unwrap_or(0);
        let _ = app.emit("flashcards-due-count", due);
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

// ── Flashcard commands ───────────────────────────────────────────────

#[tauri::command]
async fn get_flashcards(db: State<'_, Db>) -> Result<Vec<FlashcardResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let rows = db.get_all_flashcards()?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let dots: Vec<bool> = (0..5).map(|i| i < r.review_count.min(5)).collect();
                FlashcardResult {
                    id: r.id,
                    word: r.word,
                    meaning: r.meaning,
                    pronunciation: r.pronunciation.unwrap_or_default(),
                    example_target: r.example_target.unwrap_or_default(),
                    example_native: r.example_native.unwrap_or_default(),
                    source: r.topic,
                    status: r.status,
                    next_review: r.due_date,
                    dots,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

// ── Lesson commands ──────────────────────────────────────────────────

#[tauri::command]
async fn get_lessons(db: State<'_, Db>) -> Result<Vec<LessonResult>, String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        let rows = db.get_lessons()?;
        Ok(rows
            .into_iter()
            .map(|l| LessonResult {
                id: l.id,
                sequence_order: l.sequence_order,
                title: l.title,
                description: l.description.unwrap_or_default(),
                status: l.status,
                topic: l.topic,
                cefr_level: l.cefr_level,
                success_rate: l.success_rate,
            })
            .collect())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

/// Start a lesson: mark it as in_progress, create a lesson conversation,
/// set the system prompt with lesson context, and return the lesson details.
#[tauri::command]
async fn start_lesson(
    db: State<'_, Db>,
    history: State<'_, ConversationHistory>,
    lesson_id: i64,
) -> Result<LessonResult, String> {
    let db = db.inner().clone();
    let history = history.inner().clone();
    tokio::task::spawn_blocking(move || {
        let lesson = db
            .get_lesson(lesson_id)?
            .ok_or_else(|| format!("lesson {lesson_id} not found"))?;

        // Mark lesson as in_progress.
        db.update_lesson_status(lesson_id, "in_progress", None)?;

        // Create a conversation linked to this lesson.
        let conv_id = db.create_lesson_conversation(lesson_id, &lesson.title)?;

        // Reset conversation history and set the lesson conversation.
        history.clear();
        history.set_conversation_id(conv_id);

        // Build system prompt with lesson context.
        let profile = db
            .get_profile()?
            .ok_or("no profile")?;
        let goals: Vec<String> =
            serde_json::from_str(&profile.goals_json).unwrap_or_default();
        let goal_refs: Vec<&str> = goals.iter().map(|s| s.as_str()).collect();

        // Parse lesson vocabulary and grammar from JSON.
        let vocab_items: Vec<String> =
            serde_json::from_str(&lesson.target_vocabulary_json).unwrap_or_default();
        let grammar_slugs: Vec<String> =
            serde_json::from_str(&lesson.target_grammar_json).unwrap_or_default();
        let objectives: Vec<String> =
            serde_json::from_str(&lesson.objectives_json).unwrap_or_default();

        use llm::prompt::{GrammarFocus, LessonContext, VocabItem};

        let vocab_refs: Vec<VocabItem> = vocab_items
            .iter()
            .map(|v| VocabItem {
                target: v.as_str(),
                native: "", // LLM will figure out translations
            })
            .collect();
        let grammar_refs: Vec<GrammarFocus> = grammar_slugs
            .iter()
            .map(|g| GrammarFocus {
                title: g.as_str(),
                explanation: "",
            })
            .collect();
        let obj_refs: Vec<&str> = objectives.iter().map(|s| s.as_str()).collect();

        let lesson_ctx = LessonContext {
            topic: &lesson.title,
            scenario: lesson.scenario.as_deref().unwrap_or(""),
            objectives: &obj_refs,
            vocabulary: &vocab_refs,
            grammar: &grammar_refs,
        };

        let prompt = build_system_prompt(
            &profile.target_language,
            "Learner",
            &profile.native_language,
            &profile.cefr_level,
            &goal_refs,
            Some(&lesson_ctx),
        );
        history.set_system_prompt(prompt);

        Ok(LessonResult {
            id: lesson.id,
            sequence_order: lesson.sequence_order,
            title: lesson.title,
            description: lesson.description.unwrap_or_default(),
            status: "in_progress".into(),
            topic: lesson.topic,
            cefr_level: lesson.cefr_level,
            success_rate: lesson.success_rate,
        })
    })
    .await
    .map_err(|e| format!("{e}"))?
}

// ── Flashcard review commands ───────────────────────────────────────

#[tauri::command]
async fn review_flashcard(
    db: State<'_, Db>,
    app: tauri::AppHandle,
    flashcard_id: i64,
    rating: String,
    response_time_ms: Option<i64>,
) -> Result<(), String> {
    let db = db.inner().clone();
    tokio::task::spawn_blocking(move || {
        db.review_flashcard(flashcard_id, &rating, response_time_ms)?;
        // Re-emit due count.
        let due = db.flashcards_due_count().unwrap_or(0);
        let _ = app.emit("flashcards-due-count", due);
        Ok(())
    })
    .await
    .map_err(|e| format!("{e}"))?
}

// ── Conversation loading ─────────────────────────────────────────────

/// Load a previous conversation from the database, restoring its messages
/// into the in-memory ConversationHistory so the user can continue it.
#[tauri::command]
async fn load_conversation(
    db: State<'_, Db>,
    history: State<'_, ConversationHistory>,
    conversation_id: i64,
) -> Result<Vec<LoadedMessage>, String> {
    let db = db.inner().clone();
    let history = history.inner().clone();
    tokio::task::spawn_blocking(move || {
        let messages = db.get_messages_by_conversation(conversation_id)?;

        // Reset and repopulate ConversationHistory.
        history.clear();
        history.set_conversation_id(conversation_id);

        let mut result = Vec::new();
        for msg in &messages {
            let chat_msg = match msg.role.as_str() {
                "student" => ChatMessage::user(msg.content.clone()),
                "tutor" => ChatMessage::assistant(msg.content.clone()),
                _ => continue,
            };
            history.push(chat_msg);
            history.increment_messages();

            result.push(LoadedMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                translation: msg.translation.clone().unwrap_or_default(),
            });
        }

        Ok(result)
    })
    .await
    .map_err(|e| format!("{e}"))?
}

/// Reset the conversation history for the current session.
#[tauri::command]
async fn reset_conversation(history: State<'_, ConversationHistory>) -> Result<(), String> {
    history.inner().clear();
    Ok(())
}

/// Begin a tutor-led lesson: the tutor chooses a topic and initiates.
/// Resets conversation, sets a tutor-led system prompt, and auto-sends an
/// opening so the tutor starts talking immediately.
#[tauri::command]
async fn begin_tutor_lesson(
    app: tauri::AppHandle,
    audio: State<'_, AudioState>,
    llm: State<'_, LlmState>,
    history: State<'_, ConversationHistory>,
    cancel: State<'_, CancelFlag>,
    target_lang: State<'_, TargetLanguage>,
    db: State<'_, Db>,
) -> Result<ConversationTurnResult, String> {
    // Reset conversation state for a fresh lesson.
    history.inner().clear();
    store_clear_lesson_state(&app);

    // Load profile to build the system prompt.
    let db_inner = db.inner().clone();
    let profile = tokio::task::spawn_blocking(move || db_inner.get_profile())
        .await
        .map_err(|e| format!("{e}"))??
        .ok_or("no profile — complete onboarding first")?;

    let goals: Vec<String> =
        serde_json::from_str(&profile.goals_json).unwrap_or_default();
    let goal_refs: Vec<&str> = goals.iter().map(|s| s.as_str()).collect();

    // Build a tutor-led system prompt: same as free conversation, but with
    // an instruction for the tutor to choose the topic and lead the lesson.
    let mut prompt = build_system_prompt(
        &profile.target_language,
        "Learner",
        &profile.native_language,
        &profile.cefr_level,
        &goal_refs,
        None, // no specific lesson context — tutor picks the topic
    );
    // Override the closing instruction for tutor-led mode.
    let free_closing = "Start by greeting the student warmly and asking what they'd like to talk about.";
    let tutor_led_closing = "The student has just started a lesson. YOU choose the conversation topic — \
        pick something interesting, practical, and relevant to their goals and level. \
        Do NOT ask them what they want to talk about. Instead, greet them warmly, \
        set the scene for a realistic conversation situation (e.g. ordering at a restaurant, \
        asking for directions, meeting someone new), and begin the lesson immediately. \
        Keep your opening to 2-3 sentences.";
    prompt = prompt.replace(free_closing, tutor_led_closing);

    history.inner().set_system_prompt(prompt);

    // Update the target language for TTS.
    target_lang.set(language_name_to_code(&profile.target_language));

    let student_text = "I'm ready for today's lesson.".to_string();

    let cancel_flag = cancel.inner().0.clone();
    cancel_flag.store(false, Ordering::Relaxed);

    let user_msg = ChatMessage::user(student_text.clone());
    let mut messages = history.inner().messages_with_system();
    messages.push(user_msg.clone());
    history.inner().push(user_msg);

    let llm = llm.inner().clone();
    let llm_for_db = llm.clone();
    let audio_handle = audio.inner().clone();
    let app_handle = app.clone();
    let tts_lang = target_lang.get();

    let result = tokio::task::spawn_blocking(move || {
        run_streaming_turn(&llm, messages, cancel_flag, &audio_handle, &app_handle, &tts_lang)
    })
    .await
    .map_err(|e| format!("{e}"))??;

    history.inner().push(ChatMessage::assistant(result.raw.clone()));
    let _ = app.emit("tutor-message-done", &result);

    if let Some(db) = app.try_state::<Db>() {
        let db = db.inner().clone();
        let history = history.inner().clone();
        let student_text_for_db = student_text.clone();
        let result_for_db = result.clone();
        let app_for_db = app.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = persist_turn(&db, &history, &student_text_for_db, &result_for_db, &app_for_db, &llm_for_db) {
                eprintln!("[db] persist_turn failed: {e}");
            }
        });
    }

    Ok(result)
}

/// Emit store-clear event for lesson state (used when starting fresh).
fn store_clear_lesson_state(app: &tauri::AppHandle) {
    let _ = app.emit("lesson-cleared", ());
}

/// Run one conversation turn: student text in → streamed tutor response out.
/// Synthesizes
/// complete sentences to TTS as they form, and emits a final
/// `tutor-message-done` event with the parsed structured response.
#[tauri::command]
async fn conversation_turn(
    app: tauri::AppHandle,
    audio: State<'_, AudioState>,
    llm: State<'_, LlmState>,
    history: State<'_, ConversationHistory>,
    cancel: State<'_, CancelFlag>,
    target_lang: State<'_, TargetLanguage>,
    student_text: String,
) -> Result<ConversationTurnResult, String> {
    let student_text = student_text.trim().to_string();
    if student_text.is_empty() {
        return Err("empty student message".into());
    }

    // Reset cancel flag for this new turn.
    let cancel_flag = cancel.inner().0.clone();
    cancel_flag.store(false, Ordering::Relaxed);

    // Build the message list from history + new user turn.
    let user_msg = ChatMessage::user(student_text.clone());
    let mut messages = history.inner().messages_with_system();
    messages.push(user_msg.clone());

    // Record the student turn immediately.
    history.inner().push(user_msg);

    let llm = llm.inner().clone();
    let llm_for_db = llm.clone();
    let audio_handle = audio.inner().clone();
    let app_handle = app.clone();
    let tts_lang = target_lang.get();

    let result = tokio::task::spawn_blocking(move || {
        run_streaming_turn(&llm, messages, cancel_flag, &audio_handle, &app_handle, &tts_lang)
    })
    .await
    .map_err(|e| format!("{e}"))??;

    history.inner().push(ChatMessage::assistant(result.raw.clone()));
    let _ = app.emit("tutor-message-done", &result);

    if let Some(db) = app.try_state::<Db>() {
        let db = db.inner().clone();
        let history = history.inner().clone();
        let student_text_for_db = student_text.clone();
        let result_for_db = result.clone();
        let app_for_db = app.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = persist_turn(
                &db,
                &history,
                &student_text_for_db,
                &result_for_db,
                &app_for_db,
                &llm_for_db,
            ) {
                eprintln!("[db] persist_turn failed: {e}");
            }
        });
    }

    Ok(result)
}

/// Cancel any in-flight LLM generation for the current turn.
#[tauri::command]
async fn cancel_generation(cancel: State<'_, CancelFlag>) -> Result<(), String> {
    cancel.inner().0.store(true, Ordering::Relaxed);
    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Generate a short conversation title via the LLM.
fn generate_conversation_title(
    llm: &LlmState,
    student_text: &str,
    tutor_text: &str,
) -> Option<String> {
    let messages = vec![
        ChatMessage::system(
            "Generate a short title (3-5 words) for this language learning conversation. \
             Reply with ONLY the title, nothing else. No quotes, no punctuation at the end.",
        ),
        ChatMessage::user(format!(
            "Student: {}\nTutor: {}",
            student_text, tutor_text
        )),
    ];
    let cancel = Arc::new(AtomicBool::new(false));
    let rx = llm.generate(messages, cancel).ok()?;
    let mut full_text = String::new();
    loop {
        match rx.recv() {
            Ok(GenChunk::Text(t)) => full_text.push_str(&t),
            Ok(GenChunk::Done { full_text: ft }) => {
                full_text = ft;
                break;
            }
            Ok(GenChunk::Error(_)) | Err(_) => break,
        }
    }
    let title = full_text
        .trim()
        .trim_matches('"')
        .trim_matches('*')
        .to_string();
    if title.is_empty() || title.len() > 100 {
        None
    } else {
        Some(title)
    }
}

/// Generate initial learning path (10 lessons) via LLM and persist to DB.
fn generate_initial_lessons(
    llm: &LlmState,
    db: &Db,
    app: &tauri::AppHandle,
    target_language: &str,
    native_language: &str,
    level: &str,
    goals: &[&str],
) -> Result<(), String> {
    println!("[parla] generating initial learning path...");
    let messages = build_lesson_generation_prompt(target_language, native_language, level, goals);
    let cancel = Arc::new(AtomicBool::new(false));
    let rx = llm.generate(messages, cancel)?;

    let mut full_text = String::new();
    loop {
        match rx.recv() {
            Ok(GenChunk::Text(t)) => full_text.push_str(&t),
            Ok(GenChunk::Done { full_text: ft }) => {
                full_text = ft;
                break;
            }
            Ok(GenChunk::Error(e)) => return Err(e),
            Err(_) => break,
        }
    }

    // Extract JSON array from response (may have surrounding text).
    let json_start = full_text.find('[').ok_or("no JSON array in LLM response")?;
    let json_end = full_text.rfind(']').ok_or("no closing ] in LLM response")? + 1;
    let json_str = &full_text[json_start..json_end];

    let lessons: Vec<serde_json::Value> =
        serde_json::from_str(json_str).map_err(|e| format!("parse lessons JSON: {e}"))?;

    let mut new_lessons = Vec::new();
    for (i, lesson) in lessons.iter().enumerate() {
        new_lessons.push(db::NewLesson {
            sequence_order: (i + 1) as i32,
            cefr_level: lesson["cefr_level"]
                .as_str()
                .unwrap_or(level)
                .to_string(),
            topic: lesson["topic"]
                .as_str()
                .unwrap_or("general")
                .to_string(),
            title: lesson["title"]
                .as_str()
                .unwrap_or("Untitled Lesson")
                .to_string(),
            description: lesson["description"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            scenario: lesson["scenario"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            objectives_json: serde_json::to_string(&lesson["objectives"]).unwrap_or("[]".into()),
            target_vocabulary_json: serde_json::to_string(&lesson["target_vocabulary"])
                .unwrap_or("[]".into()),
            target_grammar_json: serde_json::to_string(&lesson["target_grammar"])
                .unwrap_or("[]".into()),
        });
    }

    db.insert_lessons(&new_lessons)?;
    println!("[parla] generated and stored {} lessons", new_lessons.len());

    // Emit lessons to frontend.
    emit_lessons(db, app);

    Ok(())
}

/// Emit the recent conversations list to the frontend (sidebar).
fn emit_recent_conversations(db: &Db, app: &tauri::AppHandle) {
    if let Ok(convs) = db.get_recent_conversations(20) {
        let list: Vec<serde_json::Value> = convs
            .into_iter()
            .map(|r| {
                let title = r
                    .topic
                    .unwrap_or_else(|| format!("{} conversation", r.mode));
                serde_json::json!({ "id": r.id.to_string(), "title": title })
            })
            .collect();
        let _ = app.emit("recent-conversations", list);
    }
}

/// Emit the current lesson list to the frontend.
fn emit_lessons(db: &Db, app: &tauri::AppHandle) {
    if let Ok(lessons) = db.get_lessons() {
        let list: Vec<serde_json::Value> = lessons
            .into_iter()
            .map(|l| {
                serde_json::json!({
                    "id": l.id,
                    "sequenceOrder": l.sequence_order,
                    "title": l.title,
                    "description": l.description.unwrap_or_default(),
                    "status": l.status,
                    "topic": l.topic,
                    "cefrLevel": l.cefr_level,
                    "successRate": l.success_rate,
                })
            })
            .collect();
        let _ = app.emit("lessons-updated", list);
    }
}

/// Run the streaming LLM generation loop: feed chunks to the parser,
/// synthesize and play sentences as they form, and return the final result.
fn run_streaming_turn(
    llm: &LlmState,
    messages: Vec<ChatMessage>,
    cancel: Arc<AtomicBool>,
    audio: &AudioState,
    app: &tauri::AppHandle,
    tts_lang: &str,
) -> Result<ConversationTurnResult, String> {
    let chunk_rx = llm.generate(messages, cancel.clone())?;
    let mut parser = StreamingJsonParser::new();
    let mut full_text = String::new();

    loop {
        let chunk = match chunk_rx.recv() {
            Ok(c) => c,
            Err(_) => break,
        };
        match chunk {
            GenChunk::Text(t) => {
                full_text.push_str(&t);
                parser.push(&t);
                speak_parsed_sentences(&mut parser, &cancel, audio, app, tts_lang);
            }
            GenChunk::Done { full_text: ft } => {
                full_text = ft;
                break;
            }
            GenChunk::Error(e) => return Err(e),
        }
    }

    // Flush any tail sentence after the stream ends.
    speak_parsed_sentences(&mut parser, &cancel, audio, app, tts_lang);

    // Final structured parse.
    let parsed = ParsedTutorResponse::from_streamed(&full_text);
    let (tutor_target, tutor_native, structured_err) = match &parsed {
        Ok(p) => (
            p.tutor_message.target_lang.clone(),
            p.tutor_message.native_lang.clone(),
            None,
        ),
        Err(e) => {
            eprintln!("[llm] final JSON parse failed: {e}");
            (parser.captured().to_string(), String::new(), Some(e.clone()))
        }
    };

    Ok(ConversationTurnResult {
        raw: full_text,
        tutor_target,
        tutor_native,
        parsed: parsed.ok(),
        parse_error: structured_err,
    })
}

/// Speak and emit any complete sentences from the streaming parser.
fn speak_parsed_sentences(
    parser: &mut StreamingJsonParser,
    cancel: &AtomicBool,
    audio: &AudioState,
    app: &tauri::AppHandle,
    tts_lang: &str,
) {
    for sentence in parser.take_sentences() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let lang = detect_language_from_text(&sentence).unwrap_or(tts_lang);
        if let Ok(output) = audio.synthesize(&sentence, lang) {
            if !output.samples.is_empty() {
                let _ = audio.play_audio(output.samples, output.sample_rate);
            }
        }
        let _ = app.emit("tutor-sentence", &sentence);
    }
}

/// Persist one conversation turn to SQLite and emit frontend events.
/// Called in a background task so it never blocks the voice loop.
fn persist_turn(
    db: &Db,
    history: &ConversationHistory,
    student_text: &str,
    result: &ConversationTurnResult,
    app: &tauri::AppHandle,
    llm: &LlmState,
) -> Result<(), String> {
    // Build correction input.
    let correction = result
        .parsed
        .as_ref()
        .and_then(|p| p.correction.as_ref())
        .map(|c| db::CorrectionInput {
            original: c.original.clone(),
            corrected: c.corrected.clone(),
            explanation: c.explanation.clone(),
        });

    // Build vocabulary list.
    let vocabulary: Vec<db::NewVocabulary> = result
        .parsed
        .as_ref()
        .map(|p| {
            p.new_vocabulary
                .iter()
                .map(|v| db::NewVocabulary {
                    target_text: v.target_text.clone(),
                    native_text: v.native_text.clone(),
                    pronunciation: v.pronunciation.clone(),
                    part_of_speech: v.part_of_speech.clone(),
                    topic: "conversation".into(),
                    example_target: v.example_target.clone(),
                    example_native: v.example_native.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    // Update in-memory counters.
    history.increment_messages(); // student
    history.increment_messages(); // tutor
    if correction.is_some() {
        history.increment_errors();
    }
    let (msg_count, err_count) = history.counts();

    // Persist everything in a single transaction.
    let output = db.persist_turn(&db::PersistTurnInput {
        conversation_id: history.get_conversation_id(),
        student_text: student_text.to_string(),
        tutor_target: result.tutor_target.clone(),
        tutor_native: if result.tutor_native.is_empty() {
            None
        } else {
            Some(result.tutor_native.clone())
        },
        correction,
        vocabulary,
        message_count: msg_count,
        error_count: err_count,
    })?;

    if output.is_new {
        history.set_conversation_id(output.conversation_id);
    }

    // Emit frontend events (reads happen outside the transaction).
    if result.parsed.is_some() {
        let due = db.flashcards_due_count().unwrap_or(0);
        let _ = app.emit("flashcards-due-count", due);

        let recent = db.recent_vocabulary(10).unwrap_or_default();
        let vocab_list: Vec<serde_json::Value> = recent
            .iter()
            .map(|v| {
                serde_json::json!({
                    "target": v.target_text,
                    "native": v.native_text,
                    "strength": match v.status.as_str() {
                        "mature" => 4,
                        "review" => 3,
                        "learning" => 2,
                        "new" => 1,
                        _ => 0,
                    }
                })
            })
            .collect();
        let _ = app.emit("recent-vocabulary", vocab_list);
    }

    // Generate a title for new conversations (first turn only).
    if output.is_new {
        let conv_id = output.conversation_id;
        let tutor_text = if result.tutor_native.is_empty() {
            &result.tutor_target
        } else {
            &result.tutor_native
        };
        match generate_conversation_title(llm, student_text, tutor_text) {
            Some(title) => {
                let _ = db.update_conversation_topic(conv_id, &title);
            }
            None => {
                let fallback = if student_text.len() > 40 {
                    let end = student_text[..40].rfind(' ').unwrap_or(40);
                    format!("{}…", &student_text[..end])
                } else {
                    student_text.to_string()
                };
                let _ = db.update_conversation_topic(conv_id, &fallback);
            }
        }
    }

    emit_recent_conversations(db, app);

    Ok(())
}

fn make_recording_result(result: &audio::pipeline::RecordingResult) -> StopRecordingResult {
    let duration_ms = (result.samples.len() as f64 / 16000.0 * 1000.0) as u64;
    StopRecordingResult {
        duration_ms,
        sample_count: result.samples.len(),
        speech_segments: result
            .speech_segments
            .iter()
            .map(|s| SpeechSegmentResult {
                start_ms: (s.start_sample as f64 / 16.0) as u64,
                end_ms: (s.end_sample as f64 / 16.0) as u64,
            })
            .collect(),
    }
}

// ── Response types ─────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct SpeechSegmentResult {
    start_ms: u64,
    end_ms: u64,
}

#[derive(serde::Serialize)]
struct StopRecordingResult {
    duration_ms: u64,
    sample_count: usize,
    speech_segments: Vec<SpeechSegmentResult>,
}

#[derive(serde::Serialize)]
struct TranscriptionResult {
    duration_ms: u64,
    sample_count: usize,
    speech_segments: Vec<SpeechSegmentResult>,
    transcription: String,
    /// English translation (empty if the target language is already English).
    translation: String,
}

#[derive(serde::Serialize)]
struct AudioStatusResult {
    is_recording: bool,
    is_playing: bool,
    vad_active: bool,
    stt_ready: bool,
    tts_ready: bool,
    speech_detected: bool,
}

#[derive(serde::Serialize)]
struct ModelStatus {
    vad: bool,
    stt: bool,
    llm: bool,
    models_dir: String,
}

#[derive(serde::Serialize)]
struct LlmStatusResult {
    loaded: bool,
}

#[derive(Clone, serde::Serialize)]
struct ProfileResult {
    native_language: String,
    target_language: String,
    cefr_level: String,
    goals: Vec<String>,
}

#[derive(Clone, serde::Serialize)]
struct RecentConversationResult {
    id: String,
    title: String,
}

#[derive(Clone, serde::Serialize)]
struct LoadedMessage {
    role: String,
    content: String,
    translation: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FlashcardResult {
    id: i64,
    word: String,
    meaning: String,
    pronunciation: String,
    example_target: String,
    example_native: String,
    source: String,
    status: String,
    next_review: String,
    dots: Vec<bool>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LessonResult {
    id: i64,
    sequence_order: i32,
    title: String,
    description: String,
    status: String,
    topic: String,
    cefr_level: String,
    success_rate: Option<f64>,
}

#[derive(Clone, serde::Serialize)]
struct ConversationTurnResult {
    /// The raw text the LLM emitted (full JSON).
    raw: String,
    /// The tutor's spoken line (target language) — unescaped from raw JSON.
    tutor_target: String,
    /// English translation — empty if parse failed.
    tutor_native: String,
    /// Fully parsed structured response, or None if parse failed.
    parsed: Option<ParsedTutorResponse>,
    /// Parse error message, if any.
    parse_error: Option<String>,
}

struct ConversationHistoryInner {
    messages: Vec<ChatMessage>,
    system_prompt: String,
    /// Current conversation's DB id (set on first turn, cleared on reset).
    conversation_id: Option<i64>,
    /// Running message count for the current conversation.
    message_count: i32,
    /// Running error count for the current conversation.
    error_count: i32,
}

#[derive(Clone)]
struct ConversationHistory(Arc<Mutex<ConversationHistoryInner>>);

impl ConversationHistory {
    fn new() -> Self {
        let default_prompt = build_system_prompt(
            "Spanish",
            "Learner",
            "English",
            "A1 (Beginner)",
            &["Conversation"],
            None,
        );
        Self(Arc::new(Mutex::new(ConversationHistoryInner {
            messages: Vec::new(),
            system_prompt: default_prompt,
            conversation_id: None,
            message_count: 0,
            error_count: 0,
        })))
    }

    fn set_system_prompt(&self, prompt: String) {
        self.0.lock().unwrap().system_prompt = prompt;
    }

    fn clear(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.messages.clear();
        inner.conversation_id = None;
        inner.message_count = 0;
        inner.error_count = 0;
    }

    fn push(&self, msg: ChatMessage) {
        self.0.lock().unwrap().messages.push(msg);
    }

    fn get_conversation_id(&self) -> Option<i64> {
        self.0.lock().unwrap().conversation_id
    }

    fn set_conversation_id(&self, id: i64) {
        self.0.lock().unwrap().conversation_id = Some(id);
    }

    fn increment_messages(&self) -> i32 {
        let mut inner = self.0.lock().unwrap();
        inner.message_count += 1;
        inner.message_count
    }

    fn increment_errors(&self) -> i32 {
        let mut inner = self.0.lock().unwrap();
        inner.error_count += 1;
        inner.error_count
    }

    fn counts(&self) -> (i32, i32) {
        let inner = self.0.lock().unwrap();
        (inner.message_count, inner.error_count)
    }

    /// Return the full message list prefixed with the system prompt.
    fn messages_with_system(&self) -> Vec<ChatMessage> {
        let inner = self.0.lock().unwrap();
        let mut out = Vec::with_capacity(inner.messages.len() + 1);
        out.push(ChatMessage::system(inner.system_prompt.clone()));
        out.extend(inner.messages.iter().cloned());
        out
    }
}

#[derive(Clone)]
struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

// ── App entry ──────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AudioState::new())
        .manage(LlmState::new())
        .manage(ConversationHistory::new())
        .manage(CancelFlag::new())
        .manage(TargetLanguage::new("es")) // default until profile loads
        .setup(|app| {
            let handle = app.handle().clone();
            let state: AudioState = app.state::<AudioState>().inner().clone();
            let llm_state: LlmState = app.state::<LlmState>().inner().clone();

            // Open SQLite database.
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {e}"))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| format!("failed to create data dir: {e}"))?;
            let db_path = data_dir.join("parla.db");
            println!("[parla] opening database at {}", db_path.display());
            let db =
                Db::open(&db_path).map_err(|e| Box::<dyn std::error::Error>::from(e))?;
            app.manage(db.clone());

            // If a profile exists, set the system prompt and target language from it.
            if let Ok(Some(profile)) = db.get_profile() {
                let goals: Vec<String> =
                    serde_json::from_str(&profile.goals_json).unwrap_or_default();
                let goal_refs: Vec<&str> = goals.iter().map(|s| s.as_str()).collect();
                let prompt = build_system_prompt(
                    &profile.target_language,
                    "Learner",
                    &profile.native_language,
                    &profile.cefr_level,
                    &goal_refs,
                    None,
                );
                let history: tauri::State<ConversationHistory> = app.state();
                history.set_system_prompt(prompt);
                let tl: tauri::State<TargetLanguage> = app.state();
                tl.set(language_name_to_code(&profile.target_language));
                println!("[parla] loaded profile: {} -> {}", profile.native_language, profile.target_language);
            }

            // Init audio devices immediately (fast, needed for everything)
            if let Err(e) = state.init() {
                eprintln!("[setup] audio init failed: {e}");
            }

            // Load all models in a background thread — never blocks the UI
            std::thread::spawn(move || {
                println!("[parla] background model loading started");

                let mdir = match models_dir(&handle) {
                    Ok(d) => d,
                    Err(e) => {
                        println!("[parla] {e}");
                        return;
                    }
                };
                let data_dir = mdir.parent().unwrap().to_path_buf();

                // VAD (~2 MB, loads in <1s)
                let vad_path = mdir.join("silero_vad.onnx");
                if vad_path.exists() {
                    println!("[parla] loading VAD...");
                    match state.init_vad(vad_path) {
                        Ok(()) => println!("[parla] VAD loaded"),
                        Err(e) => println!("[parla] VAD failed: {e}"),
                    }
                } else {
                    println!("[parla] VAD model not found, skipping");
                }

                // TTS (Piper ONNX voices, macOS say fallback for unsupported languages)
                println!("[parla] loading TTS...");
                let tts_dir = data_dir.join("tts");
                match state.init_tts(tts_dir) {
                    Ok(()) => println!("[parla] TTS loaded"),
                    Err(e) => println!("[parla] TTS failed: {e}"),
                }

                // STT
                println!("[parla] loading STT...");
                match find_whisper_model(&mdir) {
                    Ok(stt_path) => {
                        println!("[parla] found whisper model: {}", stt_path.display());
                        match state.init_stt(stt_path) {
                            Ok(()) => println!("[parla] STT loaded"),
                            Err(e) => println!("[parla] STT failed: {e}"),
                        }
                    }
                    Err(e) => println!("[parla] STT model not found: {e}"),
                }

                // LLM — heaviest, loaded last (can take 10-30s for a 26B GGUF)
                println!("[parla] loading LLM...");
                match find_gemma_model(&mdir) {
                    Ok(llm_path) => {
                        println!("[parla] found gemma model: {}", llm_path.display());
                        match llm_state.load_model(llm_path) {
                            Ok(()) => println!("[parla] LLM loaded"),
                            Err(e) => println!("[parla] LLM failed: {e}"),
                        }
                    }
                    Err(e) => println!("[parla] LLM model not found: {e}"),
                }

                println!("[parla] all models loaded");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            stop_recording_and_transcribe,
            loopback_test,
            speak_text,
            stop_playback,
            audio_status,
            check_models,
            llm_status,
            get_profile,
            create_profile,
            get_recent_conversations,
            rename_conversation,
            delete_conversation,
            get_flashcards,
            get_lessons,
            start_lesson,
            review_flashcard,
            load_conversation,
            begin_tutor_lesson,
            conversation_turn,
            reset_conversation,
            cancel_generation,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Shut down the llama-server subprocess before the process exits.
                // This is the primary cleanup path. LlmState::Drop is the backup.
                if let Some(llm) = app.try_state::<LlmState>() {
                    eprintln!("[parla] RunEvent::Exit — shutting down llama-server");
                    llm.shutdown();
                }
            }
        });
}
