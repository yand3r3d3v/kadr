//! Drawing. Everything here reads the state and changes nothing.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui_image::Image;

use crate::app::{App, Overlay, Screen, cut_moved};
use crate::command::{self, Look};
use crate::op::{self, CommandLine, FieldId, FieldSpec, Kind, OpKind};
use crate::probe::MediaInfo;
use crate::text::{Lang, tr};
use crate::theme::Theme;
use crate::util::{fmt_clock, fmt_size, fmt_ts};

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
const LABEL: usize = 12;
const MARK: &str = "⌜ kadr ⌟";

fn s(text: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(text.into(), style)
}

fn plain() -> Style {
    Style::default()
}

fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

fn width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

fn pad(text: &str, to: usize) -> String {
    format!("{text:<to$}")
}

fn clip(text: &str, to: usize) -> String {
    if text.chars().count() <= to {
        text.to_string()
    } else {
        text.chars().take(to.saturating_sub(1)).chain(['…']).collect()
    }
}

/// `left`, then `right` pushed to the right edge if both fit.
fn row(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, w: usize) -> Line<'static> {
    let (l, r) = (width(&left), width(&right));
    if r > 0 && l + 2 + r <= w {
        left.push(Span::raw(" ".repeat(w - l - r)));
        left.extend(right);
    }
    Line::from(left)
}

/// A cursor walking down the screen, one row at a time.
struct Page<'a, 'b> {
    f: &'a mut Frame<'b>,
    th: Theme,
    lang: Lang,
    x: u16,
    y: u16,
    w: u16,
    bottom: u16,
}

