//! `--print` and `--run`: the same command without the form.

use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::sync::mpsc;

use anyhow::Result;

use crate::op::{self, OpKind, Values};
use crate::probe::{self, MediaInfo};
use crate::run::{self, RunEvent};
use crate::util::{fmt_clock, fmt_size, fmt_ts};

struct Paint(bool);

impl Paint {
    fn wrap(&self, code: &str, text: &str) -> String {
        if self.0 { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
    }
    fn dim(&self, t: &str) -> String {
        self.wrap("2", t)
    }
    fn ice(&self, t: &str) -> String {
        self.wrap("36", t)
    }
    fn ochre(&self, t: &str) -> String {
        self.wrap("33", t)
    }
    fn carmine(&self, t: &str) -> String {
        self.wrap("1;31", t)
    }
}

pub fn print(op: OpKind, v: &Values, info: &MediaInfo) -> ExitCode {
    println!("{}", op::shell(&op::build(op, v, info)));
    ExitCode::SUCCESS
}

pub fn run(op: OpKind, v: &Values, info: &MediaInfo) -> Result<ExitCode> {
    let tty = std::io::stderr().is_terminal();
    let paint = Paint(tty && std::env::var_os("NO_COLOR").is_none());
    let fail = |text: String| {
        eprintln!("{} {text}", paint.carmine("✕"));
        Ok(ExitCode::FAILURE)
    };

    if let Some(p) = op::check(op, v, info).into_iter().next() {
        return fail(p.text);
    }
    let output = op::output_path(op, v, info);
    let name = output.to_string_lossy().into_owned();
    if output.exists() && !v.overwrite {
        return fail(format!("{name} already exists. Add -y to overwrite or -o to rename."));
    }

    let cmd = op::build(op, v, info);
    let total = op::result_duration(op, v, info);
    eprintln!("{}", paint.dim(&op::shell(&cmd)));

    let (tx, rx) = mpsc::channel();
    let handle = run::spawn(&op::argv(&cmd), move |e| {
        let _ = tx.send(e);
    })?;

    let cells = 40usize;
    let mut code = None;
    for event in rx {
        match event {
            RunEvent::Progress(p) if tty => {
                let frac = if total > 0.0 { (p.out_time / total).clamp(0.0, 1.0) } else { 0.0 };
                let filled = (frac * cells as f64).round() as usize;
                eprint!(
                    "\r{}{}{} {} / {} ",
                    paint.ice(&format!("┣{}", "━".repeat(filled))),
                    paint.dim(&"┈".repeat(cells - filled)),
                    paint.ice("┫"),
                    fmt_clock(p.out_time.min(total)),
                    fmt_clock(total)
                );
                let _ = std::io::stderr().flush();
            }
            RunEvent::Progress(_) => {}
            RunEvent::Finished { code: c, .. } => {
                code = c;
                break;
            }
        }
    }
    if tty {
        eprint!("\r\x1b[K");
    }

    if code != Some(0) {
        let removed = std::fs::remove_file(&output).is_ok();
        for line in handle.log().iter().rev().take(3).rev() {
            eprintln!("{}", paint.dim(line));
        }
        let code = code.map_or("a signal".to_string(), |c| format!("code {c}"));
        let tail = if removed { format!(" The unfinished {name} was removed.") } else { String::new() };
        return fail(format!("ffmpeg stopped with {code}.{tail}"));
    }

    let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    eprintln!("{} {name}, {}", paint.ice("✓"), fmt_size(size));
    if op == OpKind::Cut && v.mode == op::MODE_FAST
        && let Ok(result) = probe::probe(&output) {
            let got_start = (v.end - result.duration).max(0.0);
            if v.start - got_start > 0.3 {
                eprintln!(
                    "{}",
                    paint.ochre(&format!(
                        "the start moved to {}, the keyframe before it; use --exact for a clean edge",
                        fmt_ts(got_start.round())
                    ))
                );
            }
        }
    Ok(ExitCode::SUCCESS)
}
