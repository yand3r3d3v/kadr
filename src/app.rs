//! The state of the form and every transition between its screens.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui_image::picker::Picker as GraphicsPicker;
use ratatui_image::protocol::Protocol;

use crate::cli::{self, Prefill};
use crate::config::{self, Config};
use crate::op::{self, CommandLine, FieldId, FieldSpec, Kind, OpKind, Values};
use crate::picker::Picker;
use crate::preview::{self, Preview};
use crate::probe::{self, MediaInfo};
use crate::run::{self, Handle, Progress, RunEvent};
use crate::text::{Lang, tr};
use crate::theme::Theme;
use crate::ui;
use crate::util::{latin_key, parse_time};

pub enum Event {
    Input(TermEvent),
    Probed { generation: u64, path: PathBuf, info: MediaInfo },
    Run(RunEvent),
    Keyframe { generation: u64, at: f64, key: Option<f64> },
    Frame { path: PathBuf, key: preview::Key, frame: Option<Protocol> },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Pick,
    Form,
    Exists,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    Keys,
    Log,
}

pub struct Doc {
    pub info: MediaInfo,
    pub values: Values,
}

pub struct Running {
    pub handle: Handle,
    pub cmd: CommandLine,
    pub progress: Progress,
    pub total: f64,
    pub output: PathBuf,
}

pub struct CutResult {
    pub start: f64,
    pub end: f64,
    pub got_start: f64,
}

pub struct Done {
    pub op: OpKind,
    pub cmd: CommandLine,
    pub output: PathBuf,
    pub before: u64,
    pub after: u64,
    pub total: f64,
    pub cut: Option<CutResult>,
}

pub struct Failed {
    pub cmd: CommandLine,
    pub at: f64,
    pub total: f64,
    pub code: Option<i32>,
    pub lines: Vec<String>,
    pub removed: Option<String>,
}

pub struct Toast {
    pub text: String,
    pub error: bool,
    until: Instant,
}

pub struct App {
    pub theme: Theme,
    pub lang: Lang,
    pub screen: Screen,
    pub overlay: Option<Overlay>,
    pub picker: Picker,
    pub doc: Option<Doc>,
    pub op: OpKind,
    pub focus: usize,
    /// Text being typed into the focused field, not yet a value.
    pub edit: Option<String>,
    /// The open value list: the highlighted item.
    pub dropdown: Option<usize>,
    pub running: Option<Running>,
    pub done: Option<Done>,
    pub failed: Option<Failed>,
    /// For the start asked for, the keyframe a stream copy would begin at.
    pub keyframe: Option<(f64, f64)>,
    /// The frame preview; `None` where there is no terminal to ask, or when
    /// it is switched off.
    pub preview: Option<Preview>,
    /// The terminal size in cells, as of the last resize.
    pub term: (u16, u16),
    pub log: Vec<String>,
    pub toast: Option<Toast>,
    pub quit: bool,
    /// From the config: applied to every file.
    defaults: Prefill,
    /// From the command line: applied to the first file only.
    prefill: Option<Prefill>,
    keyframe_generation: Arc<AtomicU64>,
    tx: Sender<Event>,
}

fn int_of(v: &mut Values, id: FieldId) -> Option<&mut i64> {
    match id {
        FieldId::Crf => Some(&mut v.crf),
        FieldId::Quality => Some(&mut v.quality),
        FieldId::Duration => Some(&mut v.duration),
        FieldId::Fps => Some(&mut v.fps),
        FieldId::Width => Some(&mut v.width),
        _ => None,
    }
}

fn choice_of(v: &mut Values, id: FieldId) -> Option<&mut usize> {
    match id {
        FieldId::Preset => Some(&mut v.preset),
        FieldId::Resolution => Some(&mut v.resolution),
        FieldId::Sound => Some(&mut v.sound),
        FieldId::Mode => Some(&mut v.mode),
        FieldId::Format => Some(&mut v.format),
        FieldId::Container => Some(&mut v.container),
        FieldId::Speed => Some(&mut v.speed),
        FieldId::Image => Some(&mut v.image),
        _ => None,
    }
}

fn time_of(v: &mut Values, id: FieldId) -> Option<&mut f64> {
    match id {
        FieldId::Cursor => Some(&mut v.cursor),
        FieldId::Start => Some(&mut v.start),
        FieldId::End => Some(&mut v.end),
        _ => None,
    }
}

