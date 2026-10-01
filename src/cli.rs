//! The command line. Option names follow the form's field labels, not the
//! ffmpeg flags behind them.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::op::{self, OpKind, Values, compress, convert, gif, speed};
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
        /// Drop the audio track
        #[arg(long)]
        no_audio: bool,
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
    /// Turn a short fragment into a gif
    Gif {
        file: PathBuf,
        /// Where the fragment starts: 38, 0:38 or 00:00:38
        #[arg(long)]
        from: Option<String>,
        /// How long it is, in seconds
        #[arg(long)]
        length: Option<i64>,
        /// Frames per second
        #[arg(long)]
        fps: Option<i64>,
        /// Width in pixels; the height follows
        #[arg(long)]
        width: Option<i64>,
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
    /// Change the container without re-encoding
    Convert {
        file: PathBuf,
        /// mp4, mkv or mov
        #[arg(long)]
        container: Option<String>,
        /// Name of the new file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Speed the video up or slow it down
    Speed {
        file: PathBuf,
        /// 0.5, 0.75, 1.25, 1.5 or 2
        #[arg(long)]
        speed: Option<String>,
        /// Name of the new file
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Save one frame as an image
    Frame {
        file: PathBuf,
        /// The moment: 38, 0:38 or 00:00:38
        #[arg(long)]
        at: Option<String>,
        /// png or jpg
        #[arg(long)]
        format: Option<String>,
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

/// Field values given on the command line or in the config, before the file
/// is known.
#[derive(Default, Debug, Clone)]
pub struct Prefill {
    pub crf: Option<i64>,
    pub preset: Option<String>,
    pub resolution: Option<String>,
    pub no_audio: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub exact: bool,
    pub length: Option<i64>,
    pub fps: Option<i64>,
    pub width: Option<i64>,
    pub format: Option<AudioFormat>,
    pub quality: Option<i64>,
    pub container: Option<String>,
    pub speed: Option<String>,
    pub at: Option<String>,
    pub image: Option<String>,
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
        let (op, file, prefill) = match self.command {
            None => (None, self.file, Prefill::default()),
            Some(Cmd::Compress { file, crf, preset, resolution, no_audio, output }) => (
                Some(OpKind::Compress),
                Some(file),
                Prefill { crf, preset, resolution, no_audio, output, ..Default::default() },
            ),
            Some(Cmd::Cut { file, from, to, exact, output }) => {
                (Some(OpKind::Cut), Some(file), Prefill { from, to, exact, output, ..Default::default() })
            }
            Some(Cmd::Gif { file, from, length, fps, width, output }) => (
                Some(OpKind::Gif),
                Some(file),
                Prefill { from, length, fps, width, output, ..Default::default() },
            ),
            Some(Cmd::Audio { file, format, quality, output }) => {
                (Some(OpKind::Audio), Some(file), Prefill { format, quality, output, ..Default::default() })
            }
            Some(Cmd::Convert { file, container, output }) => {
                (Some(OpKind::Convert), Some(file), Prefill { container, output, ..Default::default() })
            }
            Some(Cmd::Speed { file, speed, output }) => {
                (Some(OpKind::Speed), Some(file), Prefill { speed, output, ..Default::default() })
            }
            Some(Cmd::Frame { file, at, format, output }) => {
                (Some(OpKind::Frame), Some(file), Prefill { at, image: format, output, ..Default::default() })
            }
        };
        Launch { op, file, prefill, global }
    }
}

fn time(label: &str, s: &str, info: &MediaInfo) -> Result<f64> {
    match parse_time(s) {
        Some(t) if t <= info.duration + 0.05 => Ok(t.min(info.duration)),
        Some(_) => Err(anyhow!("--{label} {s} is past the end of the file")),
        None => Err(anyhow!("--{label} {s} is not a time; write 38, 0:38 or 00:00:38")),
    }
}

fn range(label: &str, n: i64, min: i64, max: i64) -> Result<i64> {
    if (min..=max).contains(&n) {
        Ok(n)
    } else {
        Err(anyhow!("--{label} {n} is out of range; it takes {min} to {max}"))
    }
}

/// Puts the given values into `v`. With `strict`, a value that does not fit
/// is an error; without it, as for config defaults, the value is skipped: a
/// default of 720p must not stop a 480p file from opening.
pub fn apply(p: &Prefill, v: &mut Values, info: &MediaInfo, strict: bool) -> Result<()> {
    let set = |result: Result<()>| if strict { result } else { Ok(()) };

    if let Some(crf) = p.crf {
        set(range("crf", crf, 0, 51).map(|n| v.crf = n))?;
    }
    if let Some(name) = &p.preset {
        set(compress::PRESETS.iter().position(|(n, _)| n == name).map(|i| v.preset = i).ok_or_else(|| {
            anyhow!(
                "--preset {name} is not an x264 preset; pick one of: {}",
                compress::PRESETS.map(|(n, _)| n).join(", ")
            )
        }))?;
    }
    if let Some(res) = &p.resolution {
        let list = compress::resolutions(info);
        let wanted = match res.trim_end_matches('p') {
            "source" => Some(None),
            n => n.parse::<u32>().ok().map(Some),
        };
        set(wanted.and_then(|w| list.iter().position(|r| *r == w)).map(|i| v.resolution = i).ok_or_else(|| {
            anyhow!(
                "--resolution {res} does not fit this file; pick one of: {}",
                list.iter().map(|r| compress::resolution_label(*r)).collect::<Vec<_>>().join(", ")
            )
        }))?;
    }
    if p.no_audio {
        v.sound = op::SOUND_REMOVE;
    }
    if let Some(from) = &p.from {
        set(time("from", from, info).map(|t| v.start = t))?;
    }
    if let Some(to) = &p.to {
        set(time("to", to, info).map(|t| v.end = t))?;
    }
    if p.exact {
        v.mode = op::MODE_EXACT;
    }
    if let Some(n) = p.length {
        set(range("length", n, 1, gif::max_duration(info)).map(|n| v.duration = n))?;
    }
    if let Some(n) = p.fps {
        set(range("fps", n, 1, 50).map(|n| v.fps = n))?;
    }
    if let Some(n) = p.width {
        let max = info.video.as_ref().map_or(480, |v| v.width as i64).max(80);
        set(range("width", n, 80, max).map(|n| v.width = n))?;
    }
    if let Some(format) = p.format {
        v.format = if format == AudioFormat::Copy { op::FORMAT_COPY } else { op::FORMAT_MP3 };
    }
    if let Some(q) = p.quality {
        set(range("quality", q, 0, 9).map(|n| v.quality = n))?;
    }
    if let Some(c) = &p.container {
        let list = convert::containers(info);
        set(list.iter().position(|x| x == c).map(|i| v.container = i).ok_or_else(|| {
            anyhow!("--container {c} is not on offer for this file; pick one of: {}", list.join(", "))
        }))?;
    }
    if let Some(s) = &p.speed {
        let wanted = s.trim_end_matches(['x', '×']).parse::<f64>().ok();
        set(wanted
            .and_then(|w| speed::SPEEDS.iter().position(|(f, _, _)| *f == w))
            .map(|i| v.speed = i)
            .ok_or_else(|| anyhow!("--speed {s} is not on offer; pick one of: 0.5, 0.75, 1.25, 1.5, 2")))?;
    }
    if let Some(at) = &p.at {
        set(time("at", at, info).map(|t| v.cursor = t))?;
    }
    if let Some(image) = &p.image {
        let wanted = match image.as_str() {
            "png" => Ok(op::IMAGE_PNG),
            "jpg" | "jpeg" => Ok(op::IMAGE_JPG),
            other => Err(anyhow!("--format {other} is not on offer; pick png or jpg")),
        };
        set(wanted.map(|i| v.image = i))?;
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

    fn values(l: &Launch) -> Values {
        let info = lecture();
        let mut v = Values::defaults(&info);
        apply(&l.prefill, &mut v, &info, true).unwrap();
        v
    }

    #[test]
    fn bare_file_opens_the_form() {
        let l = launch(&["kadr", "a.mov"]);
        assert_eq!(l.op, None);
        assert_eq!(l.file, Some("a.mov".into()));
    }

    #[test]
    fn operation_with_options_and_print() {
        let l = launch(&["kadr", "compress", "a.mov", "--resolution", "720p", "--no-audio", "--print"]);
        assert_eq!(l.op, Some(OpKind::Compress));
        assert!(l.global.print);
        let v = values(&l);
        assert_eq!(compress::resolutions(&lecture())[v.resolution], Some(720));
        assert_eq!(v.sound, op::SOUND_REMOVE);
    }

    #[test]
    fn cut_times() {
        let l = launch(&["kadr", "cut", "a.mov", "--from", "0:38", "--to", "1:12", "--run"]);
        let v = values(&l);
        assert_eq!((v.start, v.end), (38.0, 72.0));
        assert!(l.global.run);
    }

    #[test]
    fn the_newer_operations() {
        let v = values(&launch(&["kadr", "gif", "a.mov", "--from", "38", "--length", "6", "--fps", "15"]));
        assert_eq!((v.start, v.duration, v.fps), (38.0, 6, 15));
        let v = values(&launch(&["kadr", "speed", "a.mov", "--speed", "1.5"]));
        assert_eq!(speed::SPEEDS[v.speed].0, 1.5);
        let v = values(&launch(&["kadr", "frame", "a.mov", "--at", "0:38", "--format", "jpg"]));
        assert_eq!((v.cursor, v.image), (38.0, op::IMAGE_JPG));
        let v = values(&launch(&["kadr", "convert", "a.mov", "--container", "mkv"]));
        assert_eq!(convert::containers(&lecture())[v.container], "mkv");
    }

    #[test]
    fn refuses_to_scale_up() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        let p = Prefill { resolution: Some("2160p".into()), ..Default::default() };
        assert!(apply(&p, &mut v, &info, true).is_err());
    }

    #[test]
    fn config_defaults_that_do_not_fit_are_skipped() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        let p = Prefill { resolution: Some("2160p".into()), crf: Some(26), ..Default::default() };
        apply(&p, &mut v, &info, false).unwrap();
        assert_eq!(v.crf, 26);
        assert_eq!(v.resolution, compress::default_resolution(&info));
    }

    #[test]
    fn print_and_run_do_not_mix() {
        assert!(Cli::try_parse_from(["kadr", "audio", "a.mov", "--print", "--run"]).is_err());
    }
}
