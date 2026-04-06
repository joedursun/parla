//! LLM inference via a managed `llama-server` subprocess.
//!
//! Rationale: as of Gemma 4's release (2026-04-03), the Rust `llama-cpp-2`
//! bindings on crates.io and upstream `main` still vendor a pre-Gemma-4
//! llama.cpp and fail with `unknown model architecture: 'gemma4'`. Rather
//! than fork the sys crate to bump its submodule, we delegate to the user's
//! `llama-server` binary (Homebrew ships current builds) and talk to it
//! over its OpenAI-compatible HTTP API.

pub mod parser;
pub mod prompt;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub use prompt::ChatMessage;

use crate::llm::prompt::ChatRole;

const N_CTX: u32 = 32_768;
const HEALTH_TIMEOUT_SECS: u64 = 120;
const MAX_GEN_TOKENS: u32 = 1536;

// ── Public handle ──────────────────────────────────────────────────────────

enum LlmCommand {
    Load {
        model_path: PathBuf,
        reply: std_mpsc::Sender<Result<(), String>>,
    },
    Generate {
        messages: Vec<ChatMessage>,
        chunk_tx: std_mpsc::Sender<GenChunk>,
        cancel: Arc<AtomicBool>,
    },
    Status {
        reply: std_mpsc::Sender<LlmStatus>,
    },
}

#[derive(Debug, Clone)]
pub enum GenChunk {
    Text(String),
    Done { full_text: String },
    Error(String),
}

#[derive(Debug, Clone)]
pub struct LlmStatus {
    pub loaded: bool,
}

/// Shared state accessible from both the llm thread and the Drop guard.
/// This lets us forcibly kill the subprocess even when the llm thread is
/// blocked in `wait_until_healthy` during a slow model load.
struct LlmShared {
    /// The live llama-server child, if any.
    server: Mutex<Option<LlamaServer>>,
    /// Set when the app is shutting down. Blocking operations in the llm
    /// thread poll this and bail early.
    shutting_down: AtomicBool,
}

/// Cloneable handle to the LLM. All clones talk to the same background thread.
/// When the last clone drops, the llama-server subprocess is killed.
#[derive(Clone)]
pub struct LlmState {
    cmd_tx: std_mpsc::Sender<LlmCommand>,
    shared: Arc<LlmShared>,
}

impl LlmState {
    pub fn new() -> Self {
        let shared = Arc::new(LlmShared {
            server: Mutex::new(None),
            shutting_down: AtomicBool::new(false),
        });

        let (cmd_tx, cmd_rx) = std_mpsc::channel::<LlmCommand>();
        let thread_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("parla-llm".into())
            .spawn(move || llm_thread_main(cmd_rx, thread_shared))
            .expect("failed to spawn llm thread");

        Self { cmd_tx, shared }
    }

    pub fn load_model(&self, model_path: PathBuf) -> Result<(), String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(LlmCommand::Load {
                model_path,
                reply: reply_tx,
            })
            .map_err(|_| "llm thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "llm thread gone".to_string())?
    }

