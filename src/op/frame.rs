use super::*;
use crate::util::fmt_ts;

pub fn fields(v: &Values, _info: &MediaInfo) -> Vec<FieldSpec> {
    vec![
        cut::time_field(FieldId::Cursor, "time", v.cursor, cut::TIME_HINT),
        inline(
            FieldId::Image,
            "format",
            &[("png", "lossless, bigger"), ("jpg", "smaller, slightly lossy")],
            v.image,
        ),
    ]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let mut b = Builder::new(v.overwrite);
    b.flag("-ss", Some(FieldId::Cursor)).value(fmt_ts(v.cursor), FieldId::Cursor);
    b.input(info);
    b.flag("-frames:v", None).fixed("1");
    if v.image == IMAGE_JPG {
        b.flag("-q:v", Some(FieldId::Image)).value("2", FieldId::Image);
    }
    b.output(OpKind::Frame, v, info)
}

/// The name carries the time, so frames taken one after another do not
/// overwrite each other.
pub fn output(v: &Values, info: &MediaInfo) -> Vec<Part> {
    let ext = if v.image == IMAGE_JPG { ".jpg" } else { ".png" };
    vec![
        sibling(info, format!("{}_", stem(info))),
        part(fmt_ts(v.cursor).replace(':', "-"), Role::Value, Some(FieldId::Cursor)),
        part(ext, Role::Value, Some(FieldId::Image)),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn one_png_named_after_its_time() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.cursor = 38.0;
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -ss 00:00:38 -i lecture_04.mov -frames:v 1 lecture_04_00-00-38.png"
        );
    }

    #[test]
    fn jpg_sets_the_quality() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.image = IMAGE_JPG;
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -ss 00:00:00 -i lecture_04.mov -frames:v 1 -q:v 2 lecture_04_00-00-00.jpg"
        );
    }
}
