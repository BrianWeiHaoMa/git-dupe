//! Reading `.gitdupe` as F5 reads it: one literal path per line, relative to the root.
//!
//! The file is the one at the root, read through a symbolic link; when nothing stands
//! there, its staged version, from one private `git cat-file blob :.gitdupe`, whose
//! failure of any kind means no listed path. A line is never a pattern or a comment: one
//! leading and one trailing `/` are dropped, a line empty as written is skipped, and the
//! rest is cleaned lexically relative to the root, so that `notes/./x` lists `notes/x`.
//! A line that cleans to the root or outside it hides nothing and is set aside with its
//! 1-based number. Every other byte, a carriage return included, is part of the path.
//! The bytes read are kept with their source, for `hide` and `unhide` to edit.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;

use crate::guards::operand::{self, Cleaned};
use crate::runner::locate::Workspace;
use crate::runner::{End, Run};

/// The name of the file, at the root.
pub const NAME: &[u8] = b".gitdupe";

/// What `.gitdupe` lists, and the bytes it was read from.
pub struct Listed {
    /// The cleaned paths, in line order.
    pub paths: Vec<Vec<u8>>,
    /// The lines that hide nothing.
    pub set_aside: Vec<SetAside>,
    /// The bytes read: the file's, the staged version's, or none.
    pub bytes: Vec<u8>,
    pub source: Source,
}

/// Where the bytes of `.gitdupe` came from.
pub enum Source {
    /// A regular file at the root, with its permissions.
    File(u32),
    /// A symbolic link at the root, read through, with its target's permissions.
    Link(u32),
    /// Nothing at the root: the staged version, or nothing when none is staged.
    Absent,
    /// Something at the root that cannot be read as a file, and why: it lists nothing.
    Unreadable(io::Error),
}

/// A line that names the root or a path outside it.
#[derive(Debug, PartialEq, Eq)]
pub struct SetAside {
    pub line: usize,
    pub cleaned: Cleaned,
}

pub fn read(workspace: &Workspace) -> Listed {
    reading(workspace, staged)
}

/// What the file on disk lists, the staged version never asked for: what `detach --force`
/// reads where the private repository cannot be listed (`Holds/G3`).
pub fn on_disk(workspace: &Workspace) -> Listed {
    reading(workspace, |_| Vec::new())
}

/// `.gitdupe` read from the file at the root, through a link, and from `absent` where
/// nothing stands there.
fn reading(workspace: &Workspace, absent: impl FnOnce(&Workspace) -> Vec<u8>) -> Listed {
    let file = workspace.root().join(OsStr::from_bytes(NAME));
    let (bytes, source) = match fs::symlink_metadata(&file) {
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            (absent(workspace), Source::Absent)
        }
        Err(cause) => return unreadable(cause),
        Ok(found) => {
            let read = fs::metadata(&file)
                .and_then(|target| Ok((fs::read(&file)?, target.permissions().mode())));
            match read {
                Ok((bytes, mode)) if found.file_type().is_symlink() => (bytes, Source::Link(mode)),
                Ok((bytes, mode)) => (bytes, Source::File(mode)),
                Err(cause) => return unreadable(cause),
            }
        }
    };
    let (paths, set_aside) = lines(&bytes);
    Listed {
        paths,
        set_aside,
        bytes,
        source,
    }
}

fn unreadable(cause: io::Error) -> Listed {
    Listed {
        paths: Vec::new(),
        set_aside: Vec::new(),
        bytes: Vec::new(),
        source: Source::Unreadable(cause),
    }
}

/// The staged version, stage 0 only: a `.gitdupe` in conflict lists nothing.
fn staged(workspace: &Workspace) -> Vec<u8> {
    let private = workspace.private_directory();
    let run = Run::private(
        &private,
        workspace.root(),
        ["cat-file", "blob", ":.gitdupe"],
    )
    .from(workspace.root())
    .capture_output()
    .capture_errors()
    .start();
    match run {
        Ok(run) if run.end == End::Code(0) => run.stdout,
        _ => Vec::new(),
    }
}

fn lines(bytes: &[u8]) -> (Vec<Vec<u8>>, Vec<SetAside>) {
    let mut paths = Vec::new();
    let mut set_aside = Vec::new();
    for (index, line) in bytes.split(|&byte| byte == b'\n').enumerate() {
        match path_of(line) {
            None => {}
            Some(Cleaned::Inside(path)) => paths.push(path),
            Some(cleaned) => set_aside.push(SetAside {
                line: index + 1,
                cleaned,
            }),
        }
    }
    (paths, set_aside)
}

/// What one line, without its newline, names: nothing when it is empty as written, else
/// the line without one leading and one trailing `/`, cleaned.
pub fn path_of(line: &[u8]) -> Option<Cleaned> {
    if line.is_empty() {
        return None;
    }
    let line = line.strip_prefix(b"/").unwrap_or(line);
    let line = line.strip_suffix(b"/").unwrap_or(line);
    Some(operand::clean(line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(bytes: &[u8]) -> Vec<Vec<u8>> {
        lines(bytes).0
    }

    #[test]
    fn a_leading_and_a_trailing_slash_are_dropped_and_an_empty_line_skipped() {
        assert_eq!(
            listed(b"notes/\n/docs/plan.md\n\na/./b//\nnotes/sub\n"),
            [&b"notes"[..], b"docs/plan.md", b"a/b", b"notes/sub"]
        );
        assert_eq!(listed(b"x"), [b"x"]);
        assert_eq!(listed(b""), Vec::<Vec<u8>>::new());
        assert_eq!(listed(b"\n\n"), Vec::<Vec<u8>>::new());
    }

    #[test]
    fn every_line_is_a_literal_path_never_a_comment_or_a_pattern() {
        assert_eq!(
            listed(b"#x\n*.log\n!keep\n trail \ncr\r\ncaf\xe9\n"),
            [
                &b"#x"[..],
                b"*.log",
                b"!keep",
                b" trail ",
                b"cr\r",
                b"caf\xe9"
            ]
        );
    }

    #[test]
    fn a_line_naming_the_root_or_outside_it_is_set_aside_with_its_number() {
        let (paths, set_aside) = lines(b"notes/\n../x\n.\nkept\n/\na/../../y\n");
        assert_eq!(paths, [&b"notes"[..], b"kept"]);
        assert_eq!(
            set_aside,
            [
                SetAside {
                    line: 2,
                    cleaned: Cleaned::Outside(b"../x".to_vec())
                },
                SetAside {
                    line: 3,
                    cleaned: Cleaned::Root
                },
                SetAside {
                    line: 5,
                    cleaned: Cleaned::Root
                },
                SetAside {
                    line: 6,
                    cleaned: Cleaned::Outside(b"../y".to_vec())
                },
            ]
        );
    }
}
