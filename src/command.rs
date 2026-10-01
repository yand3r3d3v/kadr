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
const DEEP: usize = INDENT + 5; // the rest of an argument too long for one line

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

/// An argument as styled pieces that a line may break between: after each
/// `,` and `;`, which is where a long filter reads naturally.
fn pieces(arg: &Arg, look: Look, th: &Theme) -> Vec<Span<'static>> {
    let mut out = vec![];
    let quote = needs_quote(&arg.text());
    if quote {
        out.push(Span::raw("'"));
    }
    for p in &arg.parts {
        let st = style(p, arg.is_output, look, th);
        let mut piece = String::new();
        for c in p.text.chars() {
            piece.push(c);
            if c == ',' || c == ';' {
                out.push(Span::styled(std::mem::take(&mut piece), st));
            }
        }
        if !piece.is_empty() {
            out.push(Span::styled(piece, st));
        }
    }
    if quote {
        out.push(Span::raw("'"));
    }
    out
}

fn cells(span: &Span) -> usize {
    span.content.chars().count()
}

/// Breaks the command into lines of at most `width` cells where it can.
pub fn lines(cmd: &CommandLine, width: usize, look: Look, th: &Theme) -> Vec<Line<'static>> {
    let mut out: Vec<Vec<Span<'static>>> = vec![vec![]];
    let mut used = 0;
    let mut deep = false;
    let newline = |out: &mut Vec<Vec<Span<'static>>>, used: &mut usize, indent: usize| {
        out.push(vec![Span::raw(" ".repeat(indent))]);
        *used = indent;
    };
    for unit in units(cmd) {
        let w: usize = unit.iter().map(arg_width).sum::<usize>() + unit.len() - 1;
        let fresh = out.last().is_some_and(|l| l.len() <= 1) && (used == 0 || used == INDENT);
        let fits_here = used == 0 || used + 1 + w <= width;
        let fits_alone = INDENT + w <= width;
        // What follows a broken argument starts its own line, back at the
        // usual indent.
        if (!fits_here && !fresh) || std::mem::take(&mut deep) {
            newline(&mut out, &mut used, INDENT);
        }
        let start_of_line = used == 0 || (used == INDENT && out.last().is_some_and(|l| l.len() == 1));
        if !start_of_line {
            out.last_mut().unwrap().push(Span::raw(" "));
            used += 1;
        }
        for (i, arg) in unit.iter().enumerate() {
            if i > 0 {
                out.last_mut().unwrap().push(Span::raw(" "));
                used += 1;
            }
            for piece in pieces(arg, look, th) {
                // Only an argument too long for any line is broken inside.
                if !fits_alone && used + cells(&piece) > width && used > DEEP {
                    newline(&mut out, &mut used, DEEP);
                    deep = true;
                }
                used += cells(&piece);
                out.last_mut().unwrap().push(piece);
            }
        }
    }
    out.into_iter().map(Line::from).collect()
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
    fn an_argument_too_long_for_a_line_breaks_after_a_separator() {
        let info = lecture();
        let v = Values::defaults(&info);
        let cmd = build(OpKind::Gif, &v, &info);
        let got = text(&lines(&cmd, 60, Look::Frozen, &Theme::ansi()));
        assert_eq!(
            got,
            vec![
                "ffmpeg -ss 00:00:00 -t 5 -i lecture_04.mov",
                "       -vf 'fps=12,scale=480:-1:flags=lanczos,split[a][b];",
                "            [a]palettegen[p];[b][p]paletteuse'",
                "       -loop 0 lecture_04.gif",
            ]
        );
        assert!(got.iter().all(|l| l.chars().count() <= 60), "{got:?}");
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
