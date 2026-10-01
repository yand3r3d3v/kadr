//! An operation is data: its fields, and the command built from their values.
//! Every piece of the command remembers which field it came from, so the form
//! can light it up, the runner can execute it and `--print` can print it.

pub mod audio;
pub mod compress;
pub mod convert;
pub mod cut;
pub mod frame;
pub mod gif;
pub mod speed;

use std::path::PathBuf;

use crate::probe::MediaInfo;
use crate::util::shell_quote;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpKind {
    Compress,
    Cut,
    Gif,
    Audio,
    Convert,
    Speed,
    Frame,
}

impl OpKind {
    pub const ALL: [OpKind; 7] = [
        OpKind::Compress,
        OpKind::Cut,
        OpKind::Gif,
        OpKind::Audio,
        OpKind::Convert,
        OpKind::Speed,
        OpKind::Frame,
    ];

    pub fn name(self) -> &'static str {
        match self {
            OpKind::Compress => "compress",
            OpKind::Cut => "cut",
            OpKind::Gif => "gif",
            OpKind::Audio => "audio",
            OpKind::Convert => "convert",
            OpKind::Speed => "speed",
            OpKind::Frame => "frame",
        }
    }

    pub fn available(self, info: &MediaInfo) -> bool {
        match self {
            OpKind::Audio => info.audio.is_some(),
            _ => info.video.is_some(),
        }
    }

    /// Operations where a frame of the video helps to choose a time.
    pub fn has_timeline(self) -> bool {
        matches!(self, OpKind::Cut | OpKind::Gif | OpKind::Frame)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldId {
    File,
    Crf,
    Preset,
    Resolution,
    Sound,
    Cursor,
    Start,
    End,
    Length,
    Mode,
    Duration,
    Fps,
    Width,
    Format,
    Quality,
    Container,
    Speed,
    Image,
    Output,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub label: String,
    pub hint: &'static str,
}

#[derive(Clone, Debug)]
pub enum Kind {
    File,
    /// `soft` is the range worth staying in; outside it the value turns red.
    Int { min: i64, max: i64, soft: (i64, i64), step: i64, big: i64 },
    /// `inline` choices sit side by side; the rest open as a list.
    Choice { items: Vec<Choice>, selected: usize, inline: bool },
    Time,
    Text,
    Info,
}

#[derive(Clone, Debug)]
pub struct FieldSpec {
    pub id: FieldId,
    pub label: &'static str,
    pub kind: Kind,
    pub value: String,
    pub hint: &'static str,
    pub enabled: bool,
}

impl FieldSpec {
    pub fn focusable(&self) -> bool {
        self.enabled && !matches!(self.kind, Kind::Info)
    }
}

fn inline(id: FieldId, label: &'static str, items: &[(&str, &'static str)], selected: usize) -> FieldSpec {
    FieldSpec {
        id,
        label,
        kind: Kind::Choice {
            items: items.iter().map(|(l, hint)| Choice { label: l.to_string(), hint }).collect(),
            selected,
            inline: true,
        },
        value: items[selected.min(items.len() - 1)].0.to_string(),
        hint: "",
        enabled: true,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Program,
    Flag,
    Fixed,
    Value,
    Output,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub text: String,
    pub role: Role,
    pub field: Option<FieldId>,
}

/// One argv entry. It can be made of several parts: in `scale=-2:720` only
/// `720` belongs to a field.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Arg {
    pub parts: Vec<Part>,
    pub is_output: bool,
}

impl Arg {
    pub fn text(&self) -> String {
        self.parts.iter().map(|p| p.text.as_str()).collect()
    }

    pub fn is_flag(&self) -> bool {
        self.parts.first().is_some_and(|p| p.role == Role::Flag)
    }
}

pub type CommandLine = Vec<Arg>;

pub fn argv(cmd: &CommandLine) -> Vec<String> {
    cmd.iter().map(Arg::text).collect()
}

pub fn shell(cmd: &CommandLine) -> String {
    argv(cmd).iter().map(|a| shell_quote(a)).collect::<Vec<_>>().join(" ")
}

