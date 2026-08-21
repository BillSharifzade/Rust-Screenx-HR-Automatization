pub fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }

    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '&' {
            let limit = (i + 12).min(chars.len());
            if let Some(end) = (i + 1..limit).find(|&j| chars[j] == ';') {
                let entity: String = chars[i + 1..end].iter().collect();
                if let Some(decoded) = resolve_entity(&entity) {
                    out.push(decoded);
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

fn resolve_entity(entity: &str) -> Option<char> {
    if let Some(number) = entity.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse::<u32>().ok()?,
        };
        return char::from_u32(code);
    }

    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "laquo" => Some('«'),
        "raquo" => Some('»'),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        "hellip" => Some('…'),
        "middot" => Some('·'),
        "bull" => Some('•'),
        "lsquo" | "rsquo" => Some('\''),
        "ldquo" | "rdquo" => Some('"'),
        _ => None,
    }
}

pub fn strip_tags(input: &str) -> String {
    if !input.contains('<') {
        return input.to_string();
    }

    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;

    while i < chars.len() {
        let starts_tag = chars[i] == '<'
            && chars
                .get(i + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == '/');

        if starts_tag {
            if let Some(end) = (i + 1..chars.len()).find(|&j| chars[j] == '>') {
                let tag: String = chars[i + 1..end].iter().collect();
                let closing = tag.starts_with('/');
                let name = tag
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();

                match name.as_str() {
                    "li" if !closing => {
                        push_break(&mut out);
                        out.push_str("- ");
                    }
                    "br" | "p" | "div" | "li" | "tr" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" => {
                        push_break(&mut out)
                    }
                    _ => {}
                }

                i = end + 1;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn push_break(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn normalize_whitespace(input: &str) -> String {
    let normalized = input
        .replace('\u{a0}', " ")
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    let mut lines: Vec<&str> = Vec::new();
    for line in normalized.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if matches!(lines.last(), None | Some(&"")) {
                continue;
            }
            lines.push("");
        } else {
            lines.push(trimmed);
        }
    }
    while matches!(lines.last(), Some(&"")) {
        lines.pop();
    }

    lines.join("\n")
}

pub fn clean_text(input: &str) -> String {
    normalize_whitespace(&strip_tags(&decode_entities(input)))
}

pub fn clean_description(input: &str) -> String {
    let cleaned = clean_text(input).replace(":**", ":**\n");

    let mut out = String::with_capacity(cleaned.len() + 64);
    let mut chars = cleaned.chars().peekable();
    let mut prev: Option<char> = None;

    while let Some(c) = chars.next() {
        let glued = prev.is_some_and(|p| !p.is_whitespace());
        let starts_bullet = c == '-' && chars.peek() == Some(&' ');
        let starts_section = is_pictograph(c);

        if glued && (starts_bullet || starts_section) {
            out.push('\n');
        }

        out.push(c);
        prev = Some(c);
    }

    normalize_whitespace(&out)
}

fn is_pictograph(c: char) -> bool {
    matches!(c as u32, 0x1F300..=0x1FAFF | 0x2600..=0x27BF | 0x2B00..=0x2BFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_numeric_and_named_entities() {
        assert_eq!(decode_entities("&#127881; &amp; &#x41;"), "🎉 & A");
        assert_eq!(decode_entities("R&D at 5 & 6"), "R&D at 5 & 6");
    }

    #[test]
    fn strips_markup_from_structured_fields() {
        assert_eq!(clean_text("<p>знание психологии</p>"), "знание психологии");
        assert_eq!(clean_text("<ul><li>Опыт</li><li>Высшее</li></ul>"), "- Опыт\n- Высшее");
    }

    #[test]
    fn keeps_empty_paragraphs_as_a_single_blank_line() {
        assert_eq!(
            clean_text("<p>1</p>\n<p>\u{a0}</p>\n<p>\u{a0}</p>\n<p>1</p>"),
            "1\n\n1"
        );
    }

    #[test]
    fn reflows_bullets_that_lost_their_newlines() {
        let input = "**Требования:**- Возраст: от 20 до 30 лет.- Опыт 3–5 лет.";
        assert_eq!(
            clean_description(input),
            "**Требования:**\n- Возраст: от 20 до 30 лет.\n- Опыт 3–5 лет."
        );
    }

    #[test]
    fn breaks_before_emoji_section_headings() {
        let input = "...их потребностей.👤 **Требования:**- Возраст: от 20 лет.";
        assert_eq!(
            clean_description(input),
            "...их потребностей.\n👤 **Требования:**\n- Возраст: от 20 лет."
        );
    }

    #[test]
    fn leaves_dash_punctuation_and_ranges_alone() {
        let input = "График 5/2 - комфортный баланс. Опыт 1–3 года.";
        assert_eq!(clean_description(input), input);
    }
}