impl Page<'_, '_> {
    fn t<'s>(&self, text: &'s str) -> &'s str {
        tr(self.lang, text)
    }

    fn line(&mut self, line: Line<'static>) {
        if self.y < self.bottom {
            self.f.render_widget(Paragraph::new(line), Rect::new(self.x, self.y, self.w, 1));
        }
        self.y += 1;
    }

    fn gap(&mut self) {
        self.y += 1;
    }

    fn text(&mut self, text: impl Into<String>, style: Style) {
        self.line(Line::from(s(text, style)));
    }

    fn header(&mut self, tabs: &[(&str, bool, bool)]) {
        let names: Vec<&str> = tabs.iter().map(|(n, _, _)| self.t(n)).collect();
        // The mark keeps its column while the tabs fit; in a narrow window
        // it gives up the padding first.
        let tabs_width: usize = names.iter().map(|n| n.chars().count() + 2).sum();
        let mark = (self.w as usize).saturating_sub(tabs_width).clamp(MARK.chars().count() + 2, 24);
        let mut spans = vec![s(pad(MARK, mark), bold())];
        for ((_, active, available), name) in tabs.iter().zip(names) {
            let style = if *active {
                bold().add_modifier(Modifier::UNDERLINED)
            } else if *available {
                fg(self.th.dim)
            } else {
                fg(self.th.line)
            };
            spans.push(s(name, style));
            spans.push(Span::raw("  "));
        }
        self.line(Line::from(spans));
        self.rule();
    }

    fn title(&mut self, title: &str) {
        let title = self.t(title).to_string();
        self.line(Line::from(vec![s(pad(MARK, 24), bold()), s(title, bold().add_modifier(Modifier::UNDERLINED))]));
        self.rule();
    }

    fn rule(&mut self) {
        self.text("─".repeat(self.w as usize), fg(self.th.line));
    }

    fn framed(&mut self, title: &str, color: Color, title_style: Style, lines: Vec<Line<'static>>) {
        let h = lines.len() as u16 + 2;
        if self.y + h <= self.bottom {
            let block = Block::bordered()
                .border_style(fg(color))
                .title(s(format!(" {} ", self.t(title)), title_style))
                .padding(Padding::horizontal(1));
            self.f.render_widget(
                Paragraph::new(lines).block(block),
                Rect::new(self.x, self.y, self.w, h),
            );
        }
        self.y += h;
    }

    fn command(&mut self, cmd: &CommandLine, look: Look) {
        let lines = command::lines(cmd, self.w as usize - 4, look, &self.th);
        let (ice, line) = (self.th.ice, self.th.line);
        self.framed("command", line, fg(ice), lines);
    }

    fn error(&mut self, lines: Vec<Line<'static>>) {
        let carmine = self.th.carmine;
        self.framed("error", carmine, bold().fg(carmine), lines);
    }

    fn keys(&mut self, keys: &[(&str, &str)]) {
        // A language with longer words must not push `esc` off the row: the
        // keys that the form itself makes obvious give way first.
        let mut keys = keys.to_vec();
        let fits = |keys: &[(&str, &str)]| {
            let cells: usize =
                keys.iter().map(|(k, what)| k.chars().count() + 1 + self.t(what).chars().count()).sum();
            cells + 3 * keys.len().saturating_sub(1) <= self.w as usize
        };
        for dropped in ["copy", "field", "value"] {
            if fits(&keys) {
                break;
            }
            keys.retain(|(_, what)| *what != dropped);
        }
        let mut spans = vec![];
        for (key, what) in &keys {
            let key_style = if *what == "overwrite" { bold().fg(self.th.carmine) } else { bold() };
            spans.push(s(*key, key_style));
            spans.push(s(format!(" {}   ", self.t(what)), fg(self.th.dim)));
        }
        let y = self.bottom;
        self.f.render_widget(Paragraph::new(Line::from(spans)), Rect::new(self.x, y, self.w, 1));
    }

    /// `┣━━━◆┈┈┈┫  00:41 / 02:10`
    fn ruler(&mut self, frac: f64, marker: Option<Span<'static>>, at: f64, total: f64, live: bool) {
        let th = self.th;
        let tail = format!("  {} / {}", fmt_clock(at), fmt_clock(total));
        let cells = (self.w as usize).saturating_sub(2 + tail.chars().count() + marker.is_some() as usize);
        let filled = ((frac.clamp(0.0, 1.0) * cells as f64).round() as usize).min(cells);
        let mut spans = vec![s("┣", fg(th.ice)), s("━".repeat(filled), fg(th.ice))];
        spans.extend(marker);
        spans.push(s("┈".repeat(cells - filled), fg(th.line)));
        spans.push(s("┫", fg(th.ice)));
        if live {
            spans.push(s(format!("  {}", fmt_clock(at)), plain()));
            spans.push(s(format!(" / {}", fmt_clock(total)), fg(th.dim)));
        } else {
            spans.push(s(tail, fg(th.dim)));
        }
        self.line(Line::from(spans));
    }

    /// The whole file with the chosen piece marked: `┣┈┈[━━◆━]┈┈┈┫`
    fn piece(&mut self, start: f64, end: f64, total: f64, keyframe: Option<f64>, cursor: Option<f64>) {
        let th = self.th;
        let cells = self.w as usize - 2;
        let at = |t: f64| ((t / total.max(0.001)).clamp(0.0, 1.0) * (cells - 1) as f64).round() as usize;
        let a = at(start).min(cells - 2);
        let b = at(end).max(a + 1).min(cells - 1);
        let key = keyframe.map(at).filter(|k| *k < a);
        let cursor = cursor.map(at);
        let mut spans = vec![s("┣", fg(th.ice))];
        for i in 0..cells {
            spans.push(match i {
                i if i == a => s("[", bold().fg(th.ochre)),
                i if i == b => s("]", bold().fg(th.ochre)),
                i if Some(i) == cursor => s("◆", bold().fg(th.ochre)),
                i if Some(i) == key => s("╎", fg(th.ochre)),
                i if i > a && i < b => s("━", fg(th.ice)),
                // The stretch a stream copy adds in front of the piece.
                i if key.is_some_and(|k| i > k && i < a) => s("━", fg(th.line)),
                _ => s("┈", fg(th.line)),
            });
        }
        spans.push(s("┫", fg(th.ice)));
        self.line(Line::from(spans));

        // Labels under the marks, dropped where they would collide.
        let mut labels = vec![' '; self.w as usize];
        let mut styles = vec![fg(th.dim); self.w as usize];
        let mut place = |col: usize, text: String, style: Style| {
            let n = text.chars().count();
            let col = col.min(labels.len().saturating_sub(n));
            let free = (col.saturating_sub(1)..(col + n + 1).min(labels.len())).all(|i| labels[i] == ' ');
            if free {
                for (i, c) in text.chars().enumerate() {
                    labels[col + i] = c;
                    styles[col + i] = style;
                }
            }
        };
        place(0, "00:00".into(), fg(th.dim));
        place(usize::MAX, fmt_clock(total), fg(th.dim));
        place(a + 1, fmt_clock(start), plain());
        place(b + 1, fmt_clock(end), plain());
        self.line(Line::from(
            labels.iter().zip(styles).map(|(c, st)| s(c.to_string(), st)).collect::<Vec<_>>(),
        ));
    }
}

fn summary(info: &MediaInfo, lang: Lang) -> String {
    let mut parts = vec![fmt_clock(info.duration)];
    match (&info.video, &info.audio) {
        (Some(v), _) => parts.push(format!("{}×{}", v.width, v.height)),
        (None, Some(a)) => {
            let rate = a.bitrate.map(|b| format!(" {}k", b / 1000)).unwrap_or_default();
            parts.push(format!("{} {}{rate}", tr(lang, "audio"), a.codec));
        }
        _ => {}
    }
    parts.push(fmt_size(info.size));
    parts.join(", ")
}

