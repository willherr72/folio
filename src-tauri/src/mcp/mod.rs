//! Local stdio MCP. Only launch-authorized snapshots are addressable by tools.
mod config;
mod previews;
mod protocol;
mod session;
mod worker;
use config::Config;
use serde_json::Value;
use session::Session;
use std::{
    ffi::OsString,
    io::{self, BufRead, Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

const MAX_FRAME: usize = 512 * 1024;
enum Frame {
    Message(Vec<u8>),
    Oversize,
    Eof,
}
fn read_frames(reader: impl Read, tx: mpsc::SyncSender<Frame>) {
    let mut reader = io::BufReader::new(reader);
    loop {
        let mut line = Vec::new();
        let mut oversized = false;
        loop {
            let available = match reader.fill_buf() {
                Ok(b) => b,
                Err(_) => {
                    let _ = tx.send(Frame::Eof);
                    return;
                }
            };
            if available.is_empty() {
                if !line.is_empty() || oversized {
                    let _ = tx.send(if oversized {
                        Frame::Oversize
                    } else {
                        Frame::Message(line)
                    });
                }
                let _ = tx.send(Frame::Eof);
                return;
            }
            let end = available.iter().position(|b| *b == b'\n');
            let take = end.map(|i| i + 1).unwrap_or(available.len());
            if !oversized {
                if line.len() + take > MAX_FRAME {
                    oversized = true;
                    line.clear();
                } else {
                    line.extend_from_slice(&available[..take]);
                }
            }
            reader.consume(take);
            if end.is_some() {
                break;
            }
        }
        if tx
            .send(if oversized {
                Frame::Oversize
            } else {
                Frame::Message(line)
            })
            .is_err()
        {
            return;
        }
    }
}
// Stdout can block indefinitely when a client stops draining its pipe. Never
// perform that write (or a blocking channel send/join) on the supervisor thread.
// A saturated/broken output queue closes the session after owned-worker cleanup.
struct Output {
    tx: mpsc::SyncSender<Vec<u8>>,
    failed: Arc<AtomicBool>,
}
impl Output {
    fn start() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(8);
        let failed = Arc::new(AtomicBool::new(false));
        let writer_failed = failed.clone();
        std::thread::spawn(move || {
            let mut stdout = io::stdout().lock();
            while let Ok(bytes) = rx.recv() {
                if stdout
                    .write_all(&bytes)
                    .and_then(|_| stdout.flush())
                    .is_err()
                {
                    writer_failed.store(true, Ordering::Release);
                    break;
                }
            }
        });
        Self { tx, failed }
    }
    fn send(&self, value: Value) -> Result<(), ()> {
        if self.failed.load(Ordering::Acquire) {
            return Err(());
        }
        let mut bytes = serde_json::to_vec(&value).map_err(|_| ())?;
        if bytes.len() >= 16 * 1024 * 1024 {
            return Err(());
        }
        bytes.push(b'\n');
        self.tx.try_send(bytes).map_err(|_| ())
    }
}
pub fn run(args: Vec<OsString>) -> i32 {
    if args == [OsString::from("--help")] {
        println!("Folio MCP — local stdio PDF tools\nfolio-cli mcp --input PDF [--input PDF ...] --out-dir EXISTING_DIR [--engine PATH] [--font FONT ...]\nLaunch inputs are immutable snapshots (1–64 PDFs, 512 MiB total). Tool calls use document IDs. Optional fonts are immutable launch-authorized inputs: at most16 files,16 MiB each,64 MiB total. Use folio_list_fonts and paragraph fontOverrides.\nExports create new child folders. Inspect text and preview replacements. Apply for further edits, save exact PDF copies, or discard. Close unused session documents to free capacity. Password entry and unsaved desktop tabs are not supported.\nSupports MCP 2026-07-28 and legacy initialization. stdout is reserved for JSON-RPC.");
        return 0;
    }
    let config = match Config::load(args) {
        Ok(c) => c,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    let (tx, rx) = mpsc::sync_channel(32);
    std::thread::spawn(move || read_frames(io::stdin().lock(), tx));
    let output = Output::start();
    let mut session = Session::new(config);
    loop {
        session.prune();
        if output.failed.load(Ordering::Acquire) {
            return 1;
        }
        let message = match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Frame::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => return 0,
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Ok(Frame::Oversize) => Some(protocol::error(
                Value::Null,
                -32600,
                "Request exceeds the 512 KiB limit.",
            )),
            Ok(Frame::Message(bytes)) => match serde_json::from_slice::<Value>(&bytes) {
                Err(_) => Some(protocol::error(Value::Null, -32700, "Invalid JSON.")),
                Ok(value) => session.dispatch(value),
            },
        };
        if let Some(value) = message {
            if output.send(value).is_err() {
                return 1;
            }
        }
        if let Some(reply) = session.poll() {
            if output.send(reply).is_err() {
                return 1;
            }
        }
    }
}
