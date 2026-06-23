use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTypePath {
    pub scope_segments: Vec<String>,
    pub file_stem: String,
}

impl LayoutTypePath {
    #[must_use]
    pub fn new(mut scope_segments: Vec<String>, file_stem: String) -> Self {
        if scope_segments
            .last()
            .is_some_and(|segment| segment == &file_stem)
        {
            scope_segments.pop();
        }
        Self {
            scope_segments,
            file_stem,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutPathSet {
    paths: BTreeSet<Vec<String>>,
}

impl LayoutPathSet {
    #[must_use]
    pub fn from_paths(paths: impl IntoIterator<Item = Vec<String>>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn from_directory_prefixes(paths: impl IntoIterator<Item = Vec<String>>) -> Self {
        let mut path_set = Self::default();
        for path in paths {
            path_set.insert_directory_prefixes(&path);
        }
        path_set
    }

    #[must_use]
    pub fn contains(&self, path: &[String]) -> bool {
        self.paths.contains(path)
    }

    #[must_use]
    pub fn contains_self_or_descendant(&self, prefix: &[String]) -> bool {
        self.paths
            .iter()
            .any(|path| layout_path_starts_with(path, prefix))
    }

    #[must_use]
    pub fn contains_descendant(&self, prefix: &[String]) -> bool {
        self.paths
            .iter()
            .any(|path| path.len() > prefix.len() && layout_path_starts_with(path, prefix))
    }

    fn insert_directory_prefixes(&mut self, path: &[String]) {
        for depth in 1..=path.len() {
            self.paths.insert(path[..depth].to_vec());
        }
    }
}

#[must_use]
pub fn layout_path_starts_with(path: &[String], prefix: &[String]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_prefix_set_finds_exact_and_descendant_paths() {
        let paths = LayoutPathSet::from_directory_prefixes(vec![
            vec![
                "components".to_owned(),
                "faceted_components".to_owned(),
                "player_component".to_owned(),
            ],
            vec!["az".to_owned(), "components".to_owned()],
        ]);

        assert!(paths.contains(&["components".to_owned()]));
        assert!(paths.contains(&["components".to_owned(), "faceted_components".to_owned()]));
        assert!(paths.contains_self_or_descendant(&[
            "components".to_owned(),
            "faceted_components".to_owned()
        ]));
        assert!(
            paths.contains_descendant(&["components".to_owned(), "faceted_components".to_owned()])
        );
        assert!(!paths.contains_descendant(&[
            "components".to_owned(),
            "faceted_components".to_owned(),
            "player_component".to_owned()
        ]));
    }
}
