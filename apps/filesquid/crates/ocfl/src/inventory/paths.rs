//! Content-path and logical-path rules (OCFL 1.1 sections 3.5.2 and
//! 3.5.3.1). Both kinds of path share the same shape rules; the inventory
//! reader maps a fault to the code for the kind of path it was reading.

use std::collections::BTreeSet;

/// How a path breaks the shared OCFL path rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathFault {
    /// An element is `.`, `..` or empty (E099 for content, E052 for logical).
    BadElement,
    /// The path begins or ends with `/` (E100 for content, E053 for logical).
    EdgeSlash,
}

/// The rule `path` breaks, or `None` when it is a well-formed OCFL path.
pub(crate) fn path_fault(path: &str) -> Option<PathFault> {
    if path.starts_with('/') || path.ends_with('/') {
        return Some(PathFault::EdgeSlash);
    }
    let bad = |element: &str| element.is_empty() || element == "." || element == "..";
    path.split('/').any(bad).then_some(PathFault::BadElement)
}

/// The first path that repeats an earlier one or conflicts with another, so
/// that one path is the initial part of another (`a/b` and `a/b/c`); `None`
/// when all are unique and non-conflicting. Paths are well formed.
pub(crate) fn first_conflict<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let mut seen = BTreeSet::new();
    for path in paths {
        if !seen.insert(path) {
            return Some(path);
        }
    }
    seen.iter()
        .copied()
        .find(|path| has_listed_parent(path, &seen))
}

/// Whether a directory above `path` is itself listed as a path in `seen`.
fn has_listed_parent(path: &str, seen: &BTreeSet<&str>) -> bool {
    path.match_indices('/')
        .any(|(at, _)| seen.contains(&path[..at]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faults_are_classified() {
        assert_eq!(path_fault("a/b.txt"), None);
        assert_eq!(path_fault("a..b/c"), None);
        assert_eq!(path_fault(""), Some(PathFault::BadElement));
        assert_eq!(path_fault("a/./b"), Some(PathFault::BadElement));
        assert_eq!(path_fault("/a"), Some(PathFault::EdgeSlash));
        assert_eq!(path_fault("a/"), Some(PathFault::EdgeSlash));
    }

    #[test]
    fn conflicts_include_parents_that_sort_apart() {
        assert_eq!(first_conflict(["a/b", "a/b-x", "a/b/c"]), Some("a/b/c"));
        assert_eq!(first_conflict(["a/b", "a/bc"]), None);
        assert_eq!(first_conflict(["x", "x"]), Some("x"));
    }
}
