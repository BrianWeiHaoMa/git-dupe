//! The hide decision's refusals: what must not become a line of `.gitdupe`, decided over
//! a public listing the caller took before anything is written (F5, G11).
//!
//! Both are decided here and nowhere else, for `hide` and for every other command that
//! hides a path. `.gitdupe` is queried beside the paths only for F5's refusal: it is
//! counted for G11's only when it lies at or below a given path.

use std::collections::BTreeSet;

use super::operand::at_or_below;

/// Why the given paths are not hidden.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `.gitdupe` is publicly tracked and not privately tracked (F5).
    TracksGitdupe,
    /// `count` publicly tracked paths lie at or below the given paths `under`, a path
    /// tracked by both repositories among them (G11).
    PubliclyTracked { count: usize, under: Vec<Vec<u8>> },
}

/// F5's refusal alone, for a command that rewrites `.gitdupe` without hiding a path.
pub fn gitdupe_refusal(
    gitdupe: &[u8],
    privately_tracked: &BTreeSet<Vec<u8>>,
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> Option<Refusal> {
    (publicly_tracked.contains(gitdupe) && !privately_tracked.contains(gitdupe))
        .then_some(Refusal::TracksGitdupe)
}

/// The refusal of hiding `paths`, F5's first, or `None` when they may be hidden.
/// `publicly_tracked` is the public listing under `paths` and `.gitdupe`.
pub fn refusal(
    paths: &[Vec<u8>],
    gitdupe: &[u8],
    privately_tracked: &BTreeSet<Vec<u8>>,
    publicly_tracked: &BTreeSet<Vec<u8>>,
) -> Option<Refusal> {
    if let Some(refusal) = gitdupe_refusal(gitdupe, privately_tracked, publicly_tracked) {
        return Some(refusal);
    }
    let count = publicly_tracked
        .iter()
        .filter(|tracked| paths.iter().any(|path| at_or_below(tracked, path)))
        .count();
    let mut under: Vec<Vec<u8>> = Vec::new();
    for path in paths {
        let holds = publicly_tracked
            .iter()
            .any(|tracked| at_or_below(tracked, path));
        if holds && !under.contains(path) {
            under.push(path.clone());
        }
    }
    (count > 0).then_some(Refusal::PubliclyTracked { count, under })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GITDUPE: &[u8] = b".gitdupe";

    fn set(paths: &[&[u8]]) -> BTreeSet<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn refused(given: &[&[u8]], private: &[&[u8]], public: &[&[u8]]) -> Option<Refusal> {
        refusal(&paths(given), GITDUPE, &set(private), &set(public))
    }

    fn tracked(count: usize, under: &[&[u8]]) -> Option<Refusal> {
        Some(Refusal::PubliclyTracked {
            count,
            under: paths(under),
        })
    }

    #[test]
    fn a_publicly_tracked_path_or_one_below_is_refused_with_their_count() {
        assert_eq!(
            refused(&[b"README.md"], &[], &[b"README.md"]),
            tracked(1, &[b"README.md"])
        );
        assert_eq!(
            refused(&[b"docs"], &[], &[b"docs/design.md", b"docs/api.md"]),
            tracked(2, &[b"docs"])
        );
        // Every publicly tracked path under any given path, each once; the given paths
        // holding one, in the order given, each once.
        assert_eq!(
            refused(
                &[b"notes", b"docs", b"docs/api.md", b"README.md", b"docs"],
                &[],
                &[b"docs/design.md", b"docs/api.md", b"README.md"]
            ),
            tracked(3, &[b"docs", b"docs/api.md", b"README.md"])
        );
    }

    #[test]
    fn a_path_tracked_by_both_counts_and_gitdupe_counts_only_below_a_given_path() {
        assert_eq!(
            refused(&[b"conf"], &[b"conf/local.ini"], &[b"conf/local.ini"]),
            tracked(1, &[b"conf"])
        );
        // `.gitdupe` tracked by both is queried but lies below no given path.
        assert_eq!(
            refused(
                &[b"docs"],
                &[GITDUPE],
                &[b"docs/design.md", b"docs/api.md", GITDUPE]
            ),
            tracked(2, &[b"docs"])
        );
        assert_eq!(refused(&[b"x"], &[GITDUPE], &[GITDUPE]), None);
        assert_eq!(
            refused(&[GITDUPE], &[GITDUPE], &[GITDUPE]),
            tracked(1, &[GITDUPE])
        );
    }

    #[test]
    fn a_publicly_tracked_gitdupe_not_privately_tracked_refuses_first() {
        assert_eq!(
            refused(&[b"x"], &[], &[GITDUPE]),
            Some(Refusal::TracksGitdupe)
        );
        assert_eq!(
            refused(&[b"README.md"], &[], &[GITDUPE, b"README.md"]),
            Some(Refusal::TracksGitdupe)
        );
        assert_eq!(
            gitdupe_refusal(GITDUPE, &set(&[]), &set(&[GITDUPE])),
            Some(Refusal::TracksGitdupe)
        );
        assert_eq!(
            gitdupe_refusal(GITDUPE, &set(&[GITDUPE]), &set(&[GITDUPE])),
            None
        );
        assert_eq!(gitdupe_refusal(GITDUPE, &set(&[]), &set(&[])), None);
    }

    #[test]
    fn nothing_public_at_or_below_the_paths_refuses_nothing() {
        assert_eq!(refused(&[b"notes"], &[], &[]), None);
        // Above, beside, and sharing a name's beginning are not below.
        assert_eq!(
            refused(
                &[b"docs/design.md/x", b"notes"],
                &[],
                &[b"docs/design.md", b"notes-old/a", b"notesx"]
            ),
            None
        );
        // A privately tracked file below the path is no refusal.
        assert_eq!(refused(&[b"notes"], &[b"notes/today.md"], &[]), None);
    }
}
