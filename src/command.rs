//! Laying the command out in its frame: wrapping with a hanging indent, and
//! lighting up the pieces that belong to the field in focus.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::op::{Arg, CommandLine, FieldId, Part, Role};
use crate::theme::Theme;
use crate::util::needs_quote;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Look {
    /// The form: values glow, the focused field's pieces are underlined.
    Edit(Option<FieldId>),
    /// Running or failed: nothing can change, nothing glows.
    Frozen,
    /// Finished: the output is the news.
    Done,
    /// The output already exists.
    Exists,
}

const INDENT: usize = 7; // under the first argument, after "ffmpeg "

fn arg_width(arg: &Arg) -> usize {
    let text = arg.text();
    text.chars().count() + if needs_quote(&text) { 2 } else { 0 }
}

/// Groups of arguments that stay on one line: a flag and its value.
fn units(cmd: &CommandLine) -> Vec<&[Arg]> {
    let mut units = vec![];
    let mut i = 0;
    while i < cmd.len() {
        let pair = cmd[i].is_flag() && cmd.get(i + 1).is_some_and(|next| !next.is_flag() && !next.is_output);
        let len = if pair { 2 } else { 1 };
        units.push(&cmd[i..i + len]);
        i += len;
    }
    units
}

/// Breaks the command into lines of at most `width` cells where it can.
pub fn wrap(cmd: &CommandLine, width: usize) -> Vec<Vec<&Arg>> {
    let mut lines: Vec<Vec<&Arg>> = vec![vec![]];
    let mut used = 0;
    for unit in units(cmd) {
        let w: usize = unit.iter().map(arg_width).sum::<usize>() + unit.len() - 1;
        let line = lines.last_mut().unwrap();
        if !line.is_empty() && used + 1 + w > width {
            lines.push(unit.iter().collect());
            used = INDENT + w;
        } else {
            used += if line.is_empty() { w } else { 1 + w };
            line.extend(unit.iter());
        }
    }
    lines
}

fn style(p: &Part, is_output: bool, look: Look, th: &Theme) -> Style {
    let plain = Style::default();
    let bold = plain.add_modifier(Modifier::BOLD);
    let lit = |s: Style| s.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let base = match p.role {
        Role::Program => bold,
        Role::Flag => plain.fg(th.dim),
        _ => plain,
    };
    match look {
        Look::Frozen => base,
        Look::Done if is_output => bold.fg(th.ice),
        Look::Done => base,
        Look::Exists if is_output => lit(plain.fg(th.carmine)),
        Look::Exists | Look::Edit(_) => {
            let focus = match look {
                Look::Edit(f) => f,
                _ => None,
            };
            let focused = focus.is_some() && p.field == focus;
            match p.role {
                Role::Flag if focused => bold,
                Role::Value if focused => lit(plain.fg(th.ochre)),
                Role::Value => plain.fg(th.ochre),
                Role::Output if focused => lit(plain.fg(th.ochre)),
                _ => base,
            }
        }
    }
}

pub fn lines(cmd: &CommandLine, width: usize, look: Look, th: &Theme) -> Vec<Line<'static>> {
    wrap(cmd, width)
        .into_iter()
        .enumerate()
        .map(|(n, args)| {
            let mut spans = vec![];
            if n > 0 {
                spans.push(Span::raw(" ".repeat(INDENT)));
            }
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                let quote = needs_quote(&arg.text());
                if quote {
                    spans.push(Span::raw("'"));
                }
                for p in &arg.parts {
                    spans.push(Span::styled(p.text.clone(), style(p, arg.is_output, look, th)));
                }
                if quote {
                    spans.push(Span::raw("'"));
                }
            }
            Line::from(spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::testing::lecture;
    use crate::op::{OpKind, Values, build};

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn wraps_with_a_hanging_indent_and_keeps_flags_with_values() {
        let info = lecture();
        let v = Values::defaults(&info);
        let cmd = build(OpKind::Compress, &v, &info);
        let got = text(&lines(&cmd, 52, Look::Frozen, &Theme::ansi()));
        assert_eq!(
            got,
            vec![
                "ffmpeg -i lecture_04.mov -c:v libx264 -crf 23",
                "       -preset medium -vf scale=-2:720 -c:a aac",
                "       -b:a 128k lecture_04_small.mp4",
            ]
        );
    }

    #[test]
    fn focus_underlines_only_its_value() {
        let info = lecture();
        let v = Values::defaults(&info);
        let cmd = build(OpKind::Compress, &v, &info);
        let th = Theme::ansi();
        let all = lines(&cmd, 200, Look::Edit(Some(FieldId::Resolution)), &th);
        let underlined: Vec<&str> = all[0]
            .spans
            .iter()
            .filter(|s| s.style.add_modifier.contains(Modifier::UNDERLINED))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(underlined, vec!["720"]);
    }

    #[test]
    fn names_with_spaces_are_shown_quoted() {
        let mut info = lecture();
        info.path = "my lecture.mov".into();
        let v = Values::defaults(&info);
        let cmd = build(OpKind::Audio, &v, &info);
        let got = text(&lines(&cmd, 200, Look::Frozen, &Theme::ansi()));
        assert_eq!(got[0], "ffmpeg -i 'my lecture.mov' -vn -c:a libmp3lame -q:a 2 'my lecture.mp3'");
    }
}
