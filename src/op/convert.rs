use super::*;

/// The containers on offer: the usual three, minus the one the file is in.
pub fn containers(info: &MediaInfo) -> Vec<&'static str> {
    let source = source_ext(info);
    ["mp4", "mkv", "mov"].into_iter().filter(|c| *c != source).collect()
}

fn chosen(v: &Values, info: &MediaInfo) -> &'static str {
    let list = containers(info);
    list[v.container.min(list.len() - 1)]
}

pub fn fields(v: &Values, info: &MediaInfo) -> Vec<FieldSpec> {
    let items: Vec<(&str, &'static str)> = containers(info)
        .into_iter()
        .map(|c| (c, "the streams are copied as they are, nothing is re-encoded"))
        .collect();
    vec![inline(FieldId::Container, "container", &items, v.container)]
}

pub fn build(v: &Values, info: &MediaInfo) -> CommandLine {
    let mut b = Builder::new(v.overwrite);
    b.input(info);
    b.flag("-c", None).fixed("copy");
    b.output(OpKind::Convert, v, info)
}

pub fn output(v: &Values, info: &MediaInfo) -> Vec<Part> {
    vec![
        sibling(info, stem(info)),
        part(format!(".{}", chosen(v, info)), Role::Value, Some(FieldId::Container)),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::testing::lecture;
    use super::*;

    #[test]
    fn mov_to_mp4_without_re_encoding() {
        let info = lecture();
        let v = Values::defaults(&info);
        assert_eq!(shell(&build(&v, &info)), "ffmpeg -i lecture_04.mov -c copy lecture_04.mp4");
    }

    #[test]
    fn never_offers_the_container_it_already_is() {
        let info = lecture();
        assert_eq!(containers(&info), vec!["mp4", "mkv"]);
    }
}