fn field_rows(app: &App, fields: &[FieldSpec], info: &MediaInfo, frozen: bool, w: usize) -> Vec<Line<'static>> {
    let th = &app.theme;
    let t = |text: &'static str| app.tr(text);
    let mut lines = vec![];
    for (i, spec) in fields.iter().enumerate() {
        let focused = !frozen && app.screen == Screen::Form && i == app.focus;
        let mut left = vec![if focused { s("▎ ", fg(th.ochre)) } else { Span::raw("  ") }];
        let label_style = if frozen {
            fg(th.dim)
        } else if !spec.enabled {
            fg(th.line)
        } else if focused {
            bold()
        } else {
            fg(th.ice)
        };
        left.push(s(pad(t(spec.label), LABEL), label_style));

        let mut right = vec![];
        if !spec.enabled {
            left.push(s(t(spec.hint), fg(th.line)));
        } else if frozen {
            let inline = matches!(spec.kind, Kind::Choice { inline: true, .. });
            // A preset called "fast" is a name, not a word to translate.
            let value = if inline { app.tr(&spec.value) } else { &spec.value };
            left.push(s(value.to_string(), plain()));
        } else {
            match &spec.kind {
                Kind::Choice { items, selected, inline: true } => {
                    for (n, item) in items.iter().enumerate() {
                        let style = match (n == *selected, focused) {
                            (true, true) => bold().fg(th.ochre).add_modifier(Modifier::UNDERLINED),
                            (true, false) => fg(th.ochre),
                            _ => fg(th.dim),
                        };
                        left.push(s(app.tr(&item.label).to_string(), style));
                        left.push(Span::raw("  "));
                    }
                    if focused {
                        let hint = items[*selected].hint;
                        right.push(s(t(if hint.is_empty() { spec.hint } else { hint }), fg(th.dim)));
                    }
                }
                Kind::Info => left.push(s(spec.value.clone(), plain())),
                kind => {
                    let editing = focused && app.edit.is_some();
                    let value = if editing { app.edit.clone().unwrap_or_default() } else { spec.value.clone() };
                    let risky = matches!(kind, Kind::Int { soft, .. }
                        if value.parse::<i64>().is_ok_and(|n| n < soft.0 || n > soft.1));
                    let color = if risky { th.carmine } else { th.ochre };
                    left.push(s(value, if focused { bold().fg(color) } else { fg(color) }));
                    if focused && matches!(kind, Kind::Int { .. } | Kind::Time | Kind::Text) {
                        left.push(s("▏", fg(th.ochre)));
                    }
                    if editing {
                        right.push(s(t("enter takes it, esc drops it"), fg(th.dim)));
                    } else if focused {
                        right.push(s(t(spec.hint), fg(th.dim)));
                    }
                }
            }
        }
        if spec.id == FieldId::File && !focused {
            right = vec![s(summary(info, app.lang), fg(th.dim))];
        }
        lines.push(row(left, right, w));

        if let (true, Some(at), Kind::Choice { items, .. }) = (focused, app.dropdown, &spec.kind) {
            // A window of five around the highlighted item.
            let first = at.saturating_sub(2).min(items.len().saturating_sub(5));
            for (n, item) in items.iter().enumerate().skip(first).take(5) {
                let here = n == at;
                let mut spans = vec![s("▎ ", fg(th.ochre)), Span::raw(" ".repeat(LABEL))];
                let name = format!("{} {}", if here { "▸" } else { " " }, pad(&item.label, 11));
                spans.push(s(name, if here { bold().fg(th.ochre) } else { fg(th.dim) }));
                spans.push(s(t(item.hint), if here { plain() } else { fg(th.dim) }));
                lines.push(Line::from(spans));
            }
        }
    }
    lines
}

fn verb(op: OpKind) -> &'static str {
    match op {
        OpKind::Compress => "Compressing",
        OpKind::Cut => "Cutting",
        OpKind::Gif => "Making a gif from",
        OpKind::Audio => "Taking the audio from",
        OpKind::Convert => "Repacking",
        OpKind::Speed => "Changing the speed of",
        OpKind::Frame => "Taking a frame from",
    }
}

