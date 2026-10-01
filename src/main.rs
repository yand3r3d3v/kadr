mod app;
mod cli;
mod command;
mod config;
mod op;
mod picker;
mod plain;
mod preview;
mod probe;
mod run;
mod text;
mod theme;
mod ui;
mod util;

use std::process::{Command, ExitCode, Stdio};

use anyhow::{Result, bail};
use clap::Parser;

use crate::op::Values;

fn installed(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn real_main() -> Result<ExitCode> {
    let launch = cli::Cli::parse().launch();
    for tool in ["ffmpeg", "ffprobe"] {
        if !installed(tool) {
            bail!("{tool} is not in PATH. Install it first: brew install ffmpeg");
        }
    }

    let config = config::load();
    let info = launch.file.as_deref().map(probe::probe).transpose()?;
    if let (Some(op), Some(info)) = (launch.op, &info)
        && !op.available(info) {
            let missing = if op == op::OpKind::Audio { "audio" } else { "video" };
            bail!("there is no {missing} in {}, so there is nothing to {}", info.path.display(), op.name());
        }

    if launch.global.print || launch.global.run {
        let (Some(op), Some(info)) = (launch.op, &info) else {
            bail!("--print and --run need an operation: kadr compress FILE --print");
        };
        let mut values = Values::defaults(info);
        cli::apply(&config.defaults, &mut values, info, false)?;
        cli::apply(&launch.prefill, &mut values, info, true)?;
        values.overwrite = launch.global.yes;
        return if launch.global.print {
            Ok(plain::print(op, &values, info))
        } else {
            plain::run(op, &values, info)
        };
    }

    app::run(app::Start { op: launch.op, info, prefill: launch.prefill, config })?;
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("\x1b[1;31m✕\x1b[0m {e:#}");
            ExitCode::FAILURE
        }
    }
}
