#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ColumnSemanticProfile<'a> {
    pub(super) row_type_name: &'a str,
    pub(super) column_name: &'a str,
    lower_column_name: String,
    words: SemanticWords,
}

impl<'a> ColumnSemanticProfile<'a> {
    pub(super) fn new(row_type_name: &'a str, column_name: &'a str) -> Self {
        Self {
            row_type_name,
            column_name,
            lower_column_name: column_name.to_ascii_lowercase(),
            words: SemanticWords::parse(column_name),
        }
    }

    pub(super) fn words(&self) -> &[String] {
        self.words.as_slice()
    }

    pub(super) fn has_word(&self, expected: &str) -> bool {
        self.words.contains(expected)
    }

    pub(super) fn has_word_matching(&self, expected: &str) -> bool {
        self.words.contains_matching(expected)
    }

    pub(super) fn has_any_word_matching(&self, expected: &[&str]) -> bool {
        expected
            .iter()
            .any(|expected| self.has_word_matching(expected))
    }

    pub(super) fn first_word_matches(&self, expected: &str) -> bool {
        self.words.first_matches(expected)
    }

    pub(super) fn last_word_is(&self, expected: &str) -> bool {
        self.words.last_is(expected)
    }

    pub(super) fn words_match(&self, expected: &[&str]) -> bool {
        self.words.matches_exact(expected)
    }

    pub(super) fn words_match_any(&self, expected: &[&[&str]]) -> bool {
        self.words.matches_any(expected)
    }

    pub(super) fn lower_column_name_ends_with(&self, suffix: &str) -> bool {
        self.lower_column_name.ends_with(suffix)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SemanticWords {
    words: Vec<String>,
}

impl SemanticWords {
    pub(super) fn parse(value: &str) -> Self {
        Self {
            words: semantic_words(value),
        }
    }

    pub(super) fn as_slice(&self) -> &[String] {
        &self.words
    }

    pub(super) fn contains(&self, expected: &str) -> bool {
        self.words.iter().any(|word| word == expected)
    }

    pub(super) fn matches_any_word(&self, other: &Self) -> bool {
        self.words.iter().any(|left| {
            other
                .words
                .iter()
                .any(|right| semantic_word_matches(left, right))
        })
    }

    pub(super) fn contains_matching(&self, expected: &str) -> bool {
        self.words
            .iter()
            .any(|word| semantic_word_matches(word, expected))
    }

    pub(super) fn first_matches(&self, expected: &str) -> bool {
        self.words
            .first()
            .is_some_and(|word| semantic_word_matches(word, expected))
    }

    pub(super) fn last_is(&self, expected: &str) -> bool {
        self.words.last().is_some_and(|word| word == expected)
    }

    pub(super) fn matches_exact(&self, expected: &[&str]) -> bool {
        semantic_words_match(&self.words, expected)
    }

    pub(super) fn matches_any(&self, expected: &[&[&str]]) -> bool {
        expected.iter().any(|expected| self.matches_exact(expected))
    }
}

pub(super) fn semantic_words_match(words: &[String], expected: &[&str]) -> bool {
    words.len() == expected.len()
        && words
            .iter()
            .map(String::as_str)
            .zip(expected.iter().copied())
            .all(|(word, expected)| word == expected)
}

pub(super) fn semantic_word_matches(left: &str, right: &str) -> bool {
    left == right
        || semantic_word_singular_matches(left, right)
        || semantic_word_singular_matches(right, left)
}

fn semantic_word_singular_matches(plural: &str, singular: &str) -> bool {
    if let Some(stem) = plural.strip_suffix("ies") {
        return singular.len() == stem.len() + 1
            && singular.starts_with(stem)
            && singular.ends_with('y');
    }
    plural.len() > 3
        && plural.ends_with('s')
        && !plural.ends_with("ss")
        && singular == &plural[..plural.len() - 1]
}

pub(super) fn semantic_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_was_lowercase = false;
    for character in value.chars() {
        if character.is_ascii_digit() {
            push_semantic_word(&mut words, &mut current);
            previous_was_lowercase = false;
            continue;
        }
        if !(character.is_ascii_alphanumeric()) {
            push_semantic_word(&mut words, &mut current);
            previous_was_lowercase = false;
            continue;
        }

        if character.is_ascii_uppercase() && previous_was_lowercase {
            push_semantic_word(&mut words, &mut current);
        }
        previous_was_lowercase = character.is_ascii_lowercase();
        current.push(character.to_ascii_lowercase());
    }
    push_semantic_word(&mut words, &mut current);
    words
}

fn push_semantic_word(words: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    if !matches!(
        current.as_str(),
        "id" | "ids"
            | "list"
            | "slot"
            | "override"
            | "name"
            | "names"
            | "key"
            | "type"
            | "data"
            | "table"
            | "row"
            | "entry"
            | "source"
            | "target"
            | "primary"
            | "secondary"
            | "display"
            | "required"
            | "linked"
            | "set"
    ) {
        words.push(std::mem::take(current));
    } else {
        current.clear();
    }
}
