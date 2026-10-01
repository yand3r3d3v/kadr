//! Running ffmpeg: progress from `-progress pipe:1`, the tail of stderr, cancel.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

const LOG_LINES: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub out_time: f64,
    pub fps: f64,
    pub speed: f64,
    pub size: u64,
}

#[derive(Debug)]
pub enum RunEvent {
    Progress(Progress),
    Finished { code: Option<i32>, cancelled: bool },
}

pub struct Handle {
    child: Arc<Mutex<Child>>,
    stdin: Option<ChildStdin>,
    cancelled: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    log: Arc<Mutex<VecDeque<String>>>,
}

impl Handle {
    /// Asks ffmpeg to stop the way a person would, with `q`, and kills it if
    /// it has not left in two seconds.
    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.write_all(b"q\n");
        }
        let (child, done) = (self.child.clone(), self.done.clone());
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            if !done.load(Ordering::SeqCst) {
                let _ = child.lock().unwrap().kill();
            }
        });
    }

    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().iter().cloned().collect()
    }
}

/// The flags kadr adds on top of the command it shows. They change how ffmpeg
/// reports, never what it produces. `-n` makes ffmpeg refuse to overwrite
/// unless the shown command carries `-y`.
pub fn service_flags(argv: &[String]) -> Vec<String> {
    let mut flags = vec!["-hide_banner".to_string(), "-nostats".into()];
    flags.extend(["-progress".to_string(), "pipe:1".into()]);
    if !argv.iter().any(|a| a == "-y") {
        flags.push("-n".into());
    }
    flags
}

/// Folds one `key=value` line of `-progress` output into `p`. Returns true
/// when a block is complete and `p` is worth showing.
pub fn feed(p: &mut Progress, line: &str) -> bool {
    let Some((key, value)) = line.trim().split_once('=') else {
        return false;
    };
    let value = value.trim();
    match key {
        "out_time_us" | "out_time_ms" => {
            if let Ok(us) = value.parse::<i64>() {
                p.out_time = us.max(0) as f64 / 1e6;
            }
        }
        "fps" => p.fps = value.parse().unwrap_or(p.fps),
        "speed" => p.speed = value.trim_end_matches('x').trim().parse().unwrap_or(p.speed),
        "total_size" => p.size = value.parse().unwrap_or(p.size),
        "progress" => return true,
        _ => {}
    }
    false
}

pub fn spawn(argv: &[String], send: impl Fn(RunEvent) + Send + 'static) -> Result<Handle> {
    let (program, rest) = argv.split_first().context("the command is empty")?;
    let mut child = Command::new(program)
        .args(service_flags(argv))
        .args(rest)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take().context("ffmpeg gave no stdout")?;
    let stderr = child.stderr.take().context("ffmpeg gave no stderr")?;

    let log = Arc::new(Mutex::new(VecDeque::new()));
    let cancelled = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let child = Arc::new(Mutex::new(child));

    let log_writer = log.clone();
    let stderr_thread = thread::spawn(move || {
        let mut text = String::new();
        let mut bytes = [0u8; 4096];
        let mut reader = stderr;
        let mut pending = Vec::new();
        while let Ok(n) = reader.read(&mut bytes) {
            if n == 0 {
                break;
            }
            pending.extend_from_slice(&bytes[..n]);
            // ffmpeg ends some lines with a bare carriage return.
            while let Some(pos) = pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
                text.clear();
                text.push_str(String::from_utf8_lossy(&pending[..pos]).trim_end());
                pending.drain(..=pos);
                if !text.is_empty() {
                    let mut log = log_writer.lock().unwrap();
                    if log.len() == LOG_LINES {
                        log.pop_front();
                    }
                    log.push_back(text.clone());
                }
            }
        }
    });

    let (child_waiter, cancelled_waiter, done_waiter) = (child.clone(), cancelled.clone(), done.clone());
    thread::spawn(move || {
        let mut progress = Progress::default();
        for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
            if feed(&mut progress, &line) {
                send(RunEvent::Progress(progress));
            }
        }
        let _ = stderr_thread.join();
        // stdout is closed, so the process is leaving; poll so `cancel` can
        // still take the lock to kill a stuck one.
        let code = loop {
            match child_waiter.lock().unwrap().try_wait() {
                Ok(Some(status)) => break status.code(),
                Ok(None) => {}
                Err(_) => break None,
            }
            thread::sleep(Duration::from_millis(20));
        };
        done_waiter.store(true, Ordering::SeqCst);
        send(RunEvent::Finished { code, cancelled: cancelled_waiter.load(Ordering::SeqCst) });
    });

    Ok(Handle { child, stdin, cancelled, done, log })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_progress_block() {
        let mut p = Progress::default();
        let block = "frame=71\nfps=71.2\ntotal_size=9800000\nout_time_us=41000000\nspeed=2.4x\n";
        for line in block.lines() {
            assert!(!feed(&mut p, line));
        }
        assert!(feed(&mut p, "progress=continue"));
        assert_eq!(p, Progress { out_time: 41.0, fps: 71.2, speed: 2.4, size: 9_800_000 });
    }

    #[test]
    fn ignores_values_ffmpeg_does_not_know_yet() {
        let mut p = Progress { out_time: 5.0, ..Default::default() };
        feed(&mut p, "out_time_us=N/A");
        feed(&mut p, "speed=N/A");
        assert_eq!(p.out_time, 5.0);
    }

    #[test]
    fn never_overwrites_without_y() {
        let shown: Vec<String> = ["ffmpeg", "-i", "a.mov", "b.mp4"].map(String::from).to_vec();
        assert!(service_flags(&shown).contains(&"-n".to_string()));
        let with_y: Vec<String> = ["ffmpeg", "-y", "-i", "a.mov", "b.mp4"].map(String::from).to_vec();
        assert!(!service_flags(&with_y).contains(&"-n".to_string()));
    }
}
