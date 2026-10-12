//! The form of a destination (G18, `Holds/G18`, S13): what a word typed after `push`,
//! `pull`, `fetch`, `remote`, or `clone`, a URL a repository's configuration gives a
//! remote, or a default remote's value is compared by. It is a comparison of names, not
//! a reading of Git's transports (N5): Git decides what a URL means.
//!
//! What the code below cannot show:
//!
//! - A destination has one or more forms. A URL with a scheme has the form of its host and
//!   path; a `file://` URL, its scheme spelled so exactly as `Holds/G18` writes it, also
//!   has the local forms of the path it names: Git hands a scheme spelled otherwise,
//!   `FILE://` included, to the remote helper of that name and opens no local path; a
//!   `[user@]host:path` word has the form of its host and path; anything else is a local
//!   path. A word holding `://` whose part before it is no scheme Git reads is a local
//!   path, as Git reads it.
//! - Each `%XX` after a `://` is decoded first, as Git decodes a `file://` or `ssh://` URL;
//!   a word without `://` is never decoded.
//! - A local path has the canonical path of each path Git opens for it: its trailing
//!   slashes dropped, the path itself, with `.git` appended, and each of those with
//!   `/.git` appended. A relative one is read from the root and also from the user's
//!   directory where that lies outside the root (`ReadFrom`). An empty one is the root.
//! - A local path that is `~` or begins with `~/` is read as Git reads it: its trailing
//!   slashes dropped, then the value of `HOME` in place of its `~`, then `.git` and
//!   `/.git` appended to what that gives; where `HOME` is unset Git opens nothing for it,
//!   and it has no form. One beginning with `~<user>` is read as written, a relative path:
//!   Git takes that user's home from the password database, which the standard library
//!   cannot ask, and the product admits it as not seen through.
//! - The canonical path is the real path, asked of the path as written, so that `link/..`
//!   is where the link's target's parent is; where the real path cannot be had — nothing
//!   there, or a path Git cannot open either, as one beyond a component it may not
//!   search or in a loop of links — it is the path cleaned as text (`operand`). This and
//!   `places` are the only parts of `Guards` that read the filesystem, and they only read.

use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use super::operand;

/// Where a relative local path is read from: the root, because Git runs a command from
/// the top of its work tree, and also the user's directory when that lies outside the
/// root, where `push` and `fetch` read one instead (S13). Both absolute. And `home`, the
/// value of `HOME` every run passes through, read in place of a leading `~` (S13).
#[derive(Clone, Copy, Debug)]
pub struct ReadFrom<'a> {
    pub root: &'a Path,
    pub outside: Option<&'a Path>,
    pub home: Option<&'a [u8]>,
}

/// One form of a destination: two destinations name the same place when one of their
/// forms is equal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Form {
    /// A URL's host lowered in ASCII and its path without a trailing `.git` and trailing
    /// slashes, its scheme, user, and port removed.
    Url { host: Vec<u8>, path: Vec<u8> },
    /// The canonical path of one path Git opens for a local destination: absolute.
    Local(Vec<u8>),
}

/// Every form of `destination`, its relative local path read `from` where `ReadFrom`
/// says.
pub fn forms(destination: &[u8], from: ReadFrom) -> Vec<Form> {
    if let Some((scheme, after)) = with_scheme(destination) {
        let after = decoded(after);
        let (authority, path) = match after.iter().position(|&byte| byte == b'/') {
            Some(slash) => after.split_at(slash),
            None => (&after[..], &b""[..]),
        };
        let mut forms = vec![url(host(authority), path)];
        if scheme == b"file" && !path.is_empty() {
            forms.extend(local(path, from));
        }
        return forms;
    }
    if let Some((host, path)) = scp(destination) {
        let path = if path.starts_with(b"/") {
            path.to_vec()
        } else {
            [&b"/"[..], path].concat()
        };
        return vec![url(host, &path)];
    }
    local(destination, from)
}

/// The canonical path of `path`, absolute: its real path when it can be had, and the
/// path cleaned as text otherwise.
pub fn canonical(path: &[u8]) -> Vec<u8> {
    match fs::canonicalize(Path::new(std::ffi::OsStr::from_bytes(path))) {
        Ok(real) => real.into_os_string().into_vec(),
        Err(_) => operand::clean_absolute(path),
    }
}

/// The scheme and what follows its `://`, when the word is a URL as Git's `is_url` reads
/// one: a letter or digit, then letters, digits, `+`, `-`, or `.` up to the first `:`,
/// then `//`.
fn with_scheme(word: &[u8]) -> Option<(&[u8], &[u8])> {
    let colon = word.iter().position(|&byte| byte == b':')?;
    let scheme = &word[..colon];
    let (first, rest) = scheme.split_first()?;
    let scheme_byte = |byte: &u8| byte.is_ascii_alphanumeric() || b"+-.".contains(byte);
    if !first.is_ascii_alphanumeric() || !rest.iter().all(scheme_byte) {
        return None;
    }
    Some((scheme, word[colon + 1..].strip_prefix(b"//")?))
}

