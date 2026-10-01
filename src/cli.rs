//! The command line. Option names follow the form's field labels, not the
//! ffmpeg flags behind them.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::op::{self, OpKind, Values, compress};
use crate::probe::MediaInfo;
use crate::util::parse_time;

#[derive(Parser, Debug)]
#[command(
    name = "kadr",
    version,
    about = "⌜ kadr ⌟  the ffmpeg command, assembled as you watch",
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    /// Open the form on this file
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Cmd>,

    #[command(flatten)]
    pub global: Global,
}

#[derive(Args, Debug, Clone, Copy)]
pub struct Global {
    /// Build the command and print it, do not run
    #[arg(long, global = true)]
    pub print: bool,

    /// Run right away, without the interface
    #[arg(long, global = true, conflicts_with = "print")]
    pub run: bool,

    /// Overwrite the output without asking
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Make the file smaller (h264)
    Compress {
        file: PathBuf,
        /// Quality, 18 (better) to 28 (smaller file)
        #[arg(long)]
        crf: Option<i64>,
        /// x264 preset: ultrafast … medium … veryslow
        #[arg(long)]
        preset: Option<String>,
        /// Frame height: source, 1080p, 720p or 480p
        #[arg(long)]
        resolution: Option<String>,
        /// Name of the new file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Take a piece by time
    Cut {
        file: PathBuf,
        /// Where the piece starts: 38, 0:38 or 00:00:38
        #[arg(long)]
        from: Option<String>,
        /// Where the piece ends
        #[arg(long)]
        to: Option<String>,
        /// Re-encode for a clean edge instead of copying the stream
        #[arg(long)]
        exact: bool,
        /// Name of the new file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Extract the audio track
    Audio {
        file: PathBuf,
        /// mp3, or copy to keep the track as is
        #[arg(long, value_enum)]
        format: Option<AudioFormat>,
        /// mp3 quality, 0 (better) to 9 (smaller file)
        #[arg(long)]
        quality: Option<i64>,
        /// Name of the new file
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Mp3,
    Copy,
}

/// Field values given on the command line, before the file is known.
#[derive(Default, Debug, Clone)]
pub struct Prefill {
    pub crf: Option<i64>,
    pub preset: Option<String>,
    pub resolution: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub exact: bool,
    pub format: Option<AudioFormat>,
    pub quality: Option<i64>,
    pub output: Option<String>,
}

pub struct Launch {
    pub op: Option<OpKind>,
    pub file: Option<PathBuf>,
    pub prefill: Prefill,
    pub global: Global,
}

impl Cli {
    pub fn launch(self) -> Launch {
        let global = self.global;
        match self.command {
            None => Launch { op: None, file: self.file, prefill: Prefill::default(), global },
            Some(Cmd::Compress { file, crf, preset, resolution, output }) => Launch {
                op: Some(OpKind::Compress),
                file: Some(file),
                prefill: Prefill { crf, preset, resolution, output, ..Default::default() },
                global,
            },
            Some(Cmd::Cut { file, from, to, exact, output }) => Launch {
                op: Some(OpKind::Cut),
                file: Some(file),
                prefill: Prefill { from, to, exact, output, ..Default::default() },
                global,
            },
            Some(Cmd::Audio { file, format, quality, output }) => Launch {
                op: Some(OpKind::Audio),
                file: Some(file),
                prefill: Prefill { format, quality, output, ..Default::default() },
                global,
            },
        }
    }
}

fn time(label: &str, s: &str, info: &MediaInfo) -> Result<f64> {
    match parse_time(s) {
        Some(t) if t <= info.duration + 0.05 => Ok(t.min(info.duration)),
        Some(_) => bail!("--{label} {s} is past the end of the file"),
        None => bail!("--{label} {s} is not a time; write 38, 0:38 or 00:00:38"),
    }
}

pub fn apply(p: &Prefill, v: &mut Values, info: &MediaInfo) -> Result<()> {
    if let Some(crf) = p.crf {
        if !(0..=51).contains(&crf) {
            bail!("--crf {crf} is out of range; x264 takes 0 to 51");
        }
        v.crf = crf;
    }
    if let Some(name) = &p.preset {
        match compress::PRESETS.iter().position(|(n, _)| n == name) {
            Some(i) => v.preset = i,
            None => bail!(
                "--preset {name} is not an x264 preset; pick one of: {}",
                compress::PRESETS.map(|(n, _)| n).join(", ")
            ),
        }
    }
    if let Some(res) = &p.resolution {
        let list = compress::resolutions(info);
        let wanted = match res.trim_end_matches('p') {
            "source" => Some(None),
            n => n.parse::<u32>().ok().map(Some),
        };
        match wanted.and_then(|w| list.iter().position(|r| *r == w)) {
            Some(i) => v.resolution = i,
            None => bail!(
                "--resolution {res} does not fit this file; pick one of: {}",
                list.iter().map(|r| compress::resolution_label(*r)).collect::<Vec<_>>().join(", ")
            ),
        }
    }
    if let Some(from) = &p.from {
        v.start = time("from", from, info)?;
    }
    if let Some(to) = &p.to {
        v.end = time("to", to, info)?;
    }
    if p.exact {
        v.mode = op::MODE_EXACT;
    }
    if let Some(format) = p.format {
        v.format = if format == AudioFormat::Copy { op::FORMAT_COPY } else { op::FORMAT_MP3 };
    }
    if let Some(q) = p.quality {
        if !(0..=9).contains(&q) {
            bail!("--quality {q} is out of range; mp3 takes 0 to 9");
        }
        v.quality = q;
    }
    if let Some(out) = &p.output {
        v.output = Some(out.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::testing::lecture;

    fn launch(args: &[&str]) -> Launch {
        Cli::try_parse_from(args).unwrap().launch()
    }

    #[test]
    fn bare_file_opens_the_form() {
        let l = launch(&["kadr", "a.mov"]);
        assert_eq!(l.op, None);
        assert_eq!(l.file, Some("a.mov".into()));
    }

    #[test]
    fn operation_with_options_and_print() {
        let l = launch(&["kadr", "compress", "a.mov", "--resolution", "720p", "--print"]);
        assert_eq!(l.op, Some(OpKind::Compress));
        assert!(l.global.print);
        let info = lecture();
        let mut v = Values::defaults(&info);
        apply(&l.prefill, &mut v, &info).unwrap();
        assert_eq!(compress::resolutions(&info)[v.resolution], Some(720));
    }

    #[test]
    fn cut_times() {
        let l = launch(&["kadr", "cut", "a.mov", "--from", "0:38", "--to", "1:12", "--run"]);
        let info = lecture();
        let mut v = Values::defaults(&info);
        apply(&l.prefill, &mut v, &info).unwrap();
        assert_eq!((v.start, v.end), (38.0, 72.0));
        assert!(l.global.run);
    }

    #[test]
    fn refuses_to_scale_up() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        let p = Prefill { resolution: Some("2160p".into()), ..Default::default() };
        assert!(apply(&p, &mut v, &info).is_err());
    }

    #[test]
    fn print_and_run_do_not_mix() {
        assert!(Cli::try_parse_from(["kadr", "audio", "a.mov", "--print", "--run"]).is_err());
    }
}
