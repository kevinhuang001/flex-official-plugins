use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;

pub struct IgnoreSet {
    names: GlobSet,
    paths: GlobSet,
}

impl IgnoreSet {
    pub fn new(patterns: &[String]) -> Self {
        let mut names = GlobSetBuilder::new();
        let mut paths = GlobSetBuilder::new();
        for raw in patterns {
            let pattern = raw.trim();
            if pattern.is_empty() || pattern.starts_with('#') {
                continue;
            }
            let Ok(glob) = GlobBuilder::new(pattern)
                .case_insensitive(true)
                .literal_separator(false)
                .build()
            else {
                continue;
            };
            if pattern.contains('/') {
                paths.add(glob);
            } else {
                names.add(glob);
            }
        }
        Self {
            names: names
                .build()
                .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap()),
            paths: paths
                .build()
                .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap()),
        }
    }

    pub fn is_ignored(&self, relative: &Path) -> bool {
        let name = relative
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        self.names.is_match(name.as_ref())
            || self
                .paths
                .is_match(relative.to_string_lossy().replace('\\', "/"))
    }
}