fn copy_to_clipboard(text: &str) -> bool {
    let tools: [(&str, &[&str]); 4] =
        [("pbcopy", &[]), ("wl-copy", &[]), ("xclip", &["-selection", "clipboard"]), ("clip.exe", &[])];
    tools.iter().any(|(tool, args)| {
        let child = Command::new(tool)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else { return false };
        let wrote = child.stdin.take().is_some_and(|mut s| s.write_all(text.as_bytes()).is_ok());
        wrote && child.wait().is_ok_and(|s| s.success())
    })
}

impl App {
    pub fn new(theme: Theme, tx: Sender<Event>, op: OpKind, prefill: Prefill, config: Config) -> Self {
        App {
            theme,
            lang: config.lang,
            screen: Screen::Pick,
            overlay: None,
            picker: Picker::new(PathBuf::from(".")),
            doc: None,
            op,
            focus: 1,
            edit: None,
            dropdown: None,
            running: None,
            done: None,
            failed: None,
            keyframe: None,
            preview: None,
            term: (0, 0),
            log: vec![],
            toast: None,
            quit: false,
            defaults: config.defaults,
            prefill: Some(prefill),
            keyframe_generation: Arc::new(AtomicU64::new(0)),
            tx,
        }
    }

    pub fn tr<'a>(&self, s: &'a str) -> &'a str {
        tr(self.lang, s)
    }

    pub fn fields(&self) -> Vec<FieldSpec> {
        self.doc.as_ref().map_or(vec![], |d| op::fields(self.op, &d.values, &d.info))
    }

    pub fn command(&self) -> Option<CommandLine> {
        self.doc.as_ref().map(|d| op::build(self.op, &d.values, &d.info))
    }

    pub fn problems(&self) -> Vec<op::Problem> {
        self.doc.as_ref().map_or(vec![], |d| op::check(self.op, &d.values, &d.info))
    }

    /// A translated template with `{}` filled in.
    fn fill(&self, template: &str, arg: &str) -> String {
        self.tr(template).replace("{}", arg)
    }

    fn say(&mut self, text: impl Into<String>, error: bool) {
        self.toast =
            Some(Toast { text: text.into(), error, until: Instant::now() + Duration::from_secs(5) });
    }

    /// Makes `info` the file being worked on.
    pub fn open(&mut self, info: MediaInfo) -> Result<()> {
        let mut values = Values::defaults(&info);
        cli::apply(&self.defaults, &mut values, &info, false)?;
        if let Some(prefill) = self.prefill.take() {
            cli::apply(&prefill, &mut values, &info, true)?;
        }
        if !self.op.available(&info) {
            self.op = OpKind::ALL.into_iter().find(|o| o.available(&info)).unwrap_or(self.op);
        }
        self.doc = Some(Doc { info, values });
        self.screen = Screen::Form;
        self.focus = 1;
        self.edit = None;
        self.dropdown = None;
        self.refresh_keyframe();
        self.sync_preview();
        Ok(())
    }

    pub fn open_picker(&mut self) {
        self.picker.reload();
        self.screen = Screen::Pick;
        self.probe_picker();
    }

    fn probe_picker(&self) {
        let generation = self.picker.generation;
        let paths: Vec<PathBuf> =
            self.picker.entries.iter().filter(|e| !e.is_dir).map(|e| e.path.clone()).collect();
        let tx = self.tx.clone();
        thread::spawn(move || {
            for path in paths {
                if let Ok(info) = probe::probe(&path)
                    && tx.send(Event::Probed { generation, path, info }).is_err()
                {
                    break;
                }
            }
        });
    }

    fn refresh_keyframe(&mut self) {
        self.keyframe = None;
        let generation = self.keyframe_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let Some(doc) = &self.doc else { return };
        if self.op != OpKind::Cut || doc.values.mode != op::MODE_FAST || doc.info.video.is_none() {
            return;
        }
        let (path, at) = (doc.info.path.clone(), doc.values.start);
        let (tx, counter) = (self.tx.clone(), self.keyframe_generation.clone());
        thread::spawn(move || {
            // Wait out a held arrow key: only the last start is worth probing.
            thread::sleep(Duration::from_millis(250));
            if counter.load(Ordering::SeqCst) == generation {
                let key = probe::keyframe_before(&path, at);
                let _ = tx.send(Event::Keyframe { generation, at, key });
            }
        });
    }

    /// The time whose frame belongs on screen now, and the size to draw it at.
    pub fn preview_target(&self) -> Option<(f64, (u16, u16))> {
        let doc = self.doc.as_ref()?;
        if self.screen != Screen::Form || !self.op.has_timeline() {
            return None;
        }
        let video = doc.info.video.as_ref()?;
        let size = preview::size(self.term, video.width as f64 / video.height.max(1) as f64)?;
        let focus = self.fields().get(self.focus).map(|f| f.id);
        let v = &doc.values;
        let t = match (self.op, focus) {
            (OpKind::Cut, Some(FieldId::Start)) | (OpKind::Gif, _) => v.start,
            (OpKind::Cut, Some(FieldId::End)) => v.end,
            _ => v.cursor,
        };
        // The very last instant of a file has no frame to show.
        Some((t.min((doc.info.duration - 0.05).max(0.0)), size))
    }

    fn sync_preview(&mut self) {
        let target = self.preview_target();
        if let (Some(preview), Some((t, size)), Some(doc)) = (&mut self.preview, target, &self.doc) {
            preview.want(&doc.info.path, t, size);
        }
    }

    pub fn tick(&mut self) {
        if self.toast.as_ref().is_some_and(|t| Instant::now() > t.until) {
            self.toast = None;
        }
        if let Some(r) = &self.running {
            self.log = r.handle.log();
        }
    }

    pub fn handle(&mut self, event: Event) {
        match event {
            Event::Input(TermEvent::Key(key)) if key.kind != KeyEventKind::Release => self.key(key),
            Event::Input(TermEvent::Resize(w, h)) => self.term = (w, h),
            Event::Input(_) => {}
            Event::Probed { generation, path, info } => {
                if generation == self.picker.generation
                    && let Some(e) = self.picker.entries.iter_mut().find(|e| e.path == path)
                {
                    e.info = Some(info);
                }
            }
            Event::Keyframe { generation, at, key } => {
                if generation == self.keyframe_generation.load(Ordering::SeqCst) {
                    self.keyframe = key.map(|k| (at, k));
                }
            }
            Event::Frame { path, key, frame } => {
                if let (Some(preview), Some(frame)) = (&mut self.preview, frame) {
                    preview.accept(&path, key, frame);
                }
            }
            Event::Run(RunEvent::Progress(p)) => {
                if let Some(r) = &mut self.running {
                    r.progress = p;
                }
            }
            Event::Run(RunEvent::Finished { code, cancelled }) => self.finished(code, cancelled),
        }
        self.sync_preview();
    }

    fn finished(&mut self, code: Option<i32>, cancelled: bool) {
        let Some(run) = self.running.take() else { return };
        self.log = run.handle.log();
        let name = run.output.to_string_lossy().into_owned();
        if cancelled {
            let removed = std::fs::remove_file(&run.output).is_ok();
            self.screen = Screen::Form;
            let text = if removed {
                self.fill("Cancelled. The unfinished {} was removed.", &name)
            } else {
                self.tr("Cancelled.").to_string()
            };
            self.say(text, false);
        } else if code == Some(0) {
            let Some(doc) = &self.doc else { return };
            let after = std::fs::metadata(&run.output).map(|m| m.len()).unwrap_or(0);
            let v = &doc.values;
            let cut = (self.op == OpKind::Cut).then(|| {
                // A stream copy keeps the end and starts earlier; the length
                // of the result says by how much.
                let got = probe::probe(&run.output).map(|i| i.duration).unwrap_or(v.end - v.start);
                let got_start = if v.mode == op::MODE_FAST { (v.end - got).max(0.0) } else { v.start };
                CutResult { start: v.start, end: v.end, got_start }
            });
            self.done = Some(Done {
                op: self.op,
                cmd: run.cmd,
                output: run.output,
                before: doc.info.size,
                after,
                total: run.total,
                cut,
            });
            self.screen = Screen::Done;
        } else {
            let removed = std::fs::remove_file(&run.output).is_ok().then_some(name);
            let lines: Vec<String> = self.log.iter().rev().take(3).rev().cloned().collect();
            self.failed = Some(Failed {
                cmd: run.cmd,
                at: run.progress.out_time,
                total: run.total,
                code,
                lines,
                removed,
            });
            self.screen = Screen::Failed;
        }
    }

    fn start(&mut self) {
        if let Some(p) = self.problems().into_iter().next() {
            let text = self.fill(p.text, &p.arg);
            self.say(text, true);
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        let output = op::output_path(self.op, &doc.values, &doc.info);
        if output.exists() && !doc.values.overwrite {
            self.screen = Screen::Exists;
            return;
        }
        let cmd = op::build(self.op, &doc.values, &doc.info);
        let total = op::result_duration(self.op, &doc.values, &doc.info);
        doc.values.overwrite = false;
        let tx = self.tx.clone();
        let spawned = run::spawn(&op::argv(&cmd), move |e| {
            let _ = tx.send(Event::Run(e));
        });
        match spawned {
            Ok(handle) => {
                self.log.clear();
                self.running =
                    Some(Running { handle, cmd, progress: Progress::default(), total, output });
                self.screen = Screen::Running;
            }
            Err(e) => {
                self.screen = Screen::Form;
                self.say(format!("{e:#}"), true);
            }
        }
    }

    fn copy_command(&mut self) {
        let cmd = match self.screen {
            Screen::Done => self.done.as_ref().map(|d| d.cmd.clone()),
            Screen::Failed => self.failed.as_ref().map(|f| f.cmd.clone()),
            Screen::Running => self.running.as_ref().map(|r| r.cmd.clone()),
            _ => self.command(),
        };
        let Some(cmd) = cmd else { return };
        if copy_to_clipboard(&op::shell(&cmd)) {
            let text = self.tr("The command is in the clipboard.").to_string();
            self.say(text, false);
        } else {
            let text = self.tr("No clipboard tool here: pbcopy, wl-copy or xclip is needed.").to_string();
            self.say(text, true);
        }
    }

    fn switch_language(&mut self) {
        self.lang = if self.lang == Lang::En { Lang::Ru } else { Lang::En };
        if let Err(e) = config::save_lang(self.lang) {
            self.say(format!("{e:#}"), true);
        }
    }

    fn key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            match &mut self.running {
                Some(r) => r.handle.cancel(),
                None => self.quit = true,
            }
            return;
        }
        if let Some(overlay) = self.overlay {
            match (overlay, key.code) {
                (_, KeyCode::Esc) | (Overlay::Keys, KeyCode::Char('?')) => self.overlay = None,
                (Overlay::Keys, KeyCode::Tab) => self.switch_language(),
                (Overlay::Log, KeyCode::Char(c)) if latin_key(c) == 'l' => self.overlay = None,
                _ => {}
            }
            return;
        }
        match self.screen {
            Screen::Pick => self.key_pick(key),
            Screen::Form => self.key_form(key),
            Screen::Exists => match hotkey(&key) {
                Some('o') => {
                    if let Some(doc) = &mut self.doc {
                        doc.values.overwrite = true;
                    }
                    self.start();
                }
                _ if key.code == KeyCode::Esc => self.screen = Screen::Form,
                _ => {}
            },
            Screen::Running => match hotkey(&key) {
                Some('l') => self.overlay = Some(Overlay::Log),
                Some('c') => self.copy_command(),
                _ if key.code == KeyCode::Esc => {
                    if let Some(r) = &mut self.running {
                        r.handle.cancel();
                    }
                }
                _ => {}
            },
            Screen::Done => {
                let moved = self.done.as_ref().and_then(|d| d.cut.as_ref()).is_some_and(cut_moved);
                match hotkey(&key) {
                    Some('t') if moved => {
                        if let Some(doc) = &mut self.doc {
                            doc.values.mode = op::MODE_EXACT;
                        }
                        self.refresh_keyframe();
                        self.start();
                    }
                    Some('f') => self.open_picker(),
                    Some('c') => self.copy_command(),
                    Some('l') => self.overlay = Some(Overlay::Log),
                    Some('q') => self.quit = true,
                    Some('n') => self.screen = Screen::Form,
                    _ if key.code == KeyCode::Enter => self.screen = Screen::Form,
                    _ if key.code == KeyCode::Esc => self.quit = true,
                    _ => {}
                }
            }
            Screen::Failed => match hotkey(&key) {
                Some('l') => self.overlay = Some(Overlay::Log),
                Some('c') => self.copy_command(),
                _ if key.code == KeyCode::Esc || key.code == KeyCode::Enter => {
                    self.screen = Screen::Form
                }
                _ => {}
            },
        }
    }

    fn key_pick(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.picker.move_by(-1),
            KeyCode::Down => self.picker.move_by(1),
            KeyCode::PageUp => self.picker.move_by(-10),
            KeyCode::PageDown => self.picker.move_by(10),
            KeyCode::Backspace => {
                self.picker.filter.pop();
                self.picker.selected = 0;
            }
            KeyCode::Char(c) => {
                self.picker.filter.push(c);
                self.picker.selected = 0;
            }
            KeyCode::Esc if !self.picker.filter.is_empty() => {
                self.picker.filter.clear();
                self.picker.selected = 0;
            }
            KeyCode::Esc if self.doc.is_some() => self.screen = Screen::Form,
            KeyCode::Esc => self.quit = true,
            KeyCode::Enter => {
                let Some(entry) = self.picker.current().cloned() else { return };
                if entry.is_dir {
                    self.picker.enter_dir(entry.path);
                    self.probe_picker();
                    return;
                }
                let opened = match entry.info {
                    Some(info) => Ok(info),
                    None => probe::probe(&entry.path),
                }
                .and_then(|info| self.open(info));
                if let Err(e) = opened {
                    self.say(format!("{e:#}"), true);
                }
            }
            _ => {}
        }
    }

    fn focused(&self) -> Option<FieldSpec> {
        self.fields().into_iter().nth(self.focus)
    }

    fn move_focus(&mut self, delta: isize) {
        self.commit_edit();
        let fields = self.fields();
        let mut i = self.focus as isize;
        loop {
            i += delta;
            if i < 0 || i >= fields.len() as isize {
                return;
            }
            if fields[i as usize].focusable() {
                self.focus = i as usize;
                return;
            }
        }
    }

    fn switch_op(&mut self, delta: isize) {
        self.commit_edit();
        let Some(doc) = &self.doc else { return };
        let ops: Vec<OpKind> = OpKind::ALL.into_iter().filter(|o| o.available(&doc.info)).collect();
        let Some(at) = ops.iter().position(|o| *o == self.op) else { return };
        self.op = ops[(at as isize + delta).rem_euclid(ops.len() as isize) as usize];
        self.focus = 1;
        self.refresh_keyframe();
    }

    fn changed(&mut self, id: FieldId) {
        if matches!(id, FieldId::Start | FieldId::Mode) {
            self.refresh_keyframe();
        }
    }

    fn adjust(&mut self, dir: i64, big: bool) {
        let (Some(spec), Some(doc)) = (self.focused(), &mut self.doc) else { return };
        let v = &mut doc.values;
        match &spec.kind {
            Kind::Int { min, max, step, big: big_step, .. } => {
                if let Some(n) = int_of(v, spec.id) {
                    *n = (*n + dir * if big { *big_step } else { *step }).clamp(*min, *max);
                }
            }
            Kind::Choice { items, .. } => {
                if let Some(c) = choice_of(v, spec.id) {
                    *c = (*c as i64 + dir).clamp(0, items.len() as i64 - 1) as usize;
                }
            }
            Kind::Time => {
                let duration = doc.info.duration;
                if let Some(t) = time_of(v, spec.id) {
                    *t = (*t + dir as f64 * if big { 10.0 } else { 1.0 }).clamp(0.0, duration);
                }
            }
            _ => return,
        }
        self.changed(spec.id);
    }

    fn step_frame(&mut self, dir: f64) {
        let (Some(spec), Some(doc)) = (self.focused(), &mut self.doc) else { return };
        let frame = 1.0 / doc.info.video.as_ref().map_or(25.0, |v| v.fps);
        let duration = doc.info.duration;
        if let Some(t) = time_of(&mut doc.values, spec.id) {
            // Keep the timestamp to the millisecond ffmpeg will be given.
            *t = ((*t + dir * frame).clamp(0.0, duration) * 1000.0).round() / 1000.0;
            self.changed(spec.id);
        }
    }

    /// `i` and `o`: the cursor becomes the start or the end of the piece.
    fn mark(&mut self, id: FieldId) {
        let Some(doc) = &mut self.doc else { return };
        if self.op != OpKind::Cut {
            return;
        }
        let cursor = doc.values.cursor;
        if let Some(t) = time_of(&mut doc.values, id) {
            *t = cursor;
        }
        self.changed(id);
    }

    fn reset_field(&mut self) {
        let (Some(spec), Some(doc)) = (self.focused(), &mut self.doc) else { return };
        let mut defaults = Values::defaults(&doc.info);
        let _ = cli::apply(&self.defaults, &mut defaults, &doc.info, false);
        let v = &mut doc.values;
        if let (Some(a), Some(b)) = (int_of(v, spec.id), int_of(&mut defaults, spec.id)) {
            *a = *b;
        }
        if let (Some(a), Some(b)) = (choice_of(v, spec.id), choice_of(&mut defaults, spec.id)) {
            *a = *b;
        }
        if let (Some(a), Some(b)) = (time_of(v, spec.id), time_of(&mut defaults, spec.id)) {
            *a = *b;
        }
        if spec.id == FieldId::Output {
            v.output = None;
        }
        self.changed(spec.id);
    }

    fn commit_edit(&mut self) {
        let Some(text) = self.edit.take() else { return };
        let (Some(spec), Some(doc)) = (self.focused(), &mut self.doc) else { return };
        let v = &mut doc.values;
        let text = text.trim();
        let mut complaint = None;
        match &spec.kind {
            Kind::Int { min, max, .. } => match text.parse::<i64>() {
                Ok(n) => {
                    if let Some(slot) = int_of(v, spec.id) {
                        *slot = n.clamp(*min, *max);
                    }
                }
                Err(_) => complaint = Some("{} is not a number."),
            },
            Kind::Time => match parse_time(text) {
                Some(t) => {
                    let duration = doc.info.duration;
                    if let Some(slot) = time_of(v, spec.id) {
                        *slot = t.min(duration);
                    }
                }
                None => complaint = Some("{} is not a time; write 38, 0:38 or 00:00:38."),
            },
            Kind::Text => v.output = (!text.is_empty()).then(|| text.to_string()),
            _ => {}
        }
        if let Some(template) = complaint {
            let message = self.fill(template, text);
            self.say(message, true);
        }
        self.changed(spec.id);
    }

    fn key_form(&mut self, key: KeyEvent) {
        let Some(spec) = self.focused() else { return };

        if let Some(at) = self.dropdown {
            let Kind::Choice { items, .. } = &spec.kind else { return };
            match key.code {
                KeyCode::Up => self.dropdown = Some(at.saturating_sub(1)),
                KeyCode::Down => self.dropdown = Some((at + 1).min(items.len() - 1)),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(c) = self.doc.as_mut().and_then(|d| choice_of(&mut d.values, spec.id)) {
                        *c = at;
                    }
                    self.dropdown = None;
                    self.changed(spec.id);
                }
                KeyCode::Esc => self.dropdown = None,
                _ => {}
            }
            return;
        }

        if let Some(buffer) = &mut self.edit {
            match key.code {
                KeyCode::Char(c) => {
                    let fits = match spec.kind {
                        Kind::Int { .. } => c.is_ascii_digit(),
                        Kind::Time => c.is_ascii_digit() || c == ':' || c == '.',
                        _ => true,
                    };
                    if fits {
                        buffer.push(c);
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Enter => self.commit_edit(),
                KeyCode::Esc => self.edit = None,
                KeyCode::Up => self.move_focus(-1),
                KeyCode::Down => self.move_focus(1),
                KeyCode::Tab => self.switch_op(1),
                KeyCode::BackTab => self.switch_op(-1),
                _ => {}
            }
            return;
        }

        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Tab => self.switch_op(1),
            KeyCode::BackTab => self.switch_op(-1),
            KeyCode::Up => self.move_focus(-1),
            KeyCode::Down => self.move_focus(1),
            KeyCode::Left => self.adjust(-1, shift),
            KeyCode::Right => self.adjust(1, shift),
            KeyCode::Backspace => self.reset_field(),
            KeyCode::Esc => self.quit = true,
            KeyCode::Enter if spec.id == FieldId::File => self.open_picker(),
            KeyCode::Enter => self.start(),
            KeyCode::Char(c) if matches!(spec.kind, Kind::Text) => self.edit = Some(c.to_string()),
            KeyCode::Char(c) if c.is_ascii_digit() && matches!(spec.kind, Kind::Int { .. } | Kind::Time) => {
                self.edit = Some(c.to_string())
            }
            KeyCode::Char(' ') => {
                if let Kind::Choice { items, selected, inline } = &spec.kind {
                    if *inline {
                        if let Some(c) = self.doc.as_mut().and_then(|d| choice_of(&mut d.values, spec.id)) {
                            *c = (*c + 1) % items.len();
                        }
                        self.changed(spec.id);
                    } else {
                        self.dropdown = Some(*selected);
                    }
                }
            }
            KeyCode::Char('?') => self.overlay = Some(Overlay::Keys),
            KeyCode::Char(c) => match latin_key(c) {
                'c' => self.copy_command(),
                'f' => self.open_picker(),
                'q' => self.quit = true,
                'i' => self.mark(FieldId::Start),
                'o' => self.mark(FieldId::End),
                'H' => self.adjust(-1, true),
                'L' => self.adjust(1, true),
                ',' => self.step_frame(-1.0),
                '.' => self.step_frame(1.0),
                _ => {}
            },
            _ => {}
        }
    }
}

