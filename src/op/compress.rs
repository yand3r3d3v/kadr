use super::*;

pub const PRESETS: [(&str, &str); 9] = [
    ("ultrafast", "fastest encode, biggest file"),
    ("superfast", ""),
    ("veryfast", "much faster, bigger file"),
    ("faster", ""),
    ("fast", ""),
    ("medium", "the ffmpeg default"),
    ("slow", "slower, smaller file at the same crf"),
    ("slower", ""),
    ("veryslow", "slowest encode, smallest file"),
];
pub const DEFAULT_PRESET: usize = 5;

/// `None` keeps the source size. kadr never offers to scale up.
pub fn resolutions(info: &MediaInfo) -> Vec<Option<u32>> {
    let source = info.video.as_ref().map_or(0, |v| v.height);
    let mut list = vec![None];
    list.extend([1080, 720, 480].into_iter().filter(|h| *h < source).map(Some));
    list
}

pub fn default_resolution(info: &MediaInfo) -> usize {
    resolutions(info).iter().position(|r| *r == Some(720)).unwrap_or(0)
}

pub fn resolution_label(r: Option<u32>) -> String {
    r.map_or("source".to_string(), |h| format!("{h}p"))
}

pub fn fields(v: &Values, info: &MediaInfo) -> Vec<FieldSpec> {
    let res = resolutions(info);
    let mut fields = vec![
        FieldSpec {
            id: FieldId::Crf,
            label: "crf",
            kind: Kind::Int { min: 0, max: 51, soft: (18, 28), step: 1, big: 5 },
            value: v.crf.to_string(),
            hint: "18 (better) to 28 (smaller file)",
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Preset,
            label: "preset",
            kind: Kind::Choice {
                items: PRESETS
                    .iter()
                    .map(|(label, hint)| Choice { label: label.to_string(), hint })
                    .collect(),
                selected: v.preset,
                inline: false,
            },
            value: PRESETS[v.preset].0.to_string(),
            hint: "space opens the list",
            enabled: true,
        },
        FieldSpec {
            id: FieldId::Resolution,
            label: "resolution",
            kind: Kind::Choice {
                items: res
                    .iter()
                    .map(|r| Choice { label: resolution_label(*r), hint: "" })
                    .collect(),
                selected: v.resolution,
                inline: true,
            },
            value: resolution_label(res[v.resolution.min(res.len() - 1)]),
            hint: "the frame height; the width follows",
            enabled: true,
        },
    ];
    if info.audio.is_some() {
        fields.push(inline(
            FieldId::Sound,
            "sound",
            &[("keep", "re-encoded to aac, 128k"), ("remove", "the result is silent")],
            v.sound,
        ));
    }
    fields
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let mut b = Builder::new(v.overwrite);
    b.input(info);
    b.flag("-c:v", None).fixed("libx264");
    b.flag("-crf", Some(FieldId::Crf)).value(v.crf.to_string(), FieldId::Crf);
    b.flag("-preset", Some(FieldId::Preset)).value(PRESETS[v.preset].0, FieldId::Preset);
    let res = resolutions(info);
    if let Some(h) = res[v.resolution.min(res.len() - 1)] {
        b.flag("-vf", Some(FieldId::Resolution)).arg(vec![
            part("scale=-2:", Role::Fixed, None),
            part(h.to_string(), Role::Value, Some(FieldId::Resolution)),
        ]);
    }
    if info.audio.is_some() {
        let sound = Some(FieldId::Sound);
        if v.sound == SOUND_REMOVE {
            b.flag("-an", sound);
        } else {
            b.flag("-c:a", sound).fixed("aac").flag("-b:a", sound).fixed("128k");
        }
    }
    b.output(OpKind::Compress, v, info)
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn default_command() {
        let info = lecture();
        let v = Values::defaults(&info);
        assert_eq!(
            shell(&build(&v, &info)),
            "ffmpeg -i lecture_04.mov -c:v libx264 -crf 23 -preset medium -vf scale=-2:720 \
             -c:a aac -b:a 128k lecture_04_small.mp4"
        );
    }

    #[test]
    fn source_resolution_drops_the_filter() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.resolution = 0;
        assert!(!shell(&build(&v, &info)).contains("-vf"));
    }

    #[test]
    fn no_audio_no_audio_flags_and_no_sound_field() {
        let mut info = lecture();
        info.audio = None;
        let v = Values::defaults(&info);
        assert!(!shell(&build(&v, &info)).contains("-c:a"));
        assert!(fields(&v, &info).iter().all(|f| f.id != FieldId::Sound));
    }

    #[test]
    fn removing_the_sound() {
        let info = lecture();
        let mut v = Values::defaults(&info);
        v.sound = SOUND_REMOVE;
        let s = shell(&build(&v, &info));
        assert!(s.contains(" -an ") && !s.contains("-c:a"), "{s}");
    }

    #[test]
    fn never_scales_up() {
        let mut info = lecture();
        info.video.as_mut().unwrap().height = 720;
        assert_eq!(resolutions(&info), vec![None, Some(480)]);
        assert_eq!(default_resolution(&info), 0);
    }

    #[test]
    fn overwrite_adds_y_and_names_with_spaces_are_quoted() {
        let mut info = lecture();
        info.path = "my lecture.mov".into();
        let mut v = Values::defaults(&info);
        v.overwrite = true;
        let s = shell(&build(&v, &info));
        assert!(s.starts_with("ffmpeg -y -i 'my lecture.mov'"));
        assert!(s.ends_with("'my lecture_small.mp4'"));
    }

    #[test]
    fn the_resolution_value_belongs_to_its_field() {
        let info = lecture();
        let v = Values::defaults(&info);
        let cmd = build(&v, &info);
        let scale = cmd.iter().find(|a| a.text() == "scale=-2:720").unwrap();
        assert_eq!(scale.parts[1].field, Some(FieldId::Resolution));
        assert_eq!(scale.parts[0].field, None);
    }
}
