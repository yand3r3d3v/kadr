//! An operation is data: its fields, and the command built from their values.
//! Every piece of the command remembers which field it came from, so the form
//! can light it up, the runner can execute it and `--print` can print it.

pub mod audio;
pub mod compress;
pub mod cut;

use std::path::PathBuf;

use crate::probe::MediaInfo;
use crate::util::shell_quote;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpKind {
    Compress,
    Cut,
    Audio,
}

impl OpKind {
    pub const ALL: [OpKind; 3] = [OpKind::Compress, OpKind::Cut, OpKind::Audio];

    pub fn name(self) -> &'static str {
        match self {
            OpKind::Compress => "compress",
            OpKind::Cut => "cut",
            OpKind::Audio => "audio",
        }
    }

    pub fn available(self, info: &MediaInfo) -> bool {
        match self {
            OpKind::Compress | OpKind::Cut => info.video.is_some(),
            OpKind::Audio => info.audio.is_some(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldId {
    File,
    Crf,
    Preset,
    Resolution,
    Start,
    End,
    Length,
    Mode,
    Format,
    Quality,
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
    Int { min: i64, max: i64, soft: (i64, i64), big: i64 },
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

    pub fn output(mut self, parts: Vec<Part>) -> CommandLine {
        self.0.push(Arg { parts, is_output: true });
        self.0
    }
}

/// The values of every field of every operation. One struct keeps a value
/// alive when the user switches tabs and comes back.
#[derive(Clone, Debug)]
pub struct Values {
    pub crf: i64,
    pub preset: usize,
    pub resolution: usize,
    pub start: f64,
    pub end: f64,
    pub mode: usize,
    pub format: usize,
    pub quality: i64,
    /// A name the user typed; `None` means the default next to the input.
    pub output: Option<String>,
    pub overwrite: bool,
}

pub const MODE_FAST: usize = 0;
pub const MODE_EXACT: usize = 1;
pub const FORMAT_MP3: usize = 0;
pub const FORMAT_COPY: usize = 1;

impl Values {
    pub fn defaults(info: &MediaInfo) -> Self {
        Values {
            crf: 23,
            preset: compress::DEFAULT_PRESET,
            resolution: compress::default_resolution(info),
            start: 0.0,
            end: info.duration,
            mode: MODE_FAST,
            format: FORMAT_MP3,
            quality: 2,
            output: None,
            overwrite: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub field: Option<FieldId>,
    pub text: String,
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
        OpKind::Audio => audio::fields(v, info),
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
        OpKind::Audio => audio::build(v, info),
    }
}

pub fn check(op: OpKind, v: &Values, info: &MediaInfo) -> Vec<Problem> {
    let mut problems = match op {
        OpKind::Cut => cut::check(v, info),
        _ => vec![],
    };
    let out = output_path(op, v, info);
    let same = match (out.canonicalize(), info.path.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => out == info.path,
    };
    if same {
        problems.push(Problem {
            field: Some(FieldId::Output),
            text: "The output is the input file itself. Change the name.".into(),
        });
    }
    problems
}

/// How long the result is: progress is counted against this, not the input.
pub fn result_duration(op: OpKind, v: &Values, info: &MediaInfo) -> f64 {
    match op {
        OpKind::Cut => (v.end - v.start).max(0.0),
        _ => info.duration,
    }
}

pub fn input_arg(info: &MediaInfo) -> String {
    let s = info.path.to_string_lossy().into_owned();
    // A leading dash would read as an option.
    if s.starts_with('-') { format!("./{s}") } else { s }
}

fn default_output(op: OpKind, v: &Values, info: &MediaInfo) -> (String, String) {
    let stem = info
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".into());
    let source_ext = info
        .path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mp4".into());
    let (name, ext) = match op {
        OpKind::Compress => (format!("{stem}_small"), "mp4".to_string()),
        OpKind::Cut if v.mode == MODE_EXACT => (format!("{stem}_cut"), "mp4".to_string()),
        OpKind::Cut => (format!("{stem}_cut"), source_ext),
        OpKind::Audio => (stem, audio::extension(v, info).to_string()),
    };
    let stem_path = info.path.with_file_name(name).to_string_lossy().into_owned();
    let stem_path = if stem_path.starts_with('-') { format!("./{stem_path}") } else { stem_path };
    (stem_path, ext)
}

pub fn output_name(op: OpKind, v: &Values, info: &MediaInfo) -> String {
    match &v.output {
        Some(name) => name.clone(),
        None => {
            let (stem, ext) = default_output(op, v, info);
            format!("{stem}.{ext}")
        }
    }
}

pub fn output_path(op: OpKind, v: &Values, info: &MediaInfo) -> PathBuf {
    PathBuf::from(output_name(op, v, info))
}

/// The output argument. `ext_field` marks the extension as driven by a field,
/// as with the audio format.
fn output_parts(op: OpKind, v: &Values, info: &MediaInfo, ext_field: Option<FieldId>) -> Vec<Part> {
    let out = Some(FieldId::Output);
    match (&v.output, ext_field) {
        (Some(name), _) => vec![part(name.clone(), Role::Output, out)],
        (None, Some(field)) => {
            let (stem, ext) = default_output(op, v, info);
            vec![part(stem, Role::Output, out), part(format!(".{ext}"), Role::Value, Some(field))]
        }
        (None, None) => vec![part(output_name(op, v, info), Role::Output, out)],
    }
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
