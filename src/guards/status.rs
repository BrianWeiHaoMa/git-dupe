//! What confines `status` (G12, `Holds/G12`): the scope, the paths its run is given, and
//! the exclusions, the publicly tracked paths under the scope that its run leaves out.
//! Both are decided over listings the handler took, by the ancestor test alone (R9): no
//! pattern is matched and nothing is asked of Git here.

use std::collections::BTreeSet;
use std::ffi::OsString;

use super::operand;
use super::pathspec;
use super::scope;

/// A publicly tracked path under the scope that `status`'s run leaves out.
#[derive(Debug, PartialEq, Eq)]
pub enum Exclusion {
    /// The path and everything below it.
    Whole(Vec<u8>),
    /// The path alone, because a staged deletion below it stays shown.
    Exact(Vec<u8>),
}

impl Exclusion {
    /// The pathspec that leaves it out (`Composition/Guards`).
    pub fn pathspec(&self) -> OsString {
        match self {
            Exclusion::Whole(path) => pathspec::top_literal_exclude(path),
            Exclusion::Exact(path) => pathspec::top_glob_exclude_exact(path),
        }
    }
}

/// The scope: without user paths, every region path and every staged deletion; with them,
/// for each user path the hidden paths under it and the staged deletions at or below it,
/// the root, the empty path, meaning everything. Each path once, in byte order. Empty when
/// nothing hidden or deleted lies at or below any user path.
pub fn scope(
    region: &[Vec<u8>],
    deletions: &BTreeSet<Vec<u8>>,
    user_paths: Option<&[Vec<u8>]>,
) -> Vec<Vec<u8>> {
    let Some(user_paths) = user_paths else {
        return region
            .iter()
            .chain(deletions)
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    };
    let mut scope = BTreeSet::new();
    for path in user_paths {
        scope.extend(scope::hidden_under(region, path));
        scope.extend(
            deletions
                .iter()
                .filter(|deleted| path.is_empty() || operand::at_or_below(deleted, path))
                .cloned(),
        );
    }
    scope.into_iter().collect()
}

/// The exclusions: each publicly tracked path at or below a scope path, unless the private
/// repository tracks it, has its deletion staged, or tracks a file below it, because the
/// status shows those and what stands at them; the path alone where a staged deletion
/// lies below it, so that the deletion stays shown. In byte order.
pub fn exclusions(
    scope: &[Vec<u8>],
    publicly_tracked: &BTreeSet<Vec<u8>>,
    privately_tracked: &BTreeSet<Vec<u8>>,
    deletions: &BTreeSet<Vec<u8>>,
) -> Vec<Exclusion> {
    let scope: BTreeSet<&[u8]> = scope.iter().map(Vec::as_slice).collect();
    let above_private = above(privately_tracked);
    let above_deleted = above(deletions);
    publicly_tracked
        .iter()
        .filter(|path| {
            scope.contains(path.as_slice())
                || operand::ancestors(path).any(|above| scope.contains(above))
        })
        .filter(|path| {
            !privately_tracked.contains(*path)
                && !deletions.contains(*path)
                && !above_private.contains(path.as_slice())
        })
        .map(|path| {
            if above_deleted.contains(path.as_slice()) {
                Exclusion::Exact(path.clone())
            } else {
                Exclusion::Whole(path.clone())
            }
        })
        .collect()
}