    pub fn generate(
        &self,
        messages: Vec<ChatMessage>,
        cancel: Arc<AtomicBool>,
    ) -> Result<std_mpsc::Receiver<GenChunk>, String> {
        let (chunk_tx, chunk_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(LlmCommand::Generate {
                messages,
                chunk_tx,
                cancel,
            })
            .map_err(|_| "llm thread gone".to_string())?;
        Ok(chunk_rx)
    }

    pub fn status(&self) -> LlmStatus {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        if self
            .cmd_tx
            .send(LlmCommand::Status { reply: reply_tx })
            .is_err()
        {
            return LlmStatus { loaded: false };
        }
        reply_rx.recv().unwrap_or(LlmStatus { loaded: false })
    }

    /// Shut down the llama-server subprocess. Signals the llm thread, then
    /// forcibly kills the child if it's still alive. Safe to call multiple times.
    pub fn shutdown(&self) {
        // Signal first — lets the llm thread exit its event loop and lets
        // any blocking wait_until_healthy bail early.
        self.shared.shutting_down.store(true, Ordering::SeqCst);

        // Drop the sender so the llm thread's recv() returns Err and the
        // thread exits. We can't literally drop self.cmd_tx here (we're &self),
        // but setting the flag above is sufficient.

        // Forcibly kill the subprocess regardless of what the llm thread is doing.
        if let Ok(mut guard) = self.shared.server.lock() {
            if let Some(mut server) = guard.take() {
                server.kill();
            }
        }
    }
}

impl Drop for LlmState {
    fn drop(&mut self) {
        // If this is the last Arc ref (us + the llm thread = 2 initially;
        // Tauri clones for each managed-state access, but those are short-lived),
        // we force-kill the server so the child process never outlives the app.
        //
        // strong_count == 1 means we're the last holder; == 2 means only us +
        // the llm thread remain. In both cases, shut down.
        if Arc::strong_count(&self.shared) <= 2 {
            self.shutdown();
        }
    }
}

// ── Background thread ──────────────────────────────────────────────────────

fn llm_thread_main(cmd_rx: std_mpsc::Receiver<LlmCommand>, shared: Arc<LlmShared>) {
    while let Ok(cmd) = cmd_rx.recv() {
        if shared.shutting_down.load(Ordering::SeqCst) {
            break;
        }

        match cmd {
            LlmCommand::Load { model_path, reply } => {
                // Stop any existing server before loading a new model.
                {
                    let mut guard = shared.server.lock().unwrap();
                    if let Some(mut old) = guard.take() {
                        old.kill();
                    }
                }

                let result =
                    LlamaServer::spawn(&model_path, &shared.shutting_down).map(|srv| {
                        *shared.server.lock().unwrap() = Some(srv);
                    });
                let _ = reply.send(result);
            }
            LlmCommand::Generate {
                messages,
                chunk_tx,
                cancel,
            } => {
                let guard = shared.server.lock().unwrap();
                let res = if let Some(ref server) = *guard {
                    generate_inner(server, &messages, &chunk_tx, cancel)
                } else {
                    Err("LLM not loaded".into())
                };
                drop(guard);

                match res {
                    Ok(full) => {
                        let _ = chunk_tx.send(GenChunk::Done { full_text: full });
                    }
                    Err(e) => {
                        let _ = chunk_tx.send(GenChunk::Error(e));
                    }
                }
            }
            LlmCommand::Status { reply } => {
                let loaded = shared.server.lock().unwrap().is_some();
                let _ = reply.send(LlmStatus { loaded });
            }
        }
    }

    // Thread is exiting — kill any remaining server.
    let mut guard = shared.server.lock().unwrap();
    if let Some(mut server) = guard.take() {
        server.kill();
    }
}

// ── llama-server subprocess manager ────────────────────────────────────────

struct LlamaServer {
    child: Child,
    port: u16,
}

impl LlamaServer {
    fn spawn(model_path: &Path, shutdown: &AtomicBool) -> Result<Self, String> {
        let binary = which_llama_server()?;
        let port = pick_free_port()?;

        eprintln!(
            "[llm] spawning {} on port {} with model {}",
            binary.display(),
            port,
            model_path.display()
        );

        let mut cmd = Command::new(&binary);
        cmd.arg("-m")
            .arg(model_path)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-c")
            .arg(N_CTX.to_string())
            .arg("-ngl")
            .arg("999")
            .arg("-fa")
            .arg("on")
            .arg("--jinja")
            .arg("--reasoning")
            .arg("off")
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn llama-server: {e}"))?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    eprintln!("[llama-server] {line}");
                }
            });
        }

        let mut server = Self { child, port };

        match server.wait_until_healthy(shutdown) {
            Ok(()) => {
                eprintln!("[llm] llama-server ready on port {}", port);
                Ok(server)
            }
            Err(e) => {
                // If health check failed (timeout or shutdown), kill the child
                // so we don't leak a process.
                server.kill();
                Err(e)
            }
        }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn wait_until_healthy(&self, shutdown: &AtomicBool) -> Result<(), String> {
        let start = Instant::now();
        let url = format!("{}/health", self.base_url());
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(2))
            .build();

        loop {
            if shutdown.load(Ordering::SeqCst) {
                return Err("shutdown requested during model load".into());
            }
            if start.elapsed() > Duration::from_secs(HEALTH_TIMEOUT_SECS) {
                return Err(format!(
                    "llama-server did not become healthy within {}s",
                    HEALTH_TIMEOUT_SECS
                ));
            }
            match agent.get(&url).call() {
                Ok(resp) if resp.status() == 200 => return Ok(()),
                _ => thread::sleep(Duration::from_millis(300)),
            }
        }
    }

    fn kill(&mut self) {
        eprintln!("[llm] stopping llama-server (pid {})", self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait(); // reap to avoid zombie
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn which_llama_server() -> Result<PathBuf, String> {
    if let Ok(output) = Command::new("which").arg("llama-server").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }
    for candidate in &[
        "/opt/homebrew/bin/llama-server",
        "/usr/local/bin/llama-server",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Ok(p);
        }
    }
    Err("llama-server not found. Install with: brew install llama.cpp".into())
}

fn pick_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("failed to bind probe socket: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("failed to read bound port: {e}"))?
        .port();
    drop(listener);
    Ok(port)
}

// ── Streaming generation over SSE ──────────────────────────────────────────

fn generate_inner(
    server: &LlamaServer,
    messages: &[ChatMessage],
    chunk_tx: &std_mpsc::Sender<GenChunk>,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    let url = format!("{}/v1/chat/completions", server.base_url());

    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": match m.role {
                    ChatRole::System => "system",
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                },
                "content": m.content,
            })
        })
        .collect();

    let body = serde_json::json!({
        "model": "parla",
        "messages": msgs,
        "stream": true,
        "max_tokens": MAX_GEN_TOKENS,
        "temperature": 0.4,
        "top_p": 0.95,
        "top_k": 40,
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(300))
        .build();

    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("llama-server request failed: {e}"))?;

    let reader = BufReader::new(resp.into_reader());
    let mut full_text = String::new();

    for line in reader.lines() {
        if cancel.load(Ordering::Relaxed) {
            eprintln!("[llm] generation cancelled");
            break;
        }
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(format!("stream read error: {e}")),
        };
        if line.is_empty() {
            continue;
        }
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        if payload.is_empty() {
            continue;
        }

        let v: serde_json::Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[llm] failed to parse SSE chunk: {e}");
                continue;
            }
        };

        let delta = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"));
        if let Some(content) = delta.and_then(|d| d.get("content")).and_then(|s| s.as_str()) {
            if !content.is_empty() {
                full_text.push_str(content);
                if chunk_tx.send(GenChunk::Text(content.to_string())).is_err() {
                    break;
                }
            }
        }
    }

    Ok(full_text)
}
