use super::*;
use crate::util::fmt_ts;

/// The palette pass that makes a gif look right: one palette built from the
/// piece itself instead of the generic 256 colours.
const PALETTE: &str = ":-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse";

fn source_width(info: &MediaInfo) -> i64 {
    info.video.as_ref().map_or(480, |v| v.width as i64).max(80)
}

pub fn default_width(info: &MediaInfo) -> i64 {
    source_width(info).min(480)
}

pub fn default_duration(info: &MediaInfo) -> i64 {
    (info.duration.floor() as i64).clamp(1, 5)
}

pub fn max_duration(info: &MediaInfo) -> i64 {
    (info.duration.floor() as i64).clamp(1, 60)
}

pub fn fields(v: &Values, info: &MediaInfo) -> Vec<FieldSpec> {
    let width = source_width(info);
    vec![
        cut::time_field(FieldId::Start, "start", v.start, cut::TIME_HINT),
        FieldSpec {
            id: FieldId::Duration,
            label: "length",
            kind: Kind::Int { min: 1, max: max_duration(info), soft: (1, 15), step: 1, big: 5 },
            value: v.duration.to_string(),
            hint: "seconds; a long gif gets heavy fast",
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Fps,
            label: "fps",
            kind: Kind::Int { min: 1, max: 50, soft: (5, 20), step: 1, big: 5 },
            value: v.fps.to_string(),
            hint: "frames per second; 10 to 15 is the usual",
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Width,
            label: "width",
            kind: Kind::Int { min: 80, max: width, soft: (80, width.min(800)), step: 20, big: 100 },
            value: v.width.to_string(),
            hint: "pixels; the height follows",
            enabled: true,
        },
    ]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let mut b = Builder::new(v.overwrite);
    b.flag("-ss", Some(FieldId::Start)).value(fmt_ts(v.start), FieldId::Start);
    b.flag("-t", Some(FieldId::Duration)).value(v.duration.to_string(), FieldId::Duration);
    b.input(info);
    b.flag("-vf", None).arg(vec![
        part("fps=", Role::Fixed, None),
        part(v.fps.to_string(), Role::Value, Some(FieldId::Fps)),
        part(",scale=", Role::Fixed, None),
        part(v.width.to_string(), Role::Value, Some(FieldId::Width)),
        part(PALETTE, Role::Fixed, None),
    ]);
    b.flag("-loop", None).fixed("0");
    b.output(OpKind::Gif, v, info)
}

pub fn check(v: &Values, info: &MediaInfo) -> Vec<Problem> {
    if v.start + v.duration as f64 > info.duration + 0.05 {
        vec![Problem::new(FieldId::Duration, "The piece runs past the end of the file.")]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn default_command() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.start = 38.0;
        v.duration = 6;
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -ss 00:00:38 -t 6 -i lecture_04.mov -vf \
             'fps=12,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse' \
             -loop 0 lecture_04.gif"
        );
    }

    #[test]
    fn only_fps_and_width_belong_to_fields_in_the_filter() {
        let info = lecture();
        let v = Values::defaults(&info);
        let cmd = build(&v, &info);
        let filter = cmd.iter().find(|a| a.text().starts_with("fps=")).unwrap();
        let owned: Vec<_> = filter.parts.iter().filter_map(|p| p.field).collect();
        assert_eq!(owned, vec![FieldId::Fps, FieldId::Width]);
    }

    #[test]
    fn the_width_never_exceeds_the_source() {
        let mut info = lecture();
        info.video.as_mut().unwrap().width = 320;
        assert_eq!(default_width(&info), 320);
    }

    #[test]
    fn running_past_the_end_is_a_problem() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.start = 128.0;
        assert_eq!(check(&v, &info)[0].field, Some(FieldId::Duration));
    }
}