/// The fields, with the frame of the video to their left when there is one.
fn form_body(page: &mut Page, app: &App, fields: &[FieldSpec], info: &MediaInfo) {
    let th = page.th;
    let target = app.preview.as_ref().zip(app.preview_target());
    let Some((preview, (_, (cols, rows)))) = target else {
        for line in field_rows(app, fields, info, false, page.w as usize) {
            page.line(line);
        }
        return;
    };

    let top = page.y;
    let area = Rect::new(page.x, top, cols, rows);
    match preview.frame() {
        Some((frame, t)) => {
            page.f.render_widget(Image::new(frame), area);
            let fps = info.video.as_ref().map_or(25.0, |v| v.fps);
            let count = format!(
                "{} {} {} {}",
                page.t("frame"),
                (t * fps).round() as u64 + 1,
                page.t("of"),
                (info.duration * fps).round().max(1.0) as u64
            );
            let caption = row(vec![s(fmt_ts(t), bold())], vec![s(count, fg(th.dim))], cols as usize);
            page.f.render_widget(Paragraph::new(caption), Rect::new(page.x, top + rows, cols, 1));
        }
        None => {
            let waiting = Paragraph::new(s(page.t("reading the frame…"), fg(th.dim)));
            page.f.render_widget(waiting, Rect::new(page.x, top + rows / 2, cols, 1));
        }
    }

    let (fx, fw) = (page.x + cols + 2, page.w - cols - 2);
    let lines = field_rows(app, fields, info, false, fw as usize);
    let used = lines.len() as u16;
    for (i, line) in lines.into_iter().enumerate() {
        let y = top + i as u16;
        if y < page.bottom {
            page.f.render_widget(Paragraph::new(line), Rect::new(fx, y, fw, 1));
        }
    }
    page.y = top + used.max(rows + 1);
}

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let th = app.theme;
    let t = |text: &'static str| app.tr(text);
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let lines = vec![
            Line::from(s(MARK, bold())),
            Line::default(),
            Line::from(format!("{} {MIN_WIDTH}×{MIN_HEIGHT},", t("The window is too small: it needs"))),
            Line::from(vec![
                Span::raw(format!("{} ", t("it is"))),
                s(format!("{}×{}", area.width, area.height), bold().fg(th.carmine)),
                Span::raw(format!(" {}", t("now."))),
            ]),
            Line::default(),
            Line::from(s(t("Make it bigger and the form comes back."), fg(th.dim))),
        ];
        f.render_widget(Paragraph::new(lines), area);
        return;
    }

    let mut page = Page {
        f,
        th,
        lang: app.lang,
        x: area.x + 2,
        y: area.y + 1,
        w: area.width - 4,
        bottom: area.y + area.height - 2,
    };
    let w = page.w as usize;

    match app.overlay {
        Some(Overlay::Keys) => return keys_page(&mut page, app),
        Some(Overlay::Log) => return log_page(&mut page, app),
        None => {}
    }

    let tabs: Vec<(&str, bool, bool)> = OpKind::ALL
        .iter()
        .map(|o| (o.name(), *o == app.op, app.doc.as_ref().is_none_or(|d| o.available(&d.info))))
        .collect();
    page.header(&tabs);

    if app.screen == Screen::Pick {
        pick_page(&mut page, app);
    } else if let Some(doc) = &app.doc {
        let fields = app.fields();
        match app.screen {
            Screen::Form => {
                form_body(&mut page, app, &fields, &doc.info);
                let problem = app.problems().into_iter().next();
                let note = match (&problem, app.keyframe) {
                    (Some(p), _) => {
                        Some(Line::from(s(format!("  {}", t(p.text).replace("{}", &p.arg)), fg(th.carmine))))
                    }
                    (None, Some((at, key))) if at - key > 0.05 => {
                        Some(if op::cut::hides_preroll(&doc.values, &doc.info) {
                            Line::from(vec![
                                s(format!("  {} ", t("keeps")), fg(th.dim)),
                                s(format!("{:.1} {}", at - key, t("s")), fg(th.ochre)),
                                s(format!(" {}", t("from the keyframe before it, hidden from players")), fg(th.dim)),
                            ])
                        } else {
                            Line::from(vec![
                                s(format!("  {} ", t("starts at")), fg(th.dim)),
                                s(fmt_ts(key), fg(th.ochre)),
                                s(t(", the keyframe before it; exact mode cuts clean"), fg(th.dim)),
                            ])
                        })
                    }
                    _ => None,
                };
                match note {
                    Some(line) => page.line(line),
                    None => page.gap(),
                }
                if let Some(cmd) = app.command() {
                    let focus = fields.get(app.focus).map(|s| s.id);
                    page.command(&cmd, Look::Edit(focus));
                }
                page.gap();
                let v = &doc.values;
                let total = doc.info.duration;
                match app.op {
                    OpKind::Cut => {
                        let key = app.keyframe.filter(|(at, _)| (*at - v.start).abs() < 0.001).map(|(_, k)| k);
                        page.piece(v.start, v.end, total, key, Some(v.cursor));
                    }
                    OpKind::Gif => page.piece(v.start, v.start + v.duration as f64, total, None, None),
                    OpKind::Frame => {
                        let frac = if total > 0.0 { v.cursor / total } else { 0.0 };
                        page.ruler(frac, Some(s("◆", bold().fg(th.ochre))), v.cursor, total, true);
                    }
                    _ => page.ruler(0.0, None, 0.0, total, false),
                }
                if app.dropdown.is_some() {
                    page.keys(&[("↑↓", "value"), ("enter", "take it"), ("esc", "keep the old one")]);
                } else if app.op == OpKind::Cut {
                    page.keys(&[
                        ("enter", "run"),
                        ("↑↓", "field"),
                        ("←→", "value"),
                        ("i o", "mark"),
                        ("tab", "operation"),
                        ("?", "keys"),
                        ("esc", "quit"),
                    ]);
                } else {
                    page.keys(&[
                        ("enter", "run"),
                        ("↑↓", "field"),
                        ("←→", "value"),
                        ("tab", "operation"),
                        ("c", "copy"),
                        ("?", "keys"),
                        ("esc", "quit"),
                    ]);
                }
            }
            Screen::Exists => {
                for line in field_rows(app, &fields, &doc.info, false, w) {
                    page.line(line);
                }
                page.gap();
                if let Some(cmd) = app.command() {
                    page.command(&cmd, Look::Exists);
                }
                page.gap();
                let name = op::output_name(app.op, &doc.values, &doc.info);
                page.error(vec![
                    Line::from(vec![s(name, bold().fg(th.carmine)), Span::raw(t(" already exists."))]),
                    Line::from(vec![
                        Span::raw(t("Press ")),
                        s("o", bold()),
                        Span::raw(t(" to overwrite it, or change the name.")),
                    ]),
                ]);
                page.keys(&[("o", "overwrite"), ("esc", "back to the form")]);
            }
            Screen::Running => {
                let Some(run) = &app.running else { return };
                for line in field_rows(app, &fields, &doc.info, true, w) {
                    page.line(line);
                }
                page.gap();
                page.command(&run.cmd, Look::Frozen);
                page.gap();
                let name = doc.info.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                page.line(Line::from(vec![Span::raw(format!("{} ", t(verb(app.op)))), s(name, bold())]));
                let p = run.progress;
                let frac = if run.total > 0.0 { p.out_time / run.total } else { 0.0 };
                page.ruler(frac, Some(s("◆", fg(th.ochre))), p.out_time.min(run.total), run.total, true);
                let mut stats =
                    vec![s(format!("{} ", t("speed")), fg(th.ice)), Span::raw(format!("{:.1}×    ", p.speed))];
                if doc.info.video.is_some() && app.op != OpKind::Audio {
                    stats.extend([s("fps ", fg(th.ice)), Span::raw(format!("{:.0}    ", p.fps))]);
                }
                stats.extend([s(format!("{} ", t("size")), fg(th.ice)), Span::raw(fmt_size(p.size))]);
                page.line(Line::from(stats));
                page.keys(&[("esc", "cancel"), ("l", "ffmpeg output")]);
            }
            Screen::Done => {
                let Some(done) = &app.done else { return };
                page.gap();
                page.command(&done.cmd, Look::Done);
                page.gap();
                if done.total > 0.0 {
                    page.ruler(1.0, None, done.total, done.total, false);
                    page.gap();
                }
                page.line(Line::from(vec![
                    s("✓", bold().fg(th.ice)),
                    Span::raw(format!(" {} ", t("Done:"))),
                    s(done.output.to_string_lossy().into_owned(), bold()),
                ]));
                page.gap();
                let mut keys = vec![("enter", "again"), ("f", "another file"), ("c", "copy"), ("esc", "quit")];
                match (&done.cut, done.op) {
                    (Some(cut), _) => {
                        let moved = cut_moved(cut);
                        let span = |a: f64, b: f64| format!("{} – {}", fmt_ts(a), fmt_ts(b));
                        page.line(Line::from(vec![
                            s(format!("  {}", pad(t("selected"), LABEL)), fg(th.dim)),
                            Span::raw(span(cut.start, cut.end)),
                            s(format!("   {:.0} {}", cut.end - cut.start, t("s")), fg(th.dim)),
                        ]));
                        // Container durations are a few milliseconds off; a
                        // tenth of a second is as fine as this reading gets.
                        let got_start = if moved { (cut.got_start * 10.0).round() / 10.0 } else { cut.start };
                        page.line(Line::from(vec![
                            s(format!("  {}", pad(t("got"), LABEL)), fg(th.ice)),
                            s(fmt_ts(got_start), if moved { bold().fg(th.carmine) } else { plain() }),
                            Span::raw(format!(" – {}", fmt_ts(cut.end))),
                            Span::raw(format!("   {:.0} {}, {}", cut.end - got_start, t("s"), fmt_size(done.after))),
                        ]));
                        if moved {
                            page.gap();
                            let by = format!("{:.0}", (cut.start - cut.got_start).max(1.0));
                            page.text(
                                t("The start moved back {} s: fast mode cuts on keyframes.").replace("{}", &by),
                                plain(),
                            );
                            page.text(t("For a clean edge pick exact mode: it re-encodes the piece."), plain());
                            keys.insert(0, ("t", "cut exactly"));
                        }
                    }
                    (None, OpKind::Compress) => {
                        let bar = 36usize;
                        let ratio = if done.before > 0 { done.after as f64 / done.before as f64 } else { 1.0 };
                        let (before, after) = if ratio <= 1.0 {
                            (bar, ((ratio * bar as f64).round() as usize).max(1))
                        } else {
                            (((bar as f64 / ratio).round() as usize).max(1), bar)
                        };
                        page.line(Line::from(vec![
                            s(pad(t("before"), 8), fg(th.dim)),
                            s(pad(&"█".repeat(before), bar + 1), fg(th.line)),
                            s(fmt_size(done.before), fg(th.dim)),
                        ]));
                        let change = if ratio <= 1.0 {
                            t("{}% smaller").replace("{}", &format!("{:.0}", (1.0 - ratio) * 100.0))
                        } else {
                            t("{}% bigger").replace("{}", &format!("{:.0}", (ratio - 1.0) * 100.0))
                        };
                        page.line(Line::from(vec![
                            s(pad(t("after"), 8), fg(th.ice)),
                            s(pad(&"█".repeat(after), bar + 1), fg(th.ice)),
                            s(fmt_size(done.after), bold()),
                            s(format!("  {change}"), fg(if ratio <= 1.0 { th.dim } else { th.carmine })),
                        ]));
                    }
                    (None, _) => {
                        page.line(Line::from(vec![
                            s(format!("  {}", pad(t("size"), LABEL)), fg(th.ice)),
                            s(fmt_size(done.after), bold()),
                        ]));
                    }
                }
                page.keys(&keys);
            }
            Screen::Failed => {
                let Some(fail) = &app.failed else { return };
                page.gap();
                page.command(&fail.cmd, Look::Frozen);
                page.gap();
                if fail.total > 0.0 {
                    let frac = fail.at / fail.total;
                    page.ruler(frac, Some(s("✕", bold().fg(th.carmine))), fail.at.min(fail.total), fail.total, true);
                    page.gap();
                }
                let code = fail.code.map_or(t("a signal").to_string(), |c| format!("{} {c}", t("code")));
                let mut lines = vec![Line::from(
                    t("ffmpeg stopped at {} with ").replace("{}", &fmt_clock(fail.at))
                        + &code
                        + t(". Its last lines:"),
                )];
                lines.extend(fail.lines.iter().map(|l| Line::from(s(clip(l, w - 4), fg(th.dim)))));
                if let Some(name) = &fail.removed {
                    lines.push(Line::from(vec![
                        Span::raw(t("The unfinished ")),
                        s(name.clone(), bold()),
                        Span::raw(t(" was removed.")),
                    ]));
                }
                page.error(lines);
                page.keys(&[("l", "full ffmpeg output"), ("c", "copy the command"), ("esc", "back to the form")]);
            }
            Screen::Pick => {}
        }
    }

    if let Some(toast) = &app.toast {
        let style = if toast.error { fg(th.carmine) } else { fg(th.ice) };
        let y = page.bottom - 1;
        let line = Paragraph::new(Line::from(s(pad(&clip(&toast.text, w), w), style)));
        page.f.render_widget(line, Rect::new(page.x, y, page.w, 1));
    }
}