pub fn part(text: impl Into<String>, role: Role, field: Option<FieldId>) -> Part {
    Part { text: text.into(), role, field }
}

pub struct Builder(CommandLine);

impl Builder {
    pub fn new(overwrite: bool) -> Self {
        let mut b = Builder(vec![]);
        b.arg(vec![part("ffmpeg", Role::Program, None)]);
        if overwrite {
            b.flag("-y", None);
        }
        b
    }

    pub fn arg(&mut self, parts: Vec<Part>) -> &mut Self {
        self.0.push(Arg { parts, is_output: false });
        self
    }

    pub fn flag(&mut self, text: &str, field: Option<FieldId>) -> &mut Self {
        self.arg(vec![part(text, Role::Flag, field)])
    }

    pub fn fixed(&mut self, text: &str) -> &mut Self {
        self.arg(vec![part(text, Role::Fixed, None)])
    }

    pub fn value(&mut self, text: impl Into<String>, field: FieldId) -> &mut Self {
        self.arg(vec![part(text, Role::Value, Some(field))])
    }

    pub fn input(&mut self, info: &MediaInfo) -> &mut Self {
        self.flag("-i", Some(FieldId::File)).value(input_arg(info), FieldId::File)
    }

    pub fn output(mut self, op: OpKind, v: &Values, info: &MediaInfo) -> CommandLine {
        self.0.push(Arg { parts: output_parts(op, v, info), is_output: true });
        self.0
    }
}

/// The values of every field of every operation. One struct keeps a value
/// alive when the user switches tabs and comes back; `start` and `cursor`
/// are shared on purpose, so a moment found in one tab is there in the next.
#[derive(Clone, Debug)]
pub struct Values {
    pub crf: i64,
    pub preset: usize,
    pub resolution: usize,
    pub sound: usize,
    pub cursor: f64,
    pub start: f64,
    pub end: f64,
    pub mode: usize,
    pub duration: i64,
    pub fps: i64,
    pub width: i64,
    pub format: usize,
    pub quality: i64,
    pub container: usize,
    pub speed: usize,
    pub image: usize,
    /// A name the user typed; `None` means the default next to the input.
    pub output: Option<String>,
    pub overwrite: bool,
}

pub const SOUND_KEEP: usize = 0;
pub const SOUND_REMOVE: usize = 1;
pub const MODE_FAST: usize = 0;
pub const MODE_EXACT: usize = 1;
pub const FORMAT_MP3: usize = 0;
pub const FORMAT_COPY: usize = 1;
pub const IMAGE_PNG: usize = 0;
pub const IMAGE_JPG: usize = 1;

impl Values {
    pub fn defaults(info: &MediaInfo) -> Self {
        Values {
            crf: 23,
            preset: compress::DEFAULT_PRESET,
            resolution: compress::default_resolution(info),
            sound: SOUND_KEEP,
            cursor: 0.0,
            start: 0.0,
            end: info.duration,
            mode: MODE_FAST,
            duration: gif::default_duration(info),
            fps: 12,
            width: gif::default_width(info),
            format: FORMAT_MP3,
            quality: 2,
            container: 0,
            speed: speed::DEFAULT,
            image: IMAGE_PNG,
            output: None,
            overwrite: false,
        }
    }
}

/// Something that stops the command from running. `text` is an English
/// template; `{}` in it stands for `arg`.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub field: Option<FieldId>,
    pub text: &'static str,
    pub arg: String,
}

impl Problem {
    fn new(field: FieldId, text: &'static str) -> Self {
        Problem { field: Some(field), text, arg: String::new() }
    }

    pub fn english(&self) -> String {
        self.text.replace("{}", &self.arg)
    }
}

pub fn fields(op: OpKind, v: &Values, info: &MediaInfo) -> Vec<FieldSpec> {
    let mut f = vec![FieldSpec {
        id: FieldId::File,
        label: "file",
        kind: Kind::File,
        value: input_arg(info),
        hint: "enter picks another file",
        enabled: true,
    }];
    f.extend(match op {
        OpKind::Compress => compress::fields(v, info),
        OpKind::Cut => cut::fields(v, info),
        OpKind::Gif => gif::fields(v, info),
        OpKind::Audio => audio::fields(v, info),
        OpKind::Convert => convert::fields(v, info),
        OpKind::Speed => speed::fields(v, info),
        OpKind::Frame => frame::fields(v, info),
    });
    f.push(FieldSpec {
        id: FieldId::Output,
        label: "output",
        kind: Kind::Text,
        value: output_name(op, v, info),
        hint: "type a new name",
        enabled: true,
    });
    f
}

