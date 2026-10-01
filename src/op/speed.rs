use super::*;

/// The factor, how it reads in the form, how it reads in a file name.
pub const SPEEDS: [(f64, &str, &str); 5] = [
    (0.5, "0.5×", "0.5x"),
    (0.75, "0.75×", "0.75x"),
    (1.25, "1.25×", "1.25x"),
    (1.5, "1.5×", "1.5x"),
    (2.0, "2×", "2x"),
];
pub const DEFAULT: usize = 4;

fn factor(v: &Values) -> String {
    let f = SPEEDS[v.speed].0;
    if f.fract() == 0.0 { format!("{f:.0}") } else { f.to_string() }
}

pub fn fields(v: &Values, _info: &MediaInfo) -> Vec<FieldSpec> {
    let items: Vec<(&str, &'static str)> = SPEEDS
        .iter()
        .map(|(f, label, _)| (*label, if *f < 1.0 { "slower and longer" } else { "faster and shorter" }))
        .collect();
    vec![inline(FieldId::Speed, "speed", &items, v.speed)]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let speed = Some(FieldId::Speed);
    let mut b = Builder::new(v.overwrite);
    b.input(info);
    b.flag("-vf", speed)
        .arg(vec![part("setpts=PTS/", Role::Fixed, None), part(factor(v), Role::Value, speed)]);
    if info.audio.is_some() {
        b.flag("-af", speed)
            .arg(vec![part("atempo=", Role::Fixed, None), part(factor(v), Role::Value, speed)]);
    }
    b.output(OpKind::Speed, v, info)
}

pub fn output(v: &Values, info: &MediaInfo) -> Vec<Part> {
    vec![
        sibling(info, format!("{}_", stem(info))),
        part(SPEEDS[v.speed].2, Role::Value, Some(FieldId::Speed)),
        part(".mp4", Role::Output, Some(FieldId::Output)),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn twice_as_fast() {
        let info = lecture();
        let v = Values::defaults(&info);
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -i lecture_04.mov -vf setpts=PTS/2 -af atempo=2 lecture_04_2x.mp4"
        );
        assert_eq!(result_duration(OpKind::Speed, &v, &info), 65.0);
    }

    #[test]
    fn slower_and_silent() {
        let mut info = lecture();
        info.audio = None;
        let mut v = Values::defaults(&info);
        v.speed = 1;
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -i lecture_04.mov -vf setpts=PTS/0.75 lecture_04_0.75x.mp4"
        );
    }
}
