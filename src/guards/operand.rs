//! Operands and the lexical cleaning of a path relative to the root (R9): `.` components
//! dropped, `..` components taken with their parent, repeated and trailing slashes
//! removed. No path is resolved through the filesystem, so a symbolic link is a name like
//! any other. An absolute path is cleaned the same way by `clean_absolute`, the one
//! lexical form of a destination's path that does not exist (`destination`).
//!
//! An operand is a word the user typed for a path. It is literal: a word that Git would
//! read as a pattern or as pathspec magic is refused before it is resolved. It is
//! resolved against the user's prefix, or, when absolute, against the root. `status` and
//! `add` also read a word beginning with `:/` as a path from the root (G12, G13):
//! `rooted_or_literal` and `resolve_rooted` are that reading, and `hide` and `unhide`,
//! which refuse every leading `:` (G11), use `literal` and `resolve`.
//!
//! The ancestor relation between root-relative paths is here too, and written nowhere
//! else: `ancestors` and `at_or_below` are every test of whether a path lies at, below,
//! or above another (R9). They take paths as they cross a boundary, clean, without a
//! leading or trailing slash, the root being the empty path, and compare whole
//! components of the bytes as given: nothing is normalized and the filesystem is never
//! asked. The root is an ancestor of nothing and at or below nothing but itself; a caller
//! that means everything under the root does not ask. The region rule form and its
//! reading back are the keeper's (`keeper::rule`).

use std::collections::BTreeSet;
use std::ops::Bound;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Where a cleaned path lies.
#[derive(Debug, PartialEq, Eq)]
pub enum Cleaned {
    /// Below the root: the path, without a leading or trailing slash.
    Inside(Vec<u8>),
    /// The root itself.
    Root,
    /// Outside the root: the cleaned path, beginning with `..`, or absolute.
    Outside(Vec<u8>),
}

/// Why a word is not a literal path.
#[derive(Debug, PartialEq, Eq)]
pub enum NotLiteral {
    /// It holds `*`, `?`, or `[`, which Git's pathspecs and ignore rules read as a
    /// pattern.
    Pattern,
    /// It begins with `:`, which Git's pathspecs read as magic.
    Magic,
}

/// Whether `word` can be read as a literal path: G11's and G13's words that are usage
/// errors whatever the prefix.
pub fn literal(word: &[u8]) -> Result<(), NotLiteral> {
    if word.starts_with(b":") {
        return Err(NotLiteral::Magic);
    }
    if holds_a_pattern_byte(word) {
        return Err(NotLiteral::Pattern);
    }
    Ok(())
}

/// Whether `word` can be read as an operand of `status` or `add`: a path from the root
/// when it begins with `:/`, `:/` alone being the root, and otherwise as `literal` reads
/// it. What follows `:/` is a literal path: a pattern byte there is still a pattern, and
/// no further magic is read.
pub fn rooted_or_literal(word: &[u8]) -> Result<(), NotLiteral> {
    match word.strip_prefix(b":/") {
        Some(path) if holds_a_pattern_byte(path) => Err(NotLiteral::Pattern),
        Some(_) => Ok(()),
        None => literal(word),
    }
}

fn holds_a_pattern_byte(word: &[u8]) -> bool {
    word.iter().any(|byte| matches!(byte, b'*' | b'?' | b'['))
}

/// The root-relative path an operand of `status` or `add` names, `rooted_or_literal`
/// having accepted it: what follows `:/` cleaned from the root whatever the prefix, and
/// any other word as `resolve` resolves it.
pub fn resolve_rooted(word: &[u8], prefix: &[u8], root: &Path) -> Cleaned {
    match word.strip_prefix(b":/") {
        Some(path) => clean(path),
        None => resolve(word, prefix, root),
    }
}

/// The root-relative path an operand names: joined to `prefix`, the user's directory
/// relative to the root as Git prints it, and cleaned; or, when absolute, cleaned and then
/// made relative to `root` when it lies below it. A path that lies neither at nor below
/// `root` is `Outside`.
pub fn resolve(word: &[u8], prefix: &[u8], root: &Path) -> Cleaned {
    if !word.starts_with(b"/") {
        return clean(&[prefix, word].concat());
    }
    let absolute = clean_absolute(word);
    let root = clean_absolute(root.as_os_str().as_bytes());
    let below = if root == b"/" {
        absolute.strip_prefix(b"/")
    } else if absolute == root {
        Some(&b""[..])
    } else {
        absolute
            .strip_prefix(&root[..])
            .and_then(|rest| rest.strip_prefix(b"/"))
    };
    match below {
        Some(b"") => Cleaned::Root,
        Some(path) => Cleaned::Inside(path.to_vec()),
        None => Cleaned::Outside(absolute),
    }
}

