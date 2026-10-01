use super::*;
use crate::util::{fmt_clock, fmt_ts};

const TIME_HINT: &str = "←→ 1 s, ⇧←→ 10 s, , . one frame";

pub fn fields(v: &Values, _info: &MediaInfo) -> Vec<FieldSpec> {
    vec![
        FieldSpec {
            id: FieldId::Start,
            label: "start",
            kind: Kind::Time,
            value: fmt_ts(v.start),
            hint: TIME_HINT,
            enabled: true,
        },
        FieldSpec {
            id: FieldId::End,
            label: "end",
            kind: Kind::Time,
            value: fmt_ts(v.end),
            hint: TIME_HINT,
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Length,
            label: "length",
            kind: Kind::Info,
            value: fmt_ts((v.end - v.start).max(0.0)),
            hint: "",
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Mode,
            label: "mode",
            kind: Kind::Choice {
                items: vec![
                    Choice {
                        label: "fast".into(),
                        hint: "copies the stream; the start snaps back to a keyframe",
                    },
                    Choice { label: "exact".into(), hint: "re-encodes, cuts clean" },
                ],
                selected: v.mode,
                inline: true,
            },
            value: if v.mode == MODE_EXACT { "exact" } else { "fast" }.into(),
            hint: "",
            enabled: true,
        },
    ]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let mode = Some(FieldId::Mode);
    let mut b = Builder::new(v.overwrite);
    b.flag("-ss", Some(FieldId::Start)).value(fmt_ts(v.start), FieldId::Start);
    b.flag("-to", Some(FieldId::End)).value(fmt_ts(v.end), FieldId::End);
    b.input(info);
    if v.mode == MODE_EXACT {
        b.flag("-c:v", mode).value("libx264", FieldId::Mode);
        b.flag("-crf", mode).value("18", FieldId::Mode);
        if info.audio.is_some() {
            b.flag("-c:a", mode).value("aac", FieldId::Mode);
        }
    } else {
        b.flag("-c", mode).value("copy", FieldId::Mode);
    }
    b.output(output_parts(OpKind::Cut, v, info, None))
}

/// A stream copy always begins at a keyframe. In mp4 and mov ffmpeg writes an
/// edit list that hides the frames before the start asked for, so players
/// show a clean cut; other containers simply start earlier.
pub fn hides_preroll(v: &Values, info: &MediaInfo) -> bool {
    output_path(OpKind::Cut, v, info)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["mp4", "mov", "m4v", "m4a", "3gp"].contains(&e.to_ascii_lowercase().as_str()))
}

pub fn check(v: &Values, info: &MediaInfo) -> Vec<Problem> {
    let mut problems = vec![];
    if v.end <= v.start {
        problems.push(Problem {
            field: Some(FieldId::End),
            text: "The end must come after the start.".into(),
        });
    }
    if v.end > info.duration + 0.05 {
        problems.push(Problem {
            field: Some(FieldId::End),
            text: format!("The end is past the end of the file ({}).", fmt_clock(info.duration)),
        });
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    fn piece() -> (MediaInfo, Values) {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.start = 38.0;
        v.end = 72.0;
        (info, v)
    }

    #[test]
    fn fast_copies_the_stream() {
        let (info, v) = piece();
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -ss 00:00:38 -to 00:01:12 -i lecture_04.mov -c copy lecture_04_cut.mov"
        );
    }

    #[test]
    fn exact_re_encodes_into_mp4() {
        let (info, mut v) = piece();
        v.mode = MODE_EXACT;
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -ss 00:00:38 -to 00:01:12 -i lecture_04.mov -c:v libx264 -crf 18 -c:a aac \
             lecture_04_cut.mp4"
        );
    }

    #[test]
    fn end_before_start_is_a_problem() {
        let (info, mut v) = piece();
        v.end = 10.0;
        assert_eq!(check(&v, &info)[0].field, Some(FieldId::End));
    }

    #[test]
    fn progress_counts_the_piece() {
        let (info, v) = piece();
        assert_eq!(result_duration(OpKind::Cut, &v, &info), 34.0);
    }
}