/// The host and path of a `[user@]host:path` word: no `://` in it, and a `:` before any
/// `/`. A host in brackets keeps its own colons, the path following the first `:` after
/// its `]`.
fn scp(word: &[u8]) -> Option<(&[u8], &[u8])> {
    if word.windows(3).any(|window| window == b"://") {
        return None;
    }
    let colon = word.iter().position(|&byte| byte == b':')?;
    if word[..colon].contains(&b'/') {
        return None;
    }
    let open = if word.starts_with(b"[") {
        Some(0)
    } else {
        word.windows(2)
            .position(|window| window == b"@[")
            .map(|at| at + 1)
    };
    let separator = open
        .and_then(|open| {
            let close = open + word[open..].iter().position(|&byte| byte == b']')?;
            let after = word[close..].iter().position(|&byte| byte == b':')?;
            Some(close + after)
        })
        .unwrap_or(colon);
    Some((host(&word[..separator]), &word[separator + 1..]))
}

/// The host of `[user@]host[:port]`: after the last `@`, inside its brackets when it
/// has them, else before its first `:`.
fn host(authority: &[u8]) -> &[u8] {
    let host = match authority.iter().rposition(|&byte| byte == b'@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    if let Some(inside) = host.strip_prefix(b"[")
        && let Some(close) = inside.iter().position(|&byte| byte == b']')
    {
        return &inside[..close];
    }
    match host.iter().position(|&byte| byte == b':') {
        Some(port) => &host[..port],
        None => host,
    }
}

/// The form of a URL's host and path: the host lowered in ASCII; the path, its case
/// kept, without trailing slashes, a trailing `.git`, and the slashes that then end it.
fn url(host: &[u8], path: &[u8]) -> Form {
    let path = without_trailing_slashes(path);
    let path = path.strip_suffix(b".git").unwrap_or(path);
    Form::Url {
        host: host.to_ascii_lowercase(),
        path: without_trailing_slashes(path).to_vec(),
    }
}

fn without_trailing_slashes(path: &[u8]) -> &[u8] {
    let end = path
        .iter()
        .rposition(|&byte| byte != b'/')
        .map_or(0, |last| last + 1);
    &path[..end]
}

/// `%XX` decoded to its byte wherever `XX` are two hexadecimal digits; every other byte
/// kept.
fn decoded(text: &[u8]) -> Vec<u8> {
    let digit = |byte: u8| char::from(byte).to_digit(16);
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if text[at] == b'%'
            && let (Some(high), Some(low)) = (
                text.get(at + 1).copied().and_then(digit),
                text.get(at + 2).copied().and_then(digit),
            )
        {
            out.push(u8::try_from(high * 16 + low).expect("two hexadecimal digits"));
            at += 3;
            continue;
        }
        out.push(text[at]);
        at += 1;
    }
    out
}

/// The local forms of `path`: joined to each place a relative one is read from, its
/// trailing slashes dropped, then the canonical path of each of the four paths Git opens.
/// One that is `~` or begins with `~/` has its trailing slashes dropped first and `HOME`
/// read in place of its `~`, and none where `HOME` is unset.
fn local(path: &[u8], from: ReadFrom) -> Vec<Form> {
    let opened: Vec<Vec<u8>> = match path.strip_prefix(b"~") {
        Some(rest) if rest.is_empty() || rest.starts_with(b"/") => match from.home {
            Some(home) => joined(&[home, without_trailing_slashes(rest)].concat(), from),
            None => Vec::new(),
        },
        _ => joined(path, from)
            .iter()
            .map(|joined| match without_trailing_slashes(joined) {
                b"" => b"/".to_vec(),
                kept => kept.to_vec(),
            })
            .collect(),
    };
    opened
        .iter()
        .flat_map(|opened| {
            [
                opened.clone(),
                [opened, &b".git"[..]].concat(),
                [opened, &b"/.git"[..]].concat(),
                [opened, &b".git/.git"[..]].concat(),
            ]
        })
        .map(|path| Form::Local(canonical(&path)))
        .collect()
}

/// `path` itself when absolute, else joined to each place `from` reads a relative one
/// from.
fn joined(path: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    if path.starts_with(b"/") {
        return vec![path.to_vec()];
    }
    [Some(from.root), from.outside]
        .into_iter()
        .flatten()
        .map(|base| [base.as_os_str().as_bytes(), b"/", path].concat())
        .collect()
}

#[cfg(test)]
mod tests;
