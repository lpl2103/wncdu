//! File and directory exclusion pattern matching.

use glob::Pattern;
use std::path::Path;

/// Filter configuration for skipping files and directories during scan.
#[derive(Debug, Clone, Default)]
pub struct ExclusionFilter {
    /// Glob patterns specified by the user (e.g. `*.tmp`, `node_modules`).
    patterns: Vec<Pattern>,
    /// Whether to exclude directories containing a `CACHEDIR.TAG` file.
    pub exclude_caches: bool,
    /// Whether to exclude hidden files and folders.
    pub exclude_hidden: bool,
}

impl ExclusionFilter {
    /// Creates a new empty filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a glob pattern string to the filter.
    ///
    /// # Errors
    /// Returns an error if the pattern string is not a valid glob pattern.
    pub fn add_pattern(&mut self, pat: &str) -> Result<(), glob::PatternError> {
        let pattern = Pattern::new(pat)?;
        self.patterns.push(pattern);
        Ok(())
    }

    /// Checks if a given file or directory should be excluded from analysis.
    #[must_use]
    pub fn is_excluded(&self, full_path: &Path, name: &str, is_dir: bool, is_hidden: bool) -> bool {
        if self.exclude_hidden && is_hidden {
            return true;
        }

        // Check glob patterns against the item name and against full path
        for pat in &self.patterns {
            if pat.matches(name) || pat.matches_path(full_path) {
                return true;
            }
        }

        // Cache directory exclusion standard (CACHEDIR.TAG)
        if is_dir && self.exclude_caches {
            let tag = full_path.join("CACHEDIR.TAG");
            if tag.exists() {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_exclusions() {
        let mut filter = ExclusionFilter::new();
        filter.add_pattern("*.tmp").unwrap();
        filter.add_pattern("target").unwrap();

        assert!(filter.is_excluded(Path::new("C:\\temp\\file.tmp"), "file.tmp", false, false));
        assert!(filter.is_excluded(Path::new("C:\\project\\target"), "target", true, false));
        assert!(!filter.is_excluded(Path::new("C:\\project\\src"), "src", true, false));
    }

    #[test]
    fn test_hidden_exclusion() {
        let mut filter = ExclusionFilter::new();
        filter.exclude_hidden = true;

        assert!(filter.is_excluded(Path::new("C:\\.git"), ".git", true, true));
        assert!(!filter.is_excluded(Path::new("C:\\docs"), "docs", true, false));
    }
}
