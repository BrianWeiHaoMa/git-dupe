//! Operands and the lexical cleaning of a path relative to the root (R9): `.` components
//! dropped, `..` components taken with their parent, repeated and trailing slashes
//! removed. No path is resolved through the filesystem, so a symbolic link is a name like
//! any other. An absolute path is cleaned the same way by `clean_absolute`, the one
//! lexical form of a destination's path that does not exist (`destination`).
//!
//! One arrangement serves the three platforms, the standard library answering for each
//! (G29, R11). A separator is what `std::path::is_separator` calls one, so that `\` is `/`
//! on Windows and a byte of a name on Linux and macOS, and is written `/` before anything
//! else reads the path. Absolute is what `Path::is_absolute` says. A drive or a UNC name
//! is what `Path::components` calls a prefix, which on Linux and macOS it never does, so
//! that `C:notes` is a plain name there: on Windows a path that begins with one is outside
//! the root, and so is a word with a root and no drive. Of an absolute path only the drive
//! letter is folded; everything after it is compared byte for byte (`Product`, terms).
//!
//! An operand is a word the user typed for a path. It is literal: a word that Git would
//! read as a pattern or as pathspec magic is refused before it is resolved. It is
//! resolved against the user's prefix, or, when absolute, against the root. `status` and
//! `add` also read a word beginning with `:/` as a path from the root (G12, G13):
//! `rooted_or_literal` and `resolve_rooted` are that reading, and `hide` and `unhide`,
//! which refuse every leading `:` (G11), use `literal` and `resolve`.
//!
//! The ancestor relation between root-relative paths is here too, and written nowhere
//! else: `ancestors` and `at_or_below`, and over a set of paths `any_at_or_below` and
//! `any_below`, are every test of whether a path lies at, below, or above another (R9).
//! They take paths as they cross a boundary, clean, without a leading or trailing slash,
//! the root being the empty path, and compare whole components of the bytes as given:
//! nothing is normalized and the filesystem is never asked. The root is an ancestor of
//! nothing and at or below nothing but itself; a caller that means everything under the
//! root does not ask. The region rule form and its reading back are the keeper's
//! (`keeper::rule`).

use std::collections::BTreeSet;
use std::ops::Bound;
use std::path::{self, Component, Path};
use std::str;

use crate::bytes;

/// Where a cleaned path lies.
#[derive(Debug, PartialEq, Eq)]
pub enum Cleaned {
    /// Below the root: the path, without a leading or trailing slash.
    Inside(Vec<u8>),
    /// The root itself.
    Root,
    /// Outside the root: the cleaned path, beginning with `..`, or absolute, or a path that
    /// begins with a drive or a UNC name, or a word with a root and no drive (G29).
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

/// The root-relative `path` as a line offers it to be typed, from the root, as an operand
/// of `hide`, `unhide`, or `add`: as it stands, or after `./` where a leading `:` would make
/// it no literal path, so that the operand resolves to that path (G25). A path holding a
/// pattern byte has no such form: `literal` still refuses what this returns for it.
pub fn offered(path: &[u8]) -> Vec<u8> {
    if path.starts_with(b":") {
        return [&b"./"[..], path].concat();
    }
    path.to_vec()
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
/// made relative to `root`, as Git prints it (S23), when it lies below it. A path that
/// lies neither at nor below `root` is `Outside`, and so is a word that names a drive and
/// no root, or a root and no drive, which no prefix can be joined to (G29).
pub fn resolve(word: &[u8], prefix: &[u8], root: &Path) -> Cleaned {
    let word = with_slashes(word);
    if !absolute(&word) {
        if word.starts_with(b"/") || prefix_length(&word) > 0 {
            return Cleaned::Outside(word);
        }
        return clean(&[prefix, &word].concat());
    }
    let absolute = clean_absolute(&word);
    let root = clean_absolute(bytes::of(root.as_os_str()));
    let below = if root.ends_with(b"/") {
        absolute.strip_prefix(&root[..])
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

/// `path` with each byte the platform calls a separator written `/`.
pub fn with_slashes(path: &[u8]) -> Vec<u8> {
    path.iter()
        .map(|&byte| {
            if path::is_separator(char::from(byte)) {
                b'/'
            } else {
                byte
            }
        })
        .collect()
}

/// Whether the platform calls `path` absolute.
pub fn absolute(path: &[u8]) -> bool {
    start(path).is_some_and(Path::is_absolute)
}

/// The length of the drive or UNC name that `path`, written with `/`, begins with.
fn prefix_length(path: &[u8]) -> usize {
    match start(path).and_then(|start| start.components().next()) {
        Some(Component::Prefix(prefix)) => prefix.as_os_str().len(),
        _ => 0,
    }
}

/// As much of `path` as is valid UTF-8, which is a name on every platform: a drive and a
/// root stand in its first bytes, and a byte further on that is no name hides neither
/// (G30), so that a `.gitdupe` line of that kind that begins with a drive is outside too.
fn start(path: &[u8]) -> Option<&Path> {
    let valid = str::from_utf8(path).map_or_else(|error| error.valid_up_to(), str::len);
    bytes::name(&path[..valid]).map(Path::new)
}

/// An absolute path cleaned as `clean` cleans one, `..` at the top being the top again: it
/// begins with its drive or UNC name, if any, then `/`, and is that alone for the top. A
/// drive letter is written in upper case, as Git prints it (S23), so that two spellings
/// compare equal; nothing else is folded. The lexical form of a destination's path that
/// does not exist (`guards::destination`) is this one.
pub fn clean_absolute(path: &[u8]) -> Vec<u8> {
    let path = with_slashes(path);
    let (top, rest) = path.split_at(prefix_length(&path));
    let below_the_top = match without_dots(rest) {
        Cleaned::Inside(path) => path,
        Cleaned::Root => Vec::new(),
        Cleaned::Outside(path) => path
            .split(|&byte| byte == b'/')
            .skip_while(|component| *component == b"..")
            .collect::<Vec<_>>()
            .join(&b'/'),
    };
    let mut cleaned = [top, b"/", &below_the_top].concat();
    // No other prefix is two bytes long once its separators are `/`.
    if top.len() == 2 {
        cleaned[0].make_ascii_uppercase();
    }
    cleaned
}

/// Cleans `path`, read relative to the root; a leading slash is one more separator. A path
/// that begins with a drive or a UNC name, with or without that slash, is `Outside`, so
/// that no such path becomes a component of one inside the root (F5, G29).
pub fn clean(path: &[u8]) -> Cleaned {
    let path = with_slashes(path);
    if prefix_length(&path) > 0 {
        return Cleaned::Outside(path);
    }
    match without_dots(&path) {
        Cleaned::Inside(path) if prefix_length(&path) > 0 => Cleaned::Outside(path),
        cleaned => cleaned,
    }
}

/// `path`, written with `/`, without its `.` components, its `..` components with their
/// parents, and its repeated, leading, and trailing slashes: read as what lies below a
/// top, so that no drive or UNC name is asked of it.
fn without_dots(path: &[u8]) -> Cleaned {
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
mod tests;
