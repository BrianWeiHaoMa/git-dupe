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
//! - A one-letter host is a drive to Git for Windows (S13), and the forms below are the same
//!   on every platform: the word is also a local path, whole, and where no separator follows
//!   its colon what follows the colon is also a relative path, the over-inclusive
//!   direction. So is `<letter>:/<path>` of a `file:///<letter>:/<path>` or
//!   `file://<letter>:/<path>` word, and, where the platform has drives, what follows the
//!   colon of such a word with no separator there. These are Git's own readings, not a match
//!   (N5).
//! - A URL's authority is what stands between its `://` and the first `/` after it,
//!   bounded before anything is decoded, as RFC 3986 bounds it and the HTTP transport
//!   reads it; each `%XX` but `%00`, which Git keeps as written, is then decoded in the
//!   authority and in the path alike, so that an encoded `/` in a credential stays in the
//!   credential. Git's own `file://` and `ssh://` reading decodes the whole URL first and
//!   splits it after (S13): where an encoded `/` in a `file://` URL's authority makes the
//!   path Git so opens another path than the URL's own, the URL has the local forms of
//!   both. A word without `://` is never decoded.
//! - A local path has the canonical path of each path Git opens for it: its trailing
//!   slashes dropped, the path itself, with `.git` appended, and each of those with
//!   `/.git` appended. A relative one is read from the root and also from the user's
//!   directory where that lies outside the root (`ReadFrom`). An empty one is the root.
//!   So is a drive's top on Windows, `C:/`, on the root's drive, beside itself: Git drops
//!   its slash too, and `C:` is the directory Git stands in on that drive.
//! - A local path that is `~` or begins with `~/` is read as Git reads it: its trailing
//!   slashes dropped, then the value of `HOME` in place of its `~`, then `.git` and
//!   `/.git` appended to what that gives; where `HOME` is unset Git opens nothing for it,
//!   and it has no form. One beginning with `~<user>` is read as written, a relative path:
//!   Git takes that user's home from the password database, which the standard library
//!   cannot ask, and the product admits it as not seen through.
//! - A relative local path is one `Path::is_absolute` does not call absolute: `C:\proj` is
//!   absolute on Windows alone, `/x` on Linux and macOS alone (`Holds/G18`). On Windows
//!   `\x` and `/x` are read from the root as the platform joins them, on its drive, and
//!   what Git for Windows's shell opens for a `/x` it rewrites is also a local path:
//!   `read_by_the_shell`, the over-inclusive direction. A `/<path>`
//!   it reads below Git's own top directory has no form of it: the product admits it.
//! - The canonical path is the real path, asked of the path as written, so that on Linux
//!   and macOS `link/..` is where the link's target's parent is; where the real path cannot
//!   be had — nothing there, or a path Git cannot open either, as one beyond a component it
//!   may not search or in a loop of links — it is the path cleaned as text (`operand`). On
//!   Windows a real path is spelled `\\?\C:\…`, so a canonical path is compared only with
//!   another. This and `places` are the only parts of `Guards` that read the filesystem,
//!   and they only read.

use std::path::{self, Path};
use std::{env, fs};

use super::operand;
use crate::bytes;

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
        let (authority, path) = at_first_slash(after);
        let path = decoded(path);
        let mut forms = vec![url(host(&decoded(authority)), &path)];
        if scheme == b"file" {
            // Git opens the path from the first `/` of the URL decoded whole (S13), which
            // an encoded `/` in the authority moves off the URL's own path: both are local
            // forms then, the over-inclusive reading.
            let whole = decoded(after);
            let (_, opened) = at_first_slash(&whole);
            let mut paths = vec![&path[..]];
            if opened != path {
                paths.push(opened);
            }
            for path in paths.into_iter().filter(|path| !path.is_empty()) {
                forms.extend(local(path, from));
            }
            if let Some(path) = on_a_drive(&whole) {
                let relative = after_the_colon(&path[2..], from);
                forms.extend(if relative.is_empty() {
                    local(path, from)
                } else {
                    relative
                });
            }
        }
        return forms;
    }
    if let Some((host, path)) = scp(destination) {
        let mut forms = vec![url(
            host,
            &if path.starts_with(b"/") {
                path.to_vec()
            } else {
                [&b"/"[..], path].concat()
            },
        )];
        if matches!(host, [letter] if letter.is_ascii_alphabetic()) {
            forms.extend(local(destination, from));
            forms.extend(after_the_colon(path, from));
        }
        return forms;
    }
    local(destination, from)
}