/// An absolute path cleaned as `clean` cleans one, `..` at `/` being `/` again: it begins
/// with `/`, and is `/` alone for the top. The lexical form of a destination's path that
/// does not exist (`guards::destination`) is this one.
pub fn clean_absolute(path: &[u8]) -> Vec<u8> {
    let below_the_top = match clean(path) {
        Cleaned::Inside(path) => path,
        Cleaned::Root => Vec::new(),
        Cleaned::Outside(path) => path
            .split(|&byte| byte == b'/')
            .skip_while(|component| *component == b"..")
            .collect::<Vec<_>>()
            .join(&b'/'),
    };
    [&b"/"[..], &below_the_top].concat()
}

/// Cleans `path`, read relative to the root; a leading slash is one more separator.
pub fn clean(path: &[u8]) -> Cleaned {
    let mut components: Vec<&[u8]> = Vec::new();
    for component in path.split(|&byte| byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                if components.last().is_some_and(|last| *last != b"..") {
                    components.pop();
                } else {
                    components.push(component);
                }
            }
            _ => components.push(component),
        }
    }
    let joined = components.join(&b'/');
    match components.first() {
        None => Cleaned::Root,
        Some(&b"..") => Cleaned::Outside(joined),
        Some(_) => Cleaned::Inside(joined),
    }
}

/// The proper ancestors of `path` below the root, outermost first: `a` then `a/b` for
/// `a/b/c`, nothing for `a` or the root.
pub fn ancestors(path: &[u8]) -> impl Iterator<Item = &[u8]> {
    path.iter()
        .enumerate()
        .filter(|(_, byte)| **byte == b'/')
        .map(|(at, _)| &path[..at])
}

/// Whether `path` is `ancestor` or lies below it, by whole components.
pub fn at_or_below(path: &[u8], ancestor: &[u8]) -> bool {
    path.strip_prefix(ancestor)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"))
}

/// Whether one of `paths` is `ancestor` or lies below it.
pub fn any_at_or_below(paths: &BTreeSet<Vec<u8>>, ancestor: &[u8]) -> bool {
    paths.contains(ancestor) || any_below(paths, ancestor)
}