fn pick_page(page: &mut Page, app: &App) {
    let th = page.th;
    let w = page.w as usize;
    let picker = &app.picker;
    let visible = picker.visible();
    let shown = visible.iter().filter(|i| !picker.entries[**i].is_dir).count();
    let place = picker.dir.to_string_lossy().into_owned();
    let count = format!("{shown} {} {} {} {place}", page.t("of"), picker.files(), page.t("in"));
    page.line(row(
        vec![
            s("▎ ", fg(th.ochre)),
            s(pad(page.t("file"), LABEL), bold()),
            s(picker.filter.clone(), bold().fg(th.ochre)),
            s("▏", fg(th.ochre)),
        ],
        vec![s(count, fg(th.dim))],
        w,
    ));
    page.gap();

    let rows = 8usize;
    let first = picker.selected.saturating_sub(rows - 1);
    for (n, i) in visible.iter().enumerate().skip(first).take(rows) {
        let e = &picker.entries[*i];
        let here = n == picker.selected;
        let name = format!("{} {}", if here { "▸" } else { " " }, pad(&clip(&e.name, 30), 31));
        let mut spans = vec![Span::raw(" ".repeat(LABEL)), s(name, if here { bold().fg(th.ochre) } else { plain() })];
        if let Some(info) = &e.info {
            let size = info
                .video
                .as_ref()
                .map(|v| format!("{}×{}", v.width, v.height))
                .unwrap_or_else(|| page.t("audio").to_string());
            spans.push(s(pad(&fmt_clock(info.duration), 10), fg(th.dim)));
            spans.push(s(pad(&size, 12), fg(th.dim)));
            spans.push(s(fmt_size(info.size), fg(th.dim)));
        }
        page.line(Line::from(spans));
    }
    if visible.is_empty() {
        page.text(format!("{}{}", " ".repeat(LABEL + 2), page.t("Nothing here matches.")), fg(th.dim));
    }
    page.y = page.y.max(page.bottom.saturating_sub(9));

    let mut cmd = vec![s("ffmpeg", bold()), Span::raw(" "), s("-i", bold()), Span::raw(" ")];
    match picker.current().filter(|e| !e.is_dir) {
        Some(e) => {
            cmd.push(s(e.path.to_string_lossy().into_owned(), bold().fg(th.ochre).add_modifier(Modifier::UNDERLINED)));
            cmd.push(s(" …", fg(th.dim)));
        }
        None => cmd.push(s("…", fg(th.dim))),
    }
    let (ice, line) = (th.ice, th.line);
    page.framed("command", line, fg(ice), vec![Line::from(cmd)]);
    page.gap();
    page.text(page.t("Pick a file and the command starts to build."), fg(th.dim));
    page.keys(&[("enter", "pick"), ("↑↓", "file"), ("abc", "filter"), ("esc", if app.doc.is_some() { "back" } else { "quit" })]);
}

