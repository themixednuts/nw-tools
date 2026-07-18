use crate::game_system_schema::semantic::ColumnSemanticProfile;

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) struct ColumnRule<T> {
    row_type_name: Option<&'static str>,
    matcher: ColumnMatcher,
    value: T,
}

impl<T: Copy> ColumnRule<T> {
    pub(in crate::game_system_schema) const fn value(self) -> T {
        self.value
    }

    pub(in crate::game_system_schema) const fn exact(
        row_type_name: &'static str,
        column_name: &'static str,
        value: T,
    ) -> Self {
        Self {
            row_type_name: Some(row_type_name),
            matcher: ColumnMatcher::Exact(column_name),
            value,
        }
    }

    pub(in crate::game_system_schema) const fn any_of(
        row_type_name: &'static str,
        column_names: &'static [&'static str],
        value: T,
    ) -> Self {
        Self {
            row_type_name: Some(row_type_name),
            matcher: ColumnMatcher::AnyOf(column_names),
            value,
        }
    }

    pub(in crate::game_system_schema) const fn predicate(
        row_type_name: &'static str,
        predicate: fn(&ColumnSemanticProfile<'_>) -> bool,
        value: T,
    ) -> Self {
        Self {
            row_type_name: Some(row_type_name),
            matcher: ColumnMatcher::Predicate(predicate),
            value,
        }
    }

    pub(in crate::game_system_schema) const fn any_row_exact(
        column_name: &'static str,
        value: T,
    ) -> Self {
        Self {
            row_type_name: None,
            matcher: ColumnMatcher::Exact(column_name),
            value,
        }
    }

    fn value_for(self, profile: &ColumnSemanticProfile<'_>) -> Option<T> {
        (self
            .row_type_name
            .is_none_or(|row_type_name| row_type_name == profile.row_type_name)
            && self.matcher.matches(profile))
        .then_some(self.value)
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::game_system_schema) enum ColumnMatcher {
    Exact(&'static str),
    AnyOf(&'static [&'static str]),
    Predicate(fn(&ColumnSemanticProfile<'_>) -> bool),
}

impl ColumnMatcher {
    fn matches(self, profile: &ColumnSemanticProfile<'_>) -> bool {
        match self {
            Self::Exact(column_name) => profile.column_name == column_name,
            Self::AnyOf(column_names) => column_names.contains(&profile.column_name),
            Self::Predicate(predicate) => predicate(profile),
        }
    }
}

pub(in crate::game_system_schema) fn matching_rule_value<T: Copy>(
    rules: &[ColumnRule<T>],
    profile: &ColumnSemanticProfile<'_>,
) -> Option<T> {
    rules.iter().find_map(|rule| rule.value_for(profile))
}
