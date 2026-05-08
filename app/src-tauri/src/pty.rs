// Embedded-terminal sessions (B5).
//
// Each session owns a pty pair (master + slave) and a child process that
// runs `<runtime> exec -it <container-id> <shell>`. The reader thread
// drains the master in 4 KiB chunks and emits `pty:tick:<id>` events
// with base64-encoded bytes (binary-safe transport over Tauri's JSON
// channel). The writer is held behind a Mutex so frontend keystrokes
// can be flushed without racing the reader.
//
// Sessions are tracked in a process-global registry keyed by uuid. The
// frontend opens one per TerminalModal and is responsible for closing
// it on dismount; we also reap on child exit so a runaway shell doesn't
// pin the registry forever.

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Emitter};

/// Maximum payload length per `pty:tick` event. Smaller chunks keep the
/// UI responsive; larger ones reduce event overhead. 4 KiB matches the
/// default pipe buffer size on macOS and is fine for shell traffic.
const READ_CHUNK: usize = 4096;

/// One running pty session. Held under a Mutex<HashMap> in REGISTRY.
struct Session {
    /// Pty master. Kept alive so child stays attached even if no other
    /// ref is around; dropping this terminates the pair.
    master: Box<dyn MasterPty + Send>,
    /// Writer cloned off the master. Mutex<> so concurrent stdin writes
    /// from the UI don't corrupt each other.
    writer: Mutex<Box<dyn Write + Send>>,
    /// Child handle for explicit kill on close.
    child: Mutex<Box<dyn portable_pty::Child + Send + Sync>>,
}

// Process-global registry. We never expect more than a handful of
// concurrent sessions so a Mutex<HashMap> is fine.
static REGISTRY: Mutex<Option<HashMap<String, Session>>> = Mutex::new(None);

fn registry() -> std::sync::MutexGuard<'static, Option<HashMap<String, Session>>> {
    let mut g = REGISTRY.lock().expect("pty registry mutex");
    if g.is_none() {
        *g = Some(HashMap::new());
    }
    g
}

/// Spawn a new pty + child. Returns the session id. The runtime binary,
/// container id, and an optional shell path are passed in so the
/// caller (commands.rs) controls runtime selection — this module never
/// reads runtime::bin() directly.
pub fn open(
    app: &AppHandle,
    runtime_bin: &str,
    container_id: &str,
    shell: Option<&str>,
    cols: u16,
    rows: u16,
) -> Result<String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("openpty failed")?;

    let mut cmd = CommandBuilder::new(runtime_bin);
    cmd.arg("exec");
    cmd.arg("-it");
    cmd.arg(container_id);
    cmd.arg(shell.unwrap_or("/bin/sh"));
    // Ensure the child sees a sensible TERM. xterm.js advertises xterm-256color.
    cmd.env("TERM", "xterm-256color");

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("spawn pty child failed")?;

    // Hold a reader cloned off the master so the child can be moved
    // into the registry while the reader thread keeps draining.
    let mut reader = pair
        .master
        .try_clone_reader()
        .context("clone pty reader failed")?;
    let writer = pair
        .master
        .take_writer()
        .context("take pty writer failed")?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let event_name = format!("pty:tick:{session_id}");
    let done_event = format!("pty:done:{session_id}");

    {
        let mut g = registry();
        g.as_mut().unwrap().insert(
            session_id.clone(),
            Session {
                master: pair.master,
                writer: Mutex::new(writer),
                child: Mutex::new(child),
            },
        );
    }

    // Drain stdout in a dedicated OS thread (portable-pty's reader is
    // blocking). Each chunk is base64-encoded so we don't have to worry
    // about non-utf8 bytes (xterm.js can decode either utf-8 or raw
    // bytes via the addon-attach path; we keep it simple and let the
    // frontend decode).
    let app2 = app.clone();
    let id_for_thread = session_id.clone();
    thread::spawn(move || {
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF — child has exited
                Ok(n) => {
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                    let _ = app2.emit(&event_name, b64);
                }
                Err(_) => break,
            }
        }
        let _ = app2.emit(&done_event, true);
        // Reap the registry slot so a closed session doesn't linger.
        if let Some(map) = registry().as_mut() {
            map.remove(&id_for_thread);
        }
    });

    Ok(session_id)
}

/// Push bytes (UTF-8 keystrokes / paste payloads) into the pty's stdin.
pub fn write(session_id: &str, data: &str) -> Result<()> {
    let g = registry();
    let map = g
        .as_ref()
        .ok_or_else(|| anyhow!("registry not initialised"))?;
    let s = map
        .get(session_id)
        .ok_or_else(|| anyhow!("unknown pty session {session_id}"))?;
    let mut w = s.writer.lock().map_err(|_| anyhow!("writer poisoned"))?;
    w.write_all(data.as_bytes())
        .with_context(|| format!("write to pty {session_id}"))?;
    w.flush().ok();
    Ok(())
}

/// Resize the pty when the xterm.js fit-addon reports new dimensions.
/// Cheap; safe to call on every browser resize event.
pub fn resize(session_id: &str, cols: u16, rows: u16) -> Result<()> {
    let g = registry();
    let map = g
        .as_ref()
        .ok_or_else(|| anyhow!("registry not initialised"))?;
    let s = map
        .get(session_id)
        .ok_or_else(|| anyhow!("unknown pty session {session_id}"))?;
    s.master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| anyhow!("resize: {e}"))?;
    Ok(())
}

/// Tear a session down — kills the child and drops the pty pair. Idempotent.
pub fn close(session_id: &str) -> Result<()> {
    let mut g = registry();
    let map = g
        .as_mut()
        .ok_or_else(|| anyhow!("registry not initialised"))?;
    if let Some(s) = map.remove(session_id) {
        if let Ok(mut child) = s.child.lock() {
            let _ = child.kill();
        }
    }
    Ok(())
}