pub fn build(op: OpKind, v: &Values, info: &MediaInfo) -> CommandLine {
    match op {
        OpKind::Compress => compress::build(v, info),
        OpKind::Cut => cut::build(v, info),
        OpKind::Gif => gif::build(v, info),
        OpKind::Audio => audio::build(v, info),
        OpKind::Convert => convert::build(v, info),
        OpKind::Speed => speed::build(v, info),
        OpKind::Frame => frame::build(v, info),
    }
}

pub fn check(op: OpKind, v: &Values, info: &MediaInfo) -> Vec<Problem> {
    let mut problems = match op {
        OpKind::Cut => cut::check(v, info),
        OpKind::Gif => gif::check(v, info),
        _ => vec![],
    };
    let out = output_path(op, v, info);
    let same = match (out.canonicalize(), info.path.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => out == info.path,
    };
    if same {
        problems.push(Problem::new(
            FieldId::Output,
            "The output is the input file itself. Change the name.",
        ));
    }
    problems
}

/// How long the result is: progress is counted against this, not the input.
pub fn result_duration(op: OpKind, v: &Values, info: &MediaInfo) -> f64 {
    match op {
        OpKind::Cut => (v.end - v.start).max(0.0),
        OpKind::Gif => v.duration as f64,
        OpKind::Speed => info.duration / speed::SPEEDS[v.speed].0,
        OpKind::Frame => 0.0,
        _ => info.duration,
    }
}

pub fn input_arg(info: &MediaInfo) -> String {
    let s = info.path.to_string_lossy().into_owned();
    // A leading dash would read as an option.
    if s.starts_with('-') { format!("./{s}") } else { s }
}

fn stem(info: &MediaInfo) -> String {
    info.path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into())
}

fn source_ext(info: &MediaInfo) -> String {
    info.path
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "mp4".into())
}

/// `name` placed next to the input file, as the start of the output path.
fn sibling(info: &MediaInfo, name: String) -> Part {
    let s = info.path.with_file_name(name).to_string_lossy().into_owned();
    let s = if s.starts_with('-') { format!("./{s}") } else { s };
    part(s, Role::Output, Some(FieldId::Output))
}

/// The output argument. Pieces of a default name can belong to fields: the
/// extension follows the audio format, the frame's name follows its time.
fn output_parts(op: OpKind, v: &Values, info: &MediaInfo) -> Vec<Part> {
    if let Some(name) = &v.output {
        return vec![part(name.clone(), Role::Output, Some(FieldId::Output))];
    }
    match op {
        OpKind::Compress => vec![sibling(info, format!("{}_small.mp4", stem(info)))],
        OpKind::Cut => cut::output(v, info),
        OpKind::Gif => vec![sibling(info, format!("{}.gif", stem(info)))],
        OpKind::Audio => audio::output(v, info),
        OpKind::Convert => convert::output(v, info),
        OpKind::Speed => speed::output(v, info),
        OpKind::Frame => frame::output(v, info),
    }
}

pub fn output_name(op: OpKind, v: &Values, info: &MediaInfo) -> String {
    output_parts(op, v, info).iter().map(|p| p.text.as_str()).collect()
}

pub fn output_path(op: OpKind, v: &Values, info: &MediaInfo) -> PathBuf {
    PathBuf::from(output_name(op, v, info))
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use crate::probe::{Audio, Video};

    pub fn lecture() -> MediaInfo {
        MediaInfo {
            path: "lecture_04.mov".into(),
            duration: 130.0,
            size: 48_000_000,
            video: Some(Video { codec: "h264".into(), width: 1920, height: 1080, fps: 25.0 }),
            audio: Some(Audio { codec: "aac".into(), bitrate: Some(128_000), channels: 2 }),
        }
    }
}