/// Whether one of `paths` lies below `ancestor`: the paths below it stand together in
/// byte order, right after `ancestor/`.
pub fn any_below(paths: &BTreeSet<Vec<u8>>, ancestor: &[u8]) -> bool {
    let below = [ancestor, b"/"].concat();
    paths
        .range::<[u8], _>((Bound::Included(&below[..]), Bound::Unbounded))
        .next()
        .is_some_and(|path| path.starts_with(&below))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(path: &[u8]) -> Cleaned {
        Cleaned::Inside(path.to_vec())
    }

    #[test]
    fn dots_and_repeated_and_trailing_slashes_are_removed() {
        assert_eq!(clean(b"notes"), inside(b"notes"));
        assert_eq!(clean(b"a/./b//"), inside(b"a/b"));
        assert_eq!(clean(b"./a/b/./"), inside(b"a/b"));
        assert_eq!(clean(b"//x"), inside(b"x"));
        assert_eq!(clean(b"a/b/../c"), inside(b"a/c"));
        assert_eq!(clean(b"a/.../b"), inside(b"a/.../b"));
    }

    #[test]
    fn an_absolute_path_is_cleaned_the_same_way_and_dot_dot_at_the_top_is_the_top() {
        for (path, cleaned) in [
            (&b"/w/repo"[..], &b"/w/repo"[..]),
            (b"/w/./repo//sub/../", b"/w/repo"),
            (b"/w/repo/absent/..", b"/w/repo"),
            (b"//w", b"/w"),
            (b"/", b"/"),
            (b"/..", b"/"),
            (b"/../../w/..", b"/"),
            (b"/../w/x.git", b"/w/x.git"),
            (b"/w/...git", b"/w/...git"),
        ] {
            assert_eq!(clean_absolute(path), cleaned, "{}", path.escape_ascii());
        }
    }

    #[test]
    fn the_root_and_what_lies_outside_it_are_told_apart() {
        for root in [&b""[..], b".", b"/", b"a/..", b"./a/b/../.."] {
            assert_eq!(clean(root), Cleaned::Root, "{}", root.escape_ascii());
        }
        assert_eq!(clean(b"../x"), Cleaned::Outside(b"../x".to_vec()));
        assert_eq!(clean(b"a/../../x"), Cleaned::Outside(b"../x".to_vec()));
        assert_eq!(clean(b"../../"), Cleaned::Outside(b"../..".to_vec()));
    }

    #[test]
    fn a_word_holding_a_pattern_byte_or_beginning_with_a_colon_is_not_literal() {
        for word in [&b"a*"[..], b"a?", b"a[1]", b"*", b"docs/[x", b"?"] {
            assert_eq!(
                literal(word),
                Err(NotLiteral::Pattern),
                "{}",
                word.escape_ascii()
            );
        }
        for word in [&b":x"[..], b":/x", b":", b":(top)x", b":*"] {
            assert_eq!(
                literal(word),
                Err(NotLiteral::Magic),
                "{}",
                word.escape_ascii()
            );
        }
        for word in [
            &b"a:b"[..],
            b"-x",
            b"notes/",
            b"",
            b"a]",
            b"caf\xe9",
            b"x!#\\ ",
        ] {
            assert_eq!(literal(word), Ok(()), "{}", word.escape_ascii());
        }
    }

    #[test]
    fn a_word_beginning_with_colon_slash_is_a_literal_path_from_the_root() {
        for word in [
            &b":/"[..],
            b":/notes",
            b":/:x",
            b":/!x",
            b":/a b\xe9",
            b":/../x",
        ] {
            assert_eq!(rooted_or_literal(word), Ok(()), "{}", word.escape_ascii());
        }
        for word in [&b":/a*"[..], b":/x?", b":/[x]"] {
            assert_eq!(
                rooted_or_literal(word),
                Err(NotLiteral::Pattern),
                "{}",
                word.escape_ascii()
            );
        }
        for word in [&b":x"[..], b":", b":(top)x", b"::/x"] {
            assert_eq!(
                rooted_or_literal(word),
                Err(NotLiteral::Magic),
                "{}",
                word.escape_ascii()
            );
        }
        assert_eq!(rooted_or_literal(b"a*"), Err(NotLiteral::Pattern));
        assert_eq!(rooted_or_literal(b"notes/"), Ok(()));

        let root = Path::new("/w/repo");
        assert_eq!(resolve_rooted(b":/", b"sub/", root), Cleaned::Root);
        assert_eq!(
            resolve_rooted(b":/notes/x/", b"sub/", root),
            inside(b"notes/x")
        );
        assert_eq!(resolve_rooted(b":/./a//b", b"", root), inside(b"a/b"));
        assert_eq!(resolve_rooted(b":/:x", b"sub/", root), inside(b":x"));
        assert_eq!(resolve_rooted(b":/sub/..", b"sub/", root), Cleaned::Root);
        assert_eq!(
            resolve_rooted(b":/../x", b"sub/", root),
            Cleaned::Outside(b"../x".to_vec())
        );
        // Read from the root, never as an absolute path.
        assert_eq!(
            resolve_rooted(b":/w/repo/x", b"", root),
            inside(b"w/repo/x")
        );
        assert_eq!(resolve_rooted(b"x", b"sub/", root), inside(b"sub/x"));
        assert_eq!(resolve_rooted(b"..", b"sub/", root), Cleaned::Root);
    }

    #[test]
    fn a_relative_operand_is_joined_to_the_prefix_and_cleaned() {
        let root = Path::new("/w/repo");
        assert_eq!(resolve(b"notes/", b"", root), inside(b"notes"));
        assert_eq!(resolve(b"./a//b/", b"sub/", root), inside(b"sub/a/b"));
        assert_eq!(resolve(b"../x", b"sub/", root), inside(b"x"));
        assert_eq!(resolve(b"-x", b"sub/dir/", root), inside(b"sub/dir/-x"));
        assert_eq!(resolve(b"", b"sub/", root), inside(b"sub"));
        assert_eq!(resolve(b"..", b"sub/", root), Cleaned::Root);
        assert_eq!(resolve(b".", b"", root), Cleaned::Root);
        assert_eq!(resolve(b"", b"", root), Cleaned::Root);
        assert_eq!(resolve(b"a/..", b"", root), Cleaned::Root);
        assert_eq!(
            resolve(b"../x", b"", root),
            Cleaned::Outside(b"../x".to_vec())
        );
        assert_eq!(
            resolve(b"../../x", b"sub/", root),
            Cleaned::Outside(b"../x".to_vec())
        );
    }

    #[test]
    fn an_absolute_operand_is_cleaned_then_made_relative_to_the_root() {
        let root = Path::new("/w/repo");
        assert_eq!(resolve(b"/w/repo/sub/y", b"sub/", root), inside(b"sub/y"));
        assert_eq!(resolve(b"/w/repo//a/./b/", b"", root), inside(b"a/b"));
        assert_eq!(resolve(b"/w/x/../repo/n", b"", root), inside(b"n"));
        assert_eq!(resolve(b"/../../w/repo/n", b"", root), inside(b"n"));
        assert_eq!(resolve(b"/..", b"", root), Cleaned::Outside(b"/".to_vec()));
        assert_eq!(resolve(b"/w/repo", b"sub/", root), Cleaned::Root);
        assert_eq!(resolve(b"/w/repo/", b"", root), Cleaned::Root);
        assert_eq!(resolve(b"/w/repo/a/..", b"", root), Cleaned::Root);
        // Lexically, never through the filesystem: a component is compared whole.
        for outside in [
            &b"/w/repository/x"[..],
            b"/w/rep",
            b"/w",
            b"/",
            b"/w/repo/../x",
        ] {
            assert!(
                matches!(resolve(outside, b"", root), Cleaned::Outside(_)),
                "{}",
                outside.escape_ascii()
            );
        }
        assert_eq!(
            resolve(b"/w/repo/../x", b"", root),
            Cleaned::Outside(b"/w/x".to_vec())
        );
        // Every absolute path lies at or below a root that is `/`.
        assert_eq!(resolve(b"/etc/x", b"", Path::new("/")), inside(b"etc/x"));
        assert_eq!(resolve(b"/", b"", Path::new("/")), Cleaned::Root);
        let root = Path::new(std::ffi::OsStr::from_bytes(b"/caf\xe9"));
        assert_eq!(resolve(b"/caf\xe9/r", b"", root), inside(b"r"));
    }

    #[test]
    fn an_ancestor_is_a_whole_leading_component_path_and_the_root_is_none() {
        let above = |path: &[u8]| ancestors(path).map(<[u8]>::to_vec).collect::<Vec<_>>();
        assert_eq!(above(b"a/b/c"), [b"a".to_vec(), b"a/b".to_vec()]);
        assert_eq!(
            above(b"docs/design.md/x"),
            [b"docs".to_vec(), b"docs/design.md".to_vec()]
        );
        assert!(above(b"a").is_empty());
        assert!(above(b"").is_empty());

        assert!(at_or_below(b"notes/a/b", b"notes"));
        assert!(at_or_below(b"notes", b"notes"));
        // Lexical: a file's path can have paths below it.
        assert!(at_or_below(b"docs/design.md/x", b"docs/design.md"));
        for beside in [&b"notesx"[..], b"notes-old", b"notes.d", b"note"] {
            assert!(!at_or_below(beside, b"notes"), "{}", beside.escape_ascii());
        }
        assert!(!at_or_below(b"notes", b"notes/a"));
        // The root is at or below nothing but itself, and no path lies below it here.
        assert!(!at_or_below(b"a", b""));
        assert!(!at_or_below(b"", b"a"));
        assert!(at_or_below(b"", b""));
    }

    #[test]
    fn a_set_holds_a_path_at_or_below_another_by_whole_components() {
        let paths: BTreeSet<Vec<u8>> = [&b"notes-old/x"[..], b"notes.d", b"notes/a/b", b"z"]
            .map(<[u8]>::to_vec)
            .into();
        assert!(any_at_or_below(&paths, b"notes"));
        assert!(any_at_or_below(&paths, b"notes/a"));
        assert!(any_at_or_below(&paths, b"notes/a/b"));
        assert!(any_at_or_below(&paths, b"z"));
        // `notes-old` and `notes.d` sort between `notes` and `notes/` and lie beside it.
        let beside: BTreeSet<Vec<u8>> =
            [&b"notes-old/x"[..], b"notes.d"].map(<[u8]>::to_vec).into();
        assert!(!any_at_or_below(&beside, b"notes"));
        assert!(!any_at_or_below(&paths, b"notes/a/b/c"));
        assert!(!any_at_or_below(&paths, b"note"));
        assert!(!any_at_or_below(&paths, b""));
        // Below excludes the path itself.
        assert!(any_below(&paths, b"notes/a"));
        assert!(!any_below(&paths, b"notes/a/b"));
        assert!(!any_below(&paths, b"z"));
    }

    #[test]
    fn every_other_byte_is_part_of_the_path() {
        assert_eq!(clean(b" trail \r"), inside(b" trail \r"));
        assert_eq!(clean(b"caf\xe9/#x/*"), inside(b"caf\xe9/#x/*"));
        assert_eq!(clean(b"a\\b"), inside(b"a\\b"));
    }
}