fn keys_page(page: &mut Page, app: &App) {
    let th = page.th;
    page.title("keys");
    let groups: [(&str, &[(&str, &str)]); 4] = [
        (
            "everywhere",
            &[
                ("tab ⇧tab", "next and previous operation"),
                ("↑↓", "field"),
                ("enter", "run the command"),
                ("c", "copy the command"),
                ("f", "another file"),
                ("esc", "back; from the form, quit"),
            ],
        ),
        (
            "in a field",
            &[
                ("←→", "one step"),
                ("⇧←→  H L", "a big step"),
                ("0-9", "type a value, enter takes it"),
                ("space", "next value, or open the list"),
                ("backspace", "back to the default"),
            ],
        ),
        (
            "in a time field",
            &[
                ("←→", "±1 s"),
                ("⇧←→  H L", "±10 s"),
                (", .", "one frame back and forward"),
                ("i o", "in cut: the cursor becomes start, end"),
            ],
        ),
        ("while ffmpeg runs", &[("esc", "cancel, remove the unfinished file"), ("l", "show the ffmpeg output")]),
    ];
    // One group per block; in a window too narrow for two columns the
    // descriptions are clipped rather than wrapped.
    let half = page.w as usize / 2;
    let lang = page.lang;
    let block = |(title, keys): (&str, &[(&str, &str)])| {
        let mut lines = vec![vec![s(pad(tr(lang, title), half), fg(th.ice))]];
        for (key, what) in keys {
            lines.push(vec![s(pad(key, 11), bold()), s(pad(&clip(tr(lang, what), half - 12), half - 11), fg(th.dim))]);
        }
        lines.push(vec![Span::raw(" ".repeat(half))]);
        lines
    };
    let left: Vec<_> = [groups[0], groups[2]].into_iter().flat_map(block).collect();
    let right: Vec<_> = [groups[1], groups[3]].into_iter().flat_map(block).collect();
    for i in 0..left.len().max(right.len()) {
        let mut spans = left.get(i).cloned().unwrap_or_else(|| vec![Span::raw(" ".repeat(half))]);
        spans.extend(right.get(i).cloned().unwrap_or_default());
        page.line(Line::from(spans));
    }
    page.text(page.t("Letter keys also work in a Cyrillic layout."), fg(th.dim));
    page.text(page.t("kadr adds -hide_banner -nostats -progress pipe:1 to what it runs,"), fg(th.dim));
    page.text(page.t("and -n unless you chose to overwrite."), fg(th.dim));
    let (here, there) = if app.lang == Lang::En { ("English", "Русский") } else { ("Русский", "English") };
    page.line(Line::from(vec![
        s("tab", bold()),
        s(format!(" {} ", page.t("language:")), fg(th.dim)),
        s(here, fg(th.ochre)),
        s(format!(" → {there}"), fg(th.dim)),
    ]));
    page.keys(&[("tab", "language"), ("?  esc", "close")]);
}

