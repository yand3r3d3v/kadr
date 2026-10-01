use super::*;

/// The extension for an untouched track, by its codec.
fn copy_extension(codec: &str) -> &'static str {
    match codec {
        "aac" | "alac" => "m4a",
        "mp3" => "mp3",
        "opus" => "opus",
        "vorbis" => "ogg",
        "flac" => "flac",
        c if c.starts_with("pcm_") => "wav",
        _ => "mka",
    }
}

pub fn extension(v: &Values, info: &MediaInfo) -> &'static str {
    if v.format == FORMAT_COPY {
        copy_extension(info.audio.as_ref().map_or("", |a| a.codec.as_str()))
    } else {
        "mp3"
    }
}

pub fn fields(v: &Values, _info: &MediaInfo) -> Vec<FieldSpec> {
    let copy = v.format == FORMAT_COPY;
    vec![
        inline(
            FieldId::Format,
            "format",
            &[("mp3", "plays everywhere"), ("as is", "the track is copied without loss")],
            v.format,
        ),
        FieldSpec {
            id: FieldId::Quality,
            label: "quality",
            kind: Kind::Int { min: 0, max: 9, soft: (0, 9), step: 1, big: 3 },
            value: v.quality.to_string(),
            hint: if copy {
                "not needed: the audio is not re-encoded"
            } else {
                "0 (better) to 9 (smaller file)"
            },
            enabled: !copy,
        },
    ]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let format = Some(FieldId::Format);
    let mut b = Builder::new(v.overwrite);
    b.input(info);
    b.flag("-vn", None);
    if v.format == FORMAT_COPY {
        b.flag("-c:a", format).value("copy", FieldId::Format);
    } else {
        b.flag("-c:a", format).value("libmp3lame", FieldId::Format);
        b.flag("-q:a", Some(FieldId::Quality)).value(v.quality.to_string(), FieldId::Quality);
    }
    b.output(OpKind::Audio, v, info)
}

/// One choice changes two places in the command: the codec and the extension.
pub fn output(v: &Values, info: &MediaInfo) -> Vec<Part> {
    vec![
        sibling(info, stem(info)),
        part(format!(".{}", extension(v, info)), Role::Value, Some(FieldId::Format)),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn mp3_by_default() {
        let info = lecture();
        let v = Values::defaults(&info);
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -i lecture_04.mov -vn -c:a libmp3lame -q:a 2 lecture_04.mp3"
        );
    }

    #[test]
    fn as_is_changes_the_codec_and_the_extension() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.format = FORMAT_COPY;
        let cmd = build(&v, &info);
        assert_eq!(shell(&cmd), "ffmpeg -i lecture_04.mov -vn -c:a copy lecture_04.m4a");
        let out = cmd.last().unwrap();
        assert_eq!(out.parts[1].text, ".m4a");
        assert_eq!(out.parts[1].field, Some(FieldId::Format));
    }

    #[test]
    fn a_typed_name_wins() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.output = Some("voice.mp3".into());
        assert!(shell(&build(&v, &info)).ends_with(" voice.mp3"));
    }
}
