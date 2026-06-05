use pinyin::ToPinyin;

#[derive(Debug, Eq, PartialEq)]
pub struct PinyinIndex {
    pub full: String,
    pub initials: String,
}

pub fn build_pinyin_index(text: &str) -> PinyinIndex {
    let mut full = String::new();
    let mut initials = String::new();

    for character in text.chars() {
        if let Some(pinyin) = character.to_pinyin() {
            full.push_str(pinyin.plain());
            initials.push_str(pinyin.first_letter());
        } else if should_keep_literal(character) {
            for lower in character.to_lowercase() {
                full.push(lower);
                initials.push(lower);
            }
        }
    }

    PinyinIndex { full, initials }
}

pub fn pinyin_matches(query: &str, text: &str) -> bool {
    let query = normalize_pinyin_query(query);
    if query.is_empty()
        || !query
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return false;
    }

    let index = build_pinyin_index(text);
    index.full.contains(&query) || index.initials.contains(&query)
}

pub fn pinyin_match_score(query: &str, text: &str) -> f32 {
    let query = normalize_pinyin_query(query);
    if query.is_empty()
        || !query
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return 0.0;
    }

    let index = build_pinyin_index(text);

    if index.full == query || index.initials == query {
        0.32
    } else if index.full.starts_with(&query) || index.initials.starts_with(&query) {
        0.24
    } else if index.full.contains(&query) || index.initials.contains(&query) {
        0.18
    } else {
        0.0
    }
}

fn normalize_pinyin_query(query: &str) -> String {
    query
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect()
}

fn should_keep_literal(character: char) -> bool {
    character.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinyin_full_matches_chinese_title() {
        assert!(pinyin_matches("jishiben", "记事本"));
    }

    #[test]
    fn pinyin_initials_match_chinese_title() {
        assert!(pinyin_matches("jsb", "记事本"));
    }

    #[test]
    fn pinyin_matching_is_case_insensitive() {
        assert!(pinyin_matches("JiShiBen", "记事本"));
        assert!(pinyin_matches("JSB", "记事本"));
    }

    #[test]
    fn pinyin_matches_mixed_chinese_and_english_title() {
        assert_eq!(build_pinyin_index("AI 翻译").full, "aifanyi");
        assert!(pinyin_matches("aifanyi", "AI 翻译"));
        assert!(pinyin_matches("aify", "AI 翻译"));
    }

    #[test]
    fn pinyin_does_not_match_unrelated_query() {
        assert!(!pinyin_matches("xyz", "记事本"));
    }
}
