//! What confines `status` (G12, `Holds/G12`): the scope, the paths its run is given, and
//! the exclusions, the publicly tracked paths under the scope that its run leaves out.
//! Both are decided over listings the handler took, by the ancestor test alone (R9), and
//! an exclusion's form over what the handler found standing at it: no pattern is matched
//! and nothing is asked of Git here.

use std::collections::BTreeSet;
use std::ffi::OsString;

use super::operand;
use super::pathspec;
use super::scope;

/// The pathspec that leaves out one of the exclusions: the path alone, never what lies
/// below it (`Composition/Guards`). A literal exclusion would also leave out a staged
/// deletion below the path, and the untracked and ignored files under a directory standing
/// where the public index holds a file, which G12 shows; where a file or nothing stands at
/// the path the two leave out the same. Where a nested repository stands at the path, a
/// submodule's checkout among them, the path and everything below it: Git's walk reports
/// such a directory as one untracked entry at the path, which the path alone does not
/// leave out, and G12 lists none of what stands there (S8).
pub fn excluded(path: &[u8], nested_repository: bool) -> OsString {
    if nested_repository {
        pathspec::top_literal_exclude(path)
    } else {
        pathspec::top_glob_exclude_exact(path)
    }
}

/// The scope: without user paths, every region path and every staged deletion; with them,
/// for each user path the hidden paths under it and the staged deletions at or below it,
/// the root, the empty path, meaning everything. Each path once, in byte order, and none
/// below another. Empty when nothing hidden or deleted lies at or below any user path.
pub fn scope(
    region: &[Vec<u8>],
    deletions: &BTreeSet<Vec<u8>>,
    user_paths: Option<&[Vec<u8>]>,
) -> Vec<Vec<u8>> {
    let mut scope = BTreeSet::new();
    match user_paths {
        None => scope.extend(region.iter().chain(deletions).cloned()),
        Some(user_paths) => {
            for path in user_paths {
                scope.extend(scope::hidden_under(region, path));
                scope.extend(
                    deletions
                        .iter()
                        .filter(|deleted| path.is_empty() || operand::at_or_below(deleted, path))
                        .cloned(),
                );
            }
        }
    }
    // A path below another adds nothing, because that path's pathspec already reaches it;
    // kept, the deletions below a hidden directory would grow the run's list with the files
    // (E8, G23).
    scope
        .iter()
        .filter(|path| !operand::ancestors(path).any(|above| scope.contains(above)))
        .cloned()
        .collect()
}

/// The exclusions: each publicly tracked path at or below a scope path, unless the private
/// repository tracks it, has its deletion staged, or tracks a file below it, because the
/// status shows those and what stands at them. In byte order, each written by `excluded`.
pub fn exclusions(
    scope: &[Vec<u8>],
    publicly_tracked: &BTreeSet<Vec<u8>>,
    privately_tracked: &BTreeSet<Vec<u8>>,
    deletions: &BTreeSet<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let scope: BTreeSet<&[u8]> = scope.iter().map(Vec::as_slice).collect();
    let above_private = above(privately_tracked);
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
        .cloned()
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
        // `notes/old.md` lies below `notes`, whose pathspec already reaches it.
        assert_eq!(
            scope(&paths(&REGION), &deletions, None),
            paths(&[
                b".env.local",
                b".gitdupe",
                b".vscode",
                b"docs/notes.md",
                b"notes",
                b"src/gone.py",
            ])
        );
        // Whole components: `notes-old/x` is not below `notes`.
        let beside = set(&[b"notes-old/x", b"notes/a/b"]);
        assert_eq!(
            scope(&paths(&[b"notes"]), &beside, None),
            paths(&[b"notes", b"notes-old/x"])
        );
    }

    #[test]
    fn a_user_path_takes_the_hidden_paths_and_the_deletions_at_or_below_it() {
        let region = paths(&REGION);
        let deletions = set(&[
            b"notes/old.md",
            b"notes/sub/z",
            b"docs/gone.md",
            b"notes-old/x",
        ]);
        let scoped = |user: &[&[u8]]| scope(&region, &deletions, Some(&paths(user)));
        // A deletion below a hidden directory adds nothing to it.
        assert_eq!(scoped(&[b"notes"]), paths(&[b"notes"]));
        assert_eq!(
            scoped(&[b"docs"]),
            paths(&[b"docs/gone.md", b"docs/notes.md"])
        );
        // A directory below a hidden one is its own scope, and a deletion within it adds
        // nothing.
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
            paths(&[b".vscode", b"notes"])
        );
        // A user path below another names nothing the other does not.
        assert_eq!(scoped(&[b"notes/sub", b"notes"]), paths(&[b"notes"]));
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
            paths(&[b"notes/shared.md", b"top/other"])
        );
        assert!(exclusions(&[], &publicly_tracked, &privately_tracked, &deletions).is_empty());
    }

    #[test]
    fn a_publicly_tracked_path_with_a_staged_deletion_below_it_is_excluded() {
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
            paths(&[b"h", b"notes/shared.md", b"notes/su", b"notes/sub"])
        );
    }

    #[test]
    fn an_exclusion_leaves_out_the_path_alone_unless_a_nested_repository_stands_there() {
        for path in [&b"notes/sub"[..], b"notes/shared.md", b"h"] {
            assert_eq!(
                excluded(path, false),
                pathspec::top_glob_exclude_exact(path)
            );
            assert_eq!(excluded(path, true), pathspec::top_literal_exclude(path));
        }
    }

    #[test]
    fn gitdupe_is_excluded_only_where_the_scope_covers_it() {
        let region = paths(&[b".gitdupe", b"notes"]);
        let publicly_tracked = set(&[b".gitdupe", b"notes/shared.md"]);
        let none = BTreeSet::new();
        let everything = scope(&region, &none, None);
        assert_eq!(
            exclusions(&everything, &publicly_tracked, &none, &none),
            paths(&[b".gitdupe", b"notes/shared.md"])
        );
        let notes = scope(&region, &none, Some(&paths(&[b"notes"])));
        assert_eq!(
            exclusions(&notes, &publicly_tracked, &none, &none),
            paths(&[b"notes/shared.md"])
        );
        // Privately tracked as well: tracked by both, and shown.
        let private = set(&[b".gitdupe"]);
        assert_eq!(
            exclusions(&everything, &publicly_tracked, &private, &none),
            paths(&[b"notes/shared.md"])
        );
    }
}
