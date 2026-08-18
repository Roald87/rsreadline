//! Shared helper for spawning a real interactive bash session inside a pty,
//! used by the integration tests that regression-test bash/readline
//! behavior our unit tests can't observe (see ARCHITECTURE.md).
//!
//! Each test using this lives in its own `tests/*.rs` file so it runs as its
//! own process, keeping `forkpty`'s fork() a single-threaded operation in
//! practice (only async-signal-safe work happens between fork and exec in
//! the child).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use nix::pty::{ForkptyResult, Winsize, forkpty};
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::waitpid;
use nix::unistd::{Pid, execvp, read, write};
use std::ffi::CString;
use std::os::fd::{AsFd, OwnedFd};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct BashSession {
    master: OwnedFd,
    child: Pid,
    #[allow(dead_code)]
    pub fake_home: PathBuf,
}

impl BashSession {
    /// Spawns `bash --norc --noprofile` in a fresh pty with `HOME` pointed
    /// at a scratch directory containing the given `.bash_history` content.
    pub fn spawn(bash_history: &str) -> Self {
        let fake_home = std::env::temp_dir().join(format!(
            "rsreadline-pty-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&fake_home).expect("create fake HOME");
        std::fs::write(fake_home.join(".bash_history"), bash_history).expect("write .bash_history");

        let winsize = Winsize {
            ws_row: 40,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: the child branch below only sets two environment
        // variables and calls execvp — no heap allocation beyond that,
        // minimizing the window for the classic post-fork-in-a-
        // multithreaded-process allocator hazard. Each test using this
        // helper lives in its own process (see module docs).
        match unsafe { forkpty(Some(&winsize), None) }.expect("forkpty") {
            ForkptyResult::Parent { child, master } => Self {
                master,
                child,
                fake_home,
            },
            ForkptyResult::Child => {
                // SAFETY: single-threaded at this point (fork() only keeps
                // the calling thread in the child).
                unsafe {
                    std::env::set_var("HOME", &fake_home);
                    std::env::set_var("TERM", "xterm-256color");
                }
                let bash = CString::new("bash").expect("no NUL");
                let norc = CString::new("--norc").expect("no NUL");
                let noprofile = CString::new("--noprofile").expect("no NUL");
                let _ = execvp(&bash, &[bash.clone(), norc, noprofile]);
                std::process::exit(127);
            }
        }
    }

    pub fn send(&self, bytes: &[u8]) {
        write(self.master.as_fd(), bytes).expect("write to pty master");
    }

    /// Reads whatever output has accumulated, polling until `deadline` has
    /// passed with no new data.
    pub fn drain(&self, quiet_for: Duration) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let start = Instant::now();
            let mut buf = [0u8; 65536];
            match self.poll_read(&mut buf, quiet_for) {
                Some(0) | None => break,
                Some(n) => out.extend_from_slice(&buf[..n]),
            }
            // Keep draining while output keeps arriving; stop once a full
            // quiet_for window passes with nothing new.
            if start.elapsed() >= quiet_for {
                break;
            }
        }
        out
    }

    fn poll_read(&self, buf: &mut [u8], timeout: Duration) -> Option<usize> {
        use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
        let mut fds = [PollFd::new(self.master.as_fd(), PollFlags::POLLIN)];
        let timeout = PollTimeout::from(timeout.as_millis() as u16);
        let n = poll(&mut fds, timeout).ok()?;
        if n == 0 {
            return None;
        }
        match read(self.master.as_fd(), buf) {
            Ok(n) => Some(n),
            Err(_) => Some(0),
        }
    }

    /// Sends `bytes`, waits briefly for bash to react, and returns whatever
    /// it wrote in response.
    pub fn send_and_drain(&self, bytes: &[u8]) -> Vec<u8> {
        self.send(bytes);
        std::thread::sleep(Duration::from_millis(150));
        self.drain(Duration::from_millis(300))
    }
}

impl Drop for BashSession {
    fn drop(&mut self) {
        let _ = kill(self.child, Signal::SIGKILL);
        let _ = waitpid(self.child, None);
        let _ = std::fs::remove_dir_all(&self.fake_home);
    }
}
