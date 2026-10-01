#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

pub const COLS: u16 = 120;
pub const ROWS: u16 = 30;

/// A process running in a test-owned ConPTY, with its screen mirrored by `vt100`.
pub struct Terminal {
    parser: vt100::Parser,
    /// Every byte received so far (OSC reports included).
    pub raw: Vec<u8>,
    rx: Receiver<Vec<u8>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
}

impl Terminal {
    pub fn spawn(cmd: CommandBuilder) -> Self {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: ROWS,
                cols: COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        Self::attach(pair.master, child)
    }

    pub fn attach(master: Box<dyn MasterPty + Send>, child: Box<dyn Child + Send + Sync>) -> Self {
        let mut reader = master.try_clone_reader().expect("reader");
        let writer = master.take_writer().expect("writer");
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            parser: vt100::Parser::new(ROWS, COLS, 0),
            raw: Vec::new(),
            rx,
            writer,
            child,
            _master: master,
        }
    }

    /// Starts the real shell-panel binary with `--no-profile` in `cwd`.
    pub fn shell_panel(cwd: &Path) -> Self {
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_shell-panel"));
        cmd.arg("--no-profile");
        cmd.cwd(cwd);
        // `cargo test` may itself be running inside a shell-panel session.
        cmd.env_remove("SHELL_PANEL_SESSION");
        Self::spawn(cmd)
    }

    fn pump(&mut self, wait: Duration) {
        if let Ok(bytes) = self.rx.recv_timeout(wait) {
            self.parser.process(&bytes);
            self.raw.extend_from_slice(&bytes);
        }
    }

    pub fn wait_until(&mut self, timeout: Duration, pred: impl Fn(&Terminal) -> bool) -> bool {
        let start = Instant::now();
        loop {
            if pred(self) {
                return true;
            }
            if start.elapsed() > timeout {
                return false;
            }
            self.pump(Duration::from_millis(50));
        }
    }

    pub fn wait_for_text(&mut self, text: &str, timeout: Duration) -> bool {
        self.wait_until(timeout, |t| t.screen().contains(text))
    }

    pub fn wait_for_raw(&mut self, needle: &[u8], timeout: Duration) -> bool {
        self.wait_until(timeout, |t| {
            t.raw.windows(needle.len()).any(|w| w == needle)
        })
    }

    pub fn screen(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).expect("write to pty");
        self.writer.flush().expect("flush pty");
    }

    pub fn wait_exit(&mut self, timeout: Duration) -> Option<u32> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status.exit_code());
            }
            self.pump(Duration::from_millis(50));
        }
        None
    }
    /// Keeps this test session out of the user's real PSReadLine history: nothing typed here is
    /// saved, and no inline prediction from the real history is drawn on the line under test.
    /// Call it once the first prompt is on screen.
    pub fn quiet_session(&mut self) {
        // PredictionSource does not exist in PSReadLine 2.0 (Windows PowerShell 5.1): try/catch.
        // The marker is built by concatenation so the echoed command cannot satisfy the wait.
        self.send(
            b"Set-PSReadLineOption -HistorySaveStyle SaveNothing; \
try { Set-PSReadLineOption -PredictionSource None } catch {}; 'QUIET' + 'READY'\r",
        );
        assert!(
            self.wait_for_text("QUIETREADY", Duration::from_secs(15)),
            "quiet_session did not finish: {}",
            self.screen()
        );
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
