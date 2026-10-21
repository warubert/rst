pub(super) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
}

pub(super) fn unescape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(position) = rest.find('&') {
        result.push_str(&rest[..position]);
        rest = &rest[position..];

        let decoded = rest
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| decode_entity(&rest[1..end]).map(|character| (character, end + 1)));

        match decoded {
            Some((character, consumed)) => {
                result.push(character);
                rest = &rest[consumed..];
            }
            None => {
                result.push('&');
                rest = &rest[1..];
            }
        }
    }

    result.push_str(rest);
    result
}

pub(super) fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}
