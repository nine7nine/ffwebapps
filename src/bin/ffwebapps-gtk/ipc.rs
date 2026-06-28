//! Live IPC client for a running web app's runtime socket
//! (`$XDG_RUNTIME_DIR/ffwebapps-<ULID>.sock`).
//!
//! Protocol (newline-delimited): on connect the runtime sends `hello v1 <pid>`,
//! `unread <n>`, then `state hidden=.. muted=.. dnd=.. suspend=.. autostart=..`,
//! and broadcasts `unread`/`state` on change. Clients send verbs (`show`,
//! `hide`, `quit`, …). We connect as `launcher` (not `tray`) so we monitor and
//! send verbs without affecting the runtime's close-to-tray behaviour.

use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use gtk::glib;
use ulid::Ulid;

/// The boolean state flags from a `state …` line.
#[derive(Clone, Copy, Default)]
pub struct LiveFlags {
    pub hidden: bool,
    pub muted: bool,
    pub dnd: bool,
    pub suspend: bool,
    pub autostart: bool,
}

/// An event from the runtime, delivered on the GTK main loop.
pub enum LiveEvent {
    Hello(i64),
    Unread(u32),
    State(LiveFlags),
    Other,
    Disconnected,
}

/// A live connection: holds the write half so we can send verbs and tear down.
pub struct LiveConn {
    writer: UnixStream,
}

impl LiveConn {
    /// Send a verb (e.g. `show`, `hide`, `mute-toggle`). Errors are ignored —
    /// the runtime may have just exited, which the reader will report.
    pub fn send(&self, verb: &str) {
        let _ = (&self.writer).write_all(format!("{verb}\n").as_bytes());
    }

    /// Close the connection; this unblocks and ends the reader thread.
    pub fn close(&self) {
        let _ = self.writer.shutdown(Shutdown::Both);
    }
}

fn socket_path(id: Ulid) -> Option<PathBuf> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    Some(PathBuf::from(runtime_dir).join(format!("ffwebapps-{id}.sock")))
}

/// Is the web app running? True iff its socket accepts a connection.
pub fn is_running(id: Ulid) -> bool {
    socket_path(id).is_some_and(|path| UnixStream::connect(path).is_ok())
}

/// Connect to a running web app and stream its events to `on_event` (called on
/// the GTK main loop). Returns `None` if the app isn't running. The reader runs
/// on a worker thread; dropping/closing the returned `LiveConn` ends it.
pub fn connect_live(id: Ulid, on_event: impl Fn(LiveEvent) + 'static) -> Option<LiveConn> {
    let path = socket_path(id)?;
    let writer = UnixStream::connect(path).ok()?;
    let reader_stream = writer.try_clone().ok()?;

    // Identify as a launcher (non-tray); the runtime greets us regardless.
    let _ = (&writer).write_all(b"hello v1 launcher\n");

    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        let reader = BufReader::new(reader_stream);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if tx.send_blocking(parse(&line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send_blocking(LiveEvent::Disconnected);
    });

    glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            on_event(event);
        }
    });

    Some(LiveConn { writer })
}

fn parse(line: &str) -> LiveEvent {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("hello v1 ") {
        return LiveEvent::Hello(rest.trim().parse().unwrap_or(0));
    }
    if let Some(rest) = line.strip_prefix("unread ") {
        return LiveEvent::Unread(rest.trim().parse().unwrap_or(0));
    }
    if let Some(rest) = line.strip_prefix("state ") {
        return LiveEvent::State(parse_flags(rest));
    }
    LiveEvent::Other
}

fn parse_flags(rest: &str) -> LiveFlags {
    let mut flags = LiveFlags::default();
    for token in rest.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            let on = value == "1";
            match key {
                "hidden" => flags.hidden = on,
                "muted" => flags.muted = on,
                "dnd" => flags.dnd = on,
                "suspend" => flags.suspend = on,
                "autostart" => flags.autostart = on,
                _ => {}
            }
        }
    }
    flags
}