/// Every proper ancestor of the paths.
fn above(paths: &BTreeSet<Vec<u8>>) -> BTreeSet<&[u8]> {
    paths
        .iter()
        .flat_map(|path| operand::ancestors(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn set(paths: &[&[u8]]) -> BTreeSet<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn whole(paths: &[&[u8]]) -> Vec<Exclusion> {
        paths
            .iter()
            .map(|path| Exclusion::Whole(path.to_vec()))
            .collect()
    }

    const REGION: [&[u8]; 5] = [
        b".env.local",
        b".gitdupe",
        b".vscode",
        b"docs/notes.md",
        b"notes",
    ];

    #[test]
    fn without_a_user_path_the_scope_is_every_region_path_and_every_staged_deletion() {
        let deletions = set(&[b"notes/old.md", b"src/gone.py", b"notes"]);
        assert_eq!(
            scope(&paths(&REGION), &deletions, None),
            paths(&[
                b".env.local",
                b".gitdupe",
                b".vscode",
                b"docs/notes.md",
                b"notes",
                b"notes/old.md",
                b"src/gone.py",
            ])
        );
    }

    #[test]
    fn a_user_path_takes_the_hidden_paths_and_the_deletions_at_or_below_it() {
        let region = paths(&REGION);
        let deletions = set(&[b"notes/old.md", b"docs/gone.md", b"notes-old/x"]);
        let scoped = |user: &[&[u8]]| scope(&region, &deletions, Some(&paths(user)));
        assert_eq!(scoped(&[b"notes"]), paths(&[b"notes", b"notes/old.md"]));
        assert_eq!(
            scoped(&[b"docs"]),
            paths(&[b"docs/gone.md", b"docs/notes.md"])
        );
        // A directory below a hidden one is its own scope.
        assert_eq!(scoped(&[b"notes/sub"]), paths(&[b"notes/sub"]));
        assert_eq!(scoped(&[b"docs/gone.md"]), paths(&[b"docs/gone.md"]));
        // Whole components: `notes-old` is not below `notes`.
        assert_eq!(scoped(&[b"notes-old"]), paths(&[b"notes-old/x"]));
        // Nothing hidden or deleted there: an empty scope.
        assert!(scoped(&[b"src"]).is_empty());
        // The root, which `:/` and `.` at the root name, is everything.
        let everything = scope(&region, &deletions, None);
        assert_eq!(scoped(&[b""]), everything);
        assert_eq!(scoped(&[b"src", b""]), everything);
        // Several user paths, each path once.
        assert_eq!(
            scoped(&[b"notes", b".vscode", b"notes"]),
            paths(&[b".vscode", b"notes", b"notes/old.md"])
        );
    }

    #[test]
    fn a_publicly_tracked_path_under_the_scope_is_excluded_unless_private_work_stands_there() {
        let privately_tracked = set(&[b"top/conf/local.ini", b"notes/both.md", b"notes/a.md"]);
        let deletions = set(&[b"notes/old.md"]);
        let publicly_tracked = set(&[
            b"notes/shared.md",
            // A file below which the private repository tracks a file.
            b"top/conf",
            // Tracked by both.
            b"notes/both.md",
            // Its deletion is staged.
            b"notes/old.md",
            b"top/other",
            // Beside the scope, not under it.
            b"notes-old",
            b"README.md",
        ]);
        let scope = paths(&[b"notes", b"notes/old.md", b"top"]);
        assert_eq!(
            exclusions(&scope, &publicly_tracked, &privately_tracked, &deletions),
            whole(&[b"notes/shared.md", b"top/other"])
        );
        assert!(exclusions(&[], &publicly_tracked, &privately_tracked, &deletions).is_empty());
    }

    #[test]
    fn a_publicly_tracked_path_with_a_staged_deletion_below_it_is_excluded_alone() {
        let privately_tracked = set(&[b"notes/a.md", b"top/conf/local.ini"]);
        let deletions = set(&[b"notes/sub/y", b"h/x", b"top/conf/gone", b"notes/subtle"]);
        let publicly_tracked = set(&[
            b"notes/sub",
            b"h",
            // A privately tracked file below it too: not excluded at all.
            b"top/conf",
            // Beside a deletion's parent, not above it.
            b"notes/su",
            b"notes/shared.md",
        ]);
        let scope = paths(&[b"h", b"h/x", b"notes", b"notes/sub/y", b"top"]);
        assert_eq!(
            exclusions(&scope, &publicly_tracked, &privately_tracked, &deletions),
            vec![
                Exclusion::Exact(b"h".to_vec()),
                Exclusion::Whole(b"notes/shared.md".to_vec()),
                Exclusion::Whole(b"notes/su".to_vec()),
                Exclusion::Exact(b"notes/sub".to_vec()),
            ]
        );
        assert_eq!(
            Exclusion::Exact(b"notes/sub".to_vec()).pathspec(),
            pathspec::top_glob_exclude_exact(b"notes/sub")
        );
        assert_eq!(
            Exclusion::Whole(b"notes/su".to_vec()).pathspec(),
            pathspec::top_literal_exclude(b"notes/su")
        );
    }

    #[test]
    fn gitdupe_is_excluded_only_where_the_scope_covers_it() {
        let region = paths(&[b".gitdupe", b"notes"]);
        let publicly_tracked = set(&[b".gitdupe", b"notes/shared.md"]);
        let none = BTreeSet::new();
        let everything = scope(&region, &none, None);
        assert_eq!(
            exclusions(&everything, &publicly_tracked, &none, &none),
            whole(&[b".gitdupe", b"notes/shared.md"])
        );
        let notes = scope(&region, &none, Some(&paths(&[b"notes"])));
        assert_eq!(
            exclusions(&notes, &publicly_tracked, &none, &none),
            whole(&[b"notes/shared.md"])
        );
        // Privately tracked as well: tracked by both, and shown.
        let private = set(&[b".gitdupe"]);
        assert_eq!(
            exclusions(&everything, &publicly_tracked, &private, &none),
            whole(&[b"notes/shared.md"])
        );
    }
}