fn hotkey(key: &KeyEvent) -> Option<char> {
    match key.code {
        KeyCode::Char(c) => Some(latin_key(c)),
        _ => None,
    }
}

/// True when the stream copy started noticeably earlier than asked.
pub fn cut_moved(c: &CutResult) -> bool {
    c.start - c.got_start > 0.3
}

pub struct Start {
    pub op: Option<OpKind>,
    pub info: Option<MediaInfo>,
    pub prefill: Prefill,
    pub config: Config,
}

pub fn run(start: Start) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut app = App::new(
        Theme::detect(),
        tx.clone(),
        start.op.unwrap_or(OpKind::Compress),
        start.prefill,
        start.config,
    );
    match start.info {
        // Before the terminal is taken over: a bad option should read as a
        // plain error, not flash by inside the form.
        Some(info) => app.open(info)?,
        None => app.open_picker(),
    }

    let mut terminal = ratatui::init();
    // The terminal is asked which graphics it speaks. That reads the answer
    // from stdin, so it has to happen before the input thread starts.
    if std::env::var("KADR_PREVIEW").as_deref() != Ok("off") {
        let graphics = GraphicsPicker::from_query_stdio().unwrap_or_else(|_| GraphicsPicker::halfblocks());
        app.preview = Some(Preview::new(graphics, tx.clone()));
    }
    if let Ok(size) = terminal.size() {
        app.term = (size.width, size.height);
    }
    app.sync_preview();

    thread::spawn(move || {
        while let Ok(e) = event::read() {
            if tx.send(Event::Input(e)).is_err() {
                break;
            }
        }
    });

    let result = (|| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &app))?;
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(e) => app.handle(e),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            while let Ok(e) = rx.try_recv() {
                app.handle(e);
            }
            app.tick();
        }
        Ok(())
    })();
    ratatui::restore();
    // Leaving mid-run must not leave ffmpeg writing behind our back.
    if let Some(mut r) = app.running.take() {
        r.handle.cancel();
        thread::sleep(Duration::from_millis(300));
        let _ = std::fs::remove_file(&r.output);
    }
    result
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::op::testing::lecture;

    pub fn app() -> App {
        let (tx, _rx) = mpsc::channel();
        // The receiver is dropped: sends from background threads just fail.
        let mut app = App::new(Theme::ansi(), tx, OpKind::Compress, Prefill::default(), Config::default());
        app.open(lecture()).unwrap();
        app
    }

    pub fn press(app: &mut App, code: KeyCode) {
        app.handle(Event::Input(TermEvent::Key(KeyEvent::new(code, KeyModifiers::NONE))));
    }

    fn shown(app: &App) -> String {
        op::shell(&app.command().unwrap())
    }

    #[test]
    fn arrows_change_the_focused_value_and_the_command_follows() {
        let mut app = app();
        press(&mut app, KeyCode::Right);
        assert!(shown(&app).contains("-crf 24"));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Right);
        assert!(shown(&app).contains("-preset slow"));
    }

    #[test]
    fn typing_digits_replaces_the_value_on_enter() {
        let mut app = app();
        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('9'));
        assert!(shown(&app).contains("-crf 23"), "not a value until committed");
        press(&mut app, KeyCode::Enter);
        assert!(shown(&app).contains("-crf 19"));
        assert_eq!(app.screen, Screen::Form, "enter committed the edit, it did not run");
    }

    #[test]
    fn the_list_opens_with_space_and_esc_keeps_the_old_value() {
        let mut app = app();
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(app.dropdown, Some(5));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Esc);
        assert!(shown(&app).contains("-preset medium"));
        press(&mut app, KeyCode::Char(' '));
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter);
        assert!(shown(&app).contains("-preset slow"));
    }

    #[test]
    fn tab_walks_through_every_operation_and_back() {
        let mut app = app();
        let mut seen = vec![app.op];
        for _ in 0..OpKind::ALL.len() {
            press(&mut app, KeyCode::Tab);
            seen.push(app.op);
        }
        assert_eq!(&seen[..OpKind::ALL.len()], &OpKind::ALL);
        assert_eq!(seen.last(), Some(&OpKind::Compress));
    }

    #[test]
    fn the_cursor_marks_the_piece_with_i_and_o() {
        let mut app = app();
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.fields()[app.focus].id, FieldId::Cursor);
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('L'));
        }
        press(&mut app, KeyCode::Char('i'));
        press(&mut app, KeyCode::Char('L'));
        press(&mut app, KeyCode::Char('щ')); // the key that types o
        assert!(shown(&app).contains("-ss 00:00:30 -to 00:00:40"), "{}", shown(&app));
    }

    #[test]
    fn time_keys_work_in_a_cyrillic_layout() {
        let mut app = app();
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char('Д')); // the key that types L
        assert!(shown(&app).contains("-ss 00:00:10"));
        press(&mut app, KeyCode::Char('ю')); // the key that types .
        assert!(shown(&app).contains("-ss 00:00:10.040"));
    }

    #[test]
    fn the_preview_follows_the_field_in_focus() {
        let mut app = app();
        app.term = (120, 40);
        press(&mut app, KeyCode::Tab);
        {
            let v = &mut app.doc.as_mut().unwrap().values;
            (v.cursor, v.start, v.end) = (5.0, 38.0, 72.0);
        }
        assert_eq!(app.preview_target().map(|t| t.0), Some(5.0));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.preview_target().map(|t| t.0), Some(38.0));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.preview_target().map(|t| t.0), Some(72.0));
        app.op = OpKind::Compress;
        assert_eq!(app.preview_target(), None);
    }

    #[test]
    fn a_bad_piece_does_not_run() {
        let mut app = app();
        press(&mut app, KeyCode::Tab);
        app.doc.as_mut().unwrap().values.end = 0.0;
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Form);
        assert!(app.toast.as_ref().unwrap().error);
    }

    #[test]
    fn backspace_returns_the_default() {
        let mut app = app();
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Backspace);
        assert!(shown(&app).contains("-crf 23"));
    }

    #[test]
    fn config_defaults_fill_the_form_and_are_what_backspace_returns_to() {
        let (tx, _rx) = mpsc::channel();
        let config = Config { defaults: Prefill { crf: Some(27), ..Default::default() }, ..Default::default() };
        let mut app = App::new(Theme::ansi(), tx, OpKind::Compress, Prefill::default(), config);
        app.open(lecture()).unwrap();
        assert!(shown(&app).contains("-crf 27"));
        press(&mut app, KeyCode::Left);
        press(&mut app, KeyCode::Backspace);
        assert!(shown(&app).contains("-crf 27"));
    }

    #[test]
    fn audio_only_files_open_on_audio() {
        let (tx, _rx) = mpsc::channel();
        let mut app = App::new(Theme::ansi(), tx, OpKind::Compress, Prefill::default(), Config::default());
        let mut info = lecture();
        info.video = None;
        app.open(info).unwrap();
        assert_eq!(app.op, OpKind::Audio);
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.op, OpKind::Audio, "nothing else to switch to");
    }
}