/// The canonical path of `path`, absolute: its real path when it can be had, and the
/// path cleaned as text otherwise.
pub fn canonical(path: &[u8]) -> Vec<u8> {
    let real = bytes::name(path).and_then(|name| fs::canonicalize(Path::new(name)).ok());
    match real {
        Some(real) => bytes::of(real.as_os_str()).to_vec(),
        None => operand::clean_absolute(path),
    }
}

/// The local forms of `path`, what follows a drive's colon, where no separator begins it, as
/// the platform calls one: a relative path, from where Git stands on that drive; none otherwise.
fn after_the_colon(path: &[u8], from: ReadFrom) -> Vec<Form> {
    if path
        .first()
        .is_some_and(|&byte| path::is_separator(char::from(byte)))
    {
        return Vec::new();
    }
    // Written from `.`, so that no reading of a local path takes it for another kind: what
    // follows the colon is a relative path whatever it holds.
    local(&[b"./", path].concat(), from)
}

/// The `<letter>:<path>` of what a `file:///<letter>:<path>` or `file://<letter>:<path>` word
/// holds after its `://`, decoded: a separator as the platform calls one after the colon, or
/// anything where the platform has drives, which is where Git for Windows opens that.
fn on_a_drive(after_the_scheme: &[u8]) -> Option<&[u8]> {
    let path = after_the_scheme
        .strip_prefix(b"/")
        .unwrap_or(after_the_scheme);
    let [letter, b':', rest @ ..] = path else {
        return None;
    };
    let separated = rest
        .first()
        .is_some_and(|&byte| path::is_separator(char::from(byte)));
    let read = separated || operand::absolute(&[*letter, b':', b'/']);
    (letter.is_ascii_alphabetic() && read).then_some(path)
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

/// `text` split before its first `/`: a URL's authority, and its path from that `/` on,
/// empty when there is none.
fn at_first_slash(text: &[u8]) -> (&[u8], &[u8]) {
    match text.iter().position(|&byte| byte == b'/') {
        Some(slash) => text.split_at(slash),
        None => (text, b""),
    }
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

/// `%XX` decoded to its byte wherever `XX` are two hexadecimal digits naming a byte other
/// than zero; every other byte kept. Git keeps `%00` as written, so the path it opens
/// for a `file://` URL holds those three bytes (S13).
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
            && high * 16 + low != 0
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
/// read in place of its `~`, and none where `HOME` is unset; `~\x` is no such form on
/// Windows (S13).
fn local(path: &[u8], from: ReadFrom) -> Vec<Form> {
    let opened: Vec<Vec<u8>> = match path.strip_prefix(b"~") {
        Some(rest) if rest.is_empty() || rest.starts_with(b"/") => match from.home {
            Some(home) => joined(&[home, without_trailing_slashes(rest)].concat(), from),
            None => Vec::new(),
        },
        _ => joined(path, from)
            .iter()
            .flat_map(|joined| {
                let written = operand::with_slashes(joined);
                let kept = without_trailing_slashes(&written);
                if operand::absolute(kept) == operand::absolute(&written) {
                    return vec![kept.to_vec()];
                }
                // The top, `/` or `C:/`, keeps its slash: without it, it is another path. Git
                // drops the slash of `C:/` and opens `C:`, where it stands on that drive.
                let mut paths = vec![[kept, b"/"].concat()];
                paths.extend(stood_in_on(kept, from));
                paths
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

/// Each place `from` reads a relative path from that stands on `drive`, which is `C:` spelled
/// in either case, or nothing: Git stands there on that drive.
fn stood_in_on(drive: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    [Some(from.root), from.outside]
        .into_iter()
        .flatten()
        .map(|base| bytes::of(base.as_os_str()))
        .filter(|base| {
            !drive.is_empty()
                && base
                    .get(..2)
                    .is_some_and(|on| on.eq_ignore_ascii_case(drive))
        })
        .map(<[u8]>::to_vec)
        .collect()
}

/// `path` itself when absolute, else joined to each place `from` reads a relative one from,
/// and what `read_by_the_shell` reads for it.
fn joined(path: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    if operand::absolute(path) {
        return vec![path.to_vec()];
    }
    let mut paths = join_to_each_place(path, from);
    paths.extend(read_by_the_shell(path, from));
    paths
}

/// What Git for Windows's shell opens for `path`, which begins with a separator.
fn read_by_the_shell(path: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    let mut paths = Vec::from_iter(drive_as_directory(path));
    paths.extend(through_the_shell(path, from));
    paths
}

/// `path`, not absolute, joined to each place `from` reads a relative one from: a root and no
/// drive, which only Windows has, on that place's drive, where Git opens it; and any other
/// path after that place, a drive and no root staying a name in it.
fn join_to_each_place(path: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    let rooted = bytes::name(path).filter(|name| Path::new(name).has_root());
    [Some(from.root), from.outside]
        .into_iter()
        .flatten()
        .map(|base| match rooted {
            Some(name) => bytes::of(base.join(name).as_os_str()).to_vec(),
            None => [bytes::of(base.as_os_str()), b"/", path].concat(),
        })
        .collect()
}

/// The first component of a path that begins with `/`, separators and `.` components before
/// it being none to the shell, and what follows it, which begins with a separator as the
/// platform calls one, or is empty.
fn first_component(path: &[u8]) -> Option<(&[u8], &[u8])> {
    let separator = |byte: &u8| path::is_separator(char::from(*byte));
    let mut path = path.strip_prefix(b"/")?;
    loop {
        match path {
            [first, rest @ ..] if separator(first) => path = rest,
            [b'.', second, rest @ ..] if separator(second) => path = rest,
            _ => break,
        }
    }
    Some(path.split_at(path.iter().position(separator).unwrap_or(path.len())))
}

/// The `<letter>:/<path>` of a `/<letter>/<path>` that `Path::is_absolute` does not call
/// absolute, which only Windows has: Git for Windows's shell opens that path on that drive
/// when Git starts a transport program there.
fn drive_as_directory(path: &[u8]) -> Option<Vec<u8>> {
    match first_component(path)? {
        ([letter], rest) if letter.is_ascii_alphabetic() && !rest.is_empty() => {
            Some([&[*letter, b':'][..], rest].concat())
        }
        _ => None,
    }
}

/// What else that shell opens for such a path: `/<letter>` the drive's top, `/<letter>:/<path>`
/// that path on the drive, `/<letter>:<path>` with no separator `<path>` from where Git stands
/// on the drive, `/tmp/<path>`, `tmp` in any case, that path below the directory `temp_dir`
/// gives, which is the shell's `/tmp`, and `/proc/<path>` the root: that file system reaches any
/// place and every spelling of it, so that no form of its own can hold one. The refused
/// direction: it costs `git dupe git`. A public URL so spelled is compared by where it opens
/// too, `below_proc`: the root alone would keep a word from the project and let one reach the
/// remote.
fn through_the_shell(path: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    let Some((first, rest)) = first_component(path) else {
        return Vec::new();
    };
    match first {
        [letter] if letter.is_ascii_alphabetic() && rest.is_empty() => {
            vec![[*letter, b':', b'/'].to_vec()]
        }
        [letter, b':'] if letter.is_ascii_alphabetic() => {
            vec![[&[*letter, b':', b'/'][..], rest].concat()]
        }
        [letter, b':', after @ ..] if letter.is_ascii_alphabetic() => {
            join_to_each_place(&[b"./", after, rest].concat(), from)
        }
        b"proc" => {
            let mut paths = join_to_each_place(b".", from);
            paths.extend(below_proc(rest, from));
            paths
        }
        _ if first.eq_ignore_ascii_case(b"tmp") => {
            vec![[bytes::of(env::temp_dir().as_os_str()), b"/", rest].concat()]
        }
        _ => Vec::new(),
    }
}

/// The places `/proc/<rest>` opens by name, each component spelled in lower case: `cygdrive/<path>`
/// and the `root/<path>` of `self` or of a running process's number are `/<path>`, and
/// `self/cwd/<path>` is `<path>` from where Git stands. Another process's `cwd` is where that
/// process stands, which nothing here can ask.
fn below_proc(rest: &[u8], from: ReadFrom) -> Vec<Vec<u8>> {
    let rest = operand::with_slashes(rest);
    let Some((first, after)) = first_component(&rest) else {
        return Vec::new();
    };
    let a_process = first == b"self" || first.iter().all(u8::is_ascii_digit);
    match (first, first_component(after)) {
        (b"cygdrive", _) => read_by_the_shell(after, from),
        (_, Some((b"root", path))) if a_process => read_by_the_shell(path, from),
        (b"self", Some((b"cwd", path))) => join_to_each_place(&[b"./", path].concat(), from),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