fn log_page(page: &mut Page, app: &App) {
    let th = page.th;
    page.title("ffmpeg output");
    let room = (page.bottom - page.y) as usize;
    if app.log.is_empty() {
        page.text(page.t("ffmpeg has not said anything yet."), fg(th.dim));
    }
    for l in app.log.iter().skip(app.log.len().saturating_sub(room)) {
        page.text(clip(l, page.w as usize), plain());
    }
    page.keys(&[("l  esc", "close")]);
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::app::tests::{app, press};
    use crate::text;

    fn screen(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn compress_form() {
        let got = screen(&app(), 100, 24);
        assert!(got.contains("⌜ kadr ⌟"), "{got}");
        assert!(got.contains("▎ crf         23▏"), "{got}");
        assert!(got.contains("18 (better) to 28 (smaller file)"), "{got}");
        assert!(got.contains("file        lecture_04.mov"), "{got}");
        assert!(got.contains("02:10, 1920×1080, 48 MB"), "{got}");
        assert!(got.contains("ffmpeg -i lecture_04.mov -c:v libx264 -crf 23 -preset medium"), "{got}");
        assert!(got.contains("00:00 / 02:10"), "{got}");
        assert!(got.contains("enter run"), "{got}");
    }

    #[test]
    fn every_tab_fits_at_the_smallest_width() {
        let got = screen(&app(), MIN_WIDTH, MIN_HEIGHT);
        let tabs = got.lines().nth(1).unwrap();
        for op in OpKind::ALL {
            assert!(tabs.contains(op.name()), "{tabs}");
        }
    }

    #[test]
    fn nothing_spills_at_the_smallest_size() {
        let mut app = app();
        app.focus = 2;
        app.dropdown = Some(5);
        let got = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        assert!(got.contains("▸ medium"), "{got}");
        assert!(got.contains("lecture_04_small.mp4"), "{got}");
        assert!(got.split('\n').next_back().unwrap().is_empty(), "the bottom margin stays clear:\n{got}");
        assert!(got.contains("esc keep the old one"), "{got}");
    }

    #[test]
    fn cut_form_marks_the_piece_and_the_cursor() {
        let mut app = app();
        app.op = OpKind::Cut;
        let v = &mut app.doc.as_mut().unwrap().values;
        (v.cursor, v.start, v.end) = (50.0, 38.0, 72.0);
        let got = screen(&app, 100, 24);
        assert!(got.contains("ffmpeg -ss 00:00:38 -to 00:01:12 -i lecture_04.mov -c copy"), "{got}");
        assert!(got.contains("length      00:00:34"), "{got}");
        let ruler = got.lines().find(|l| l.contains('[')).unwrap();
        let (a, c, b) = (ruler.find('[').unwrap(), ruler.find('◆').unwrap(), ruler.find(']').unwrap());
        assert!(a < c && c < b, "{ruler}");
        assert!(got.contains("00:38") && got.contains("01:12"), "{got}");
    }

    #[test]
    fn the_gif_filter_wraps_inside_the_frame() {
        let mut app = app();
        app.op = OpKind::Gif;
        let got = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        assert!(got.contains("palettegen[p];[b][p]paletteuse'"), "{got}");
        assert!(got.contains("-loop 0 lecture_04.gif"), "{got}");
        for line in got.lines().filter(|l| l.contains('│')) {
            assert!(line.trim_end().ends_with('│'), "a line ran over the frame:\n{got}");
        }
    }

    #[test]
    fn too_small_says_so() {
        let got = screen(&app(), 52, 11);
        assert!(got.contains("it is 52×11 now."), "{got}");
    }

    #[test]
    fn keys_page_fits() {
        let mut app = app();
        app.overlay = Some(Overlay::Keys);
        let got = screen(&app, MIN_WIDTH, MIN_HEIGHT);
        assert!(got.contains("while ffmpeg runs"), "{got}");
        assert!(got.contains("show the ffmpeg output"), "{got}");
        assert!(got.contains("-hide_banner"), "{got}");
        assert!(got.contains("English → Русский"), "{got}");
        assert!(got.contains("close"), "{got}");
    }

    #[test]
    fn russian_form() {
        let mut app = app();
        app.lang = Lang::Ru;
        let got = screen(&app, 100, 24);
        assert!(got.contains("сжать  вырезать  gif  звук"), "{got}");
        assert!(got.contains("▎ crf         23▏"), "{got}");
        assert!(got.contains("от 18 (лучше) до 28 (меньше файл)"), "{got}");
        assert!(got.contains("команда"), "{got}");
        assert!(got.contains("enter пуск"), "{got}");
        // The command itself is never translated.
        assert!(got.contains("ffmpeg -i lecture_04.mov -c:v libx264"), "{got}");
    }

    #[test]
    fn the_russian_key_bar_keeps_esc_at_the_smallest_width() {
        let mut app = app();
        app.lang = Lang::Ru;
        for _ in OpKind::ALL {
            let got = screen(&app, MIN_WIDTH, MIN_HEIGHT);
            assert!(got.contains("esc выход"), "{:?}:\n{got}", app.op);
            assert!(got.contains("enter пуск"), "{:?}:\n{got}", app.op);
            press(&mut app, KeyCode::Tab);
        }
    }

    /// Walks every operation and screen in Russian and checks that no
    /// English sentence is left on it.
    #[test]
    fn nothing_is_left_untranslated() {
        let mut app = app();
        app.lang = Lang::Ru;
        let mut seen = String::new();
        for _ in OpKind::ALL {
            let fields = app.fields().len();
            for focus in 1..fields {
                app.focus = focus;
                seen.push_str(&screen(&app, 140, 40));
            }
            app.focus = 1;
            press(&mut app, KeyCode::Tab);
        }
        app.overlay = Some(Overlay::Keys);
        seen.push_str(&screen(&app, 140, 40));
        app.overlay = None;
        app.open_picker();
        seen.push_str(&screen(&app, 140, 40));

        let untranslated: Vec<&str> = text::english_keys().filter(|k| k.len() > 9 && seen.contains(*k)).collect();
        assert!(untranslated.is_empty(), "left in English: {untranslated:?}");
    }
}
