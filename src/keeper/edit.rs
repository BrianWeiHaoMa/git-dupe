//! `.gitdupe`'s edits: hide appends a line per path, unhide removes the lines naming a
//! path, and either stages the file in the private repository.
//!
//! An edit starts from the bytes the hidden paths were read from — the file on disk, else
//! its staged version, else nothing — keeps every line it does not add or remove byte for
//! byte, and replaces the file whole only when a line changes, from a fresh file inside
//! the private Git directory with the existing file's permissions. A symbolic link at
//! `.gitdupe` is replaced by a regular file, its target untouched. Something standing at
//! `.gitdupe` that cannot be read as a file is not written over, because its lines are
//! not known. The file is staged whenever it stands on disk as a regular file, whether or
//! not a line changed, so that a rerun stages what a killed run wrote.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;

use super::gitdupe::{self, Listed, NAME, Source};
use super::hidden::HiddenPaths;
use super::{Failed, listing, replace};
use crate::guards::operand::{self, Cleaned};
use crate::guards::pathspec;
use crate::runner::locate::Workspace;
use crate::runner::{End, Failure, Run};

/// The hidden paths as a handler reads them before an edit: `.gitdupe` with the bytes it
/// was read from, and the hidden paths of it and the private index.
pub struct Hidden {
    pub gitdupe: Listed,
    pub paths: HiddenPaths,
}

impl Hidden {
    fn of(gitdupe: Listed, privately_tracked: BTreeSet<Vec<u8>>) -> Hidden {
        let paths = HiddenPaths::of(gitdupe.paths.clone(), privately_tracked);
        Hidden { gitdupe, paths }
    }
}

/// `.gitdupe` and one private listing. A listing that fails fails the handler, which has
/// written nothing.
pub fn hidden(workspace: &Workspace) -> Result<Hidden, Failed> {
    let gitdupe = gitdupe::read(workspace);
    let privately_tracked = listing::private(workspace)?;
    Ok(Hidden::of(gitdupe, privately_tracked))
}

/// What an edit did.
pub struct Edited {
    /// The paths added or removed, in the order given, each once.
    pub changed: Vec<Vec<u8>>,
    /// A symbolic link at `.gitdupe` became a regular file.
    pub link_replaced: bool,
    /// The staging run after the write, when it failed: the file is written.
    pub not_staged: Option<Failed>,
}

/// Why an edit wrote nothing.
pub enum NotEdited {
    /// Something stands at `.gitdupe` that cannot be read as a file.
    Unreadable(io::Error),
    /// The fresh file could not be written or renamed onto `.gitdupe`.
    NotWritten(io::Error),
}

/// The paths a hide of `paths` adds, in order: each one not listed, not below a listed
/// path, and not at or below a path added before it.
pub fn hiding(hidden: &Hidden, paths: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut listed: BTreeSet<&[u8]> = hidden.gitdupe.paths.iter().map(Vec::as_slice).collect();
    let mut added = Vec::new();
    for path in paths {
        let covered = listed.contains(path.as_slice())
            || operand::ancestors(path).any(|above| listed.contains(above));
        if !covered {
            listed.insert(path);
            added.push(path.clone());
        }
    }
    added
}

/// Appends the lines of `hiding` to `.gitdupe` and stages it.
pub fn hide(workspace: &Workspace, hidden: Hidden, paths: &[Vec<u8>]) -> Result<Edited, NotEdited> {
    let added = hiding(&hidden, paths);
    let content = (!added.is_empty()).then(|| appended(&hidden.gitdupe.bytes, &added));
    write_and_stage(workspace, hidden.gitdupe.source, content, added)
}

/// Removes each line whose cleaned path equals one of `paths` and stages `.gitdupe`. A
/// line naming a path below one of them stays.
pub fn unhide(
    workspace: &Workspace,
    hidden: Hidden,
    paths: &[Vec<u8>],
) -> Result<Edited, NotEdited> {
    let (content, removed) = removed(&hidden.gitdupe.bytes, paths);
    let content = (!removed.is_empty()).then_some(content);
    write_and_stage(workspace, hidden.gitdupe.source, content, removed)
}

fn write_and_stage(
    workspace: &Workspace,
    source: Source,
    content: Option<Vec<u8>>,
    changed: Vec<Vec<u8>>,
) -> Result<Edited, NotEdited> {
    let file = workspace.root().join(OsStr::from_bytes(NAME));
    let mode = match source {
        Source::Unreadable(cause) => return Err(NotEdited::Unreadable(cause)),
        Source::File(mode) | Source::Link(mode) => Some(mode),
        Source::Absent => None,
    };
    let link_replaced = content.is_some() && matches!(source, Source::Link(_));
    if let Some(content) = content {
        let private = workspace.private_directory();
        replace::whole(&private, &file, &content, mode).map_err(NotEdited::NotWritten)?;
    }
    let on_disk = fs::symlink_metadata(&file).is_ok_and(|found| found.file_type().is_file());
    let not_staged = if on_disk {
        stage(workspace).err()
    } else {
        None
    };
    Ok(Edited {
        changed,
        link_replaced,
        not_staged,
    })
}

/// One private `git add -f` of `.gitdupe`, which public Git's rules may ignore, with
/// Git's own output and message.
fn stage(workspace: &Workspace) -> Result<(), Failed> {
    let private = workspace.private_directory();
    let words = [
        "add".into(),
        "-f".into(),
        "--".into(),
        pathspec::top_literal(NAME),
    ];
    let run = Run::private(&private, workspace.root(), words)
        .from(workspace.root())
        .own_paths()
        .start();
    match run {
        Ok(finished) if finished.end == End::Code(0) => Ok(()),
        Ok(finished) => Err(Failed::Exited(finished.end)),
        Err(Failure::TooLong) => Err(Failed::TooLong(1)),
        Err(failure) => Err(Failed::NotStarted(failure)),
    }
}

/// `bytes` with one line per path appended, a last line lacking its newline given one
/// first.
fn appended(bytes: &[u8], paths: &[Vec<u8>]) -> Vec<u8> {
    let mut appended = bytes.to_vec();
    if !appended.is_empty() && !appended.ends_with(b"\n") {
        appended.push(b'\n');
    }
    for path in paths {
        appended.extend_from_slice(path);
        appended.push(b'\n');
    }
    appended
}

/// `bytes` without each line whose cleaned path equals one of `paths`, every other line
/// byte for byte; and the paths that named a line, in the order given, each once.
fn removed(bytes: &[u8], paths: &[Vec<u8>]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut kept = Vec::with_capacity(bytes.len());
    let mut named = BTreeSet::new();
    for piece in bytes.split_inclusive(|&byte| byte == b'\n') {
        let line = piece.strip_suffix(b"\n").unwrap_or(piece);
        match gitdupe::path_of(line) {
            Some(Cleaned::Inside(path)) if paths.contains(&path) => {
                named.insert(path);
            }
            _ => kept.extend_from_slice(piece),
        }
    }
    let mut removed = Vec::new();
    for path in paths {
        if named.remove(path) {
            removed.push(path.clone());
        }
    }
    (kept, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn hidden(bytes: &[u8], tracked: &[&[u8]]) -> Hidden {
        let listed = bytes
            .split(|&byte| byte == b'\n')
            .filter_map(|line| match gitdupe::path_of(line) {
                Some(Cleaned::Inside(path)) => Some(path),
                _ => None,
            })
            .collect();
        let gitdupe = Listed {
            paths: listed,
            set_aside: Vec::new(),
            bytes: bytes.to_vec(),
            source: Source::Absent,
        };
        Hidden::of(gitdupe, tracked.iter().map(|path| path.to_vec()).collect())
    }

    #[test]
    fn hiding_adds_a_path_neither_listed_nor_below_a_listed_or_earlier_one() {
        let listed = hidden(b"notes/\n\n/docs/x/\n", &[b"conf/local.ini"]);
        assert_eq!(
            hiding(
                &listed,
                &paths(&[b"notes", b"notes/sub", b"docs/x/y", b"docs"])
            ),
            paths(&[b"docs"])
        );
        assert_eq!(
            hiding(&listed, &paths(&[b"a", b"b", b"a", b"a/c", b"notes-old"])),
            paths(&[b"a", b"b", b"notes-old"])
        );
        // A privately tracked file is hidden but not listed: it gets its line.
        assert_eq!(
            hiding(&listed, &paths(&[b"conf/local.ini"])),
            paths(&[b"conf/local.ini"])
        );
        // A path above a listed one is not below it.
        assert_eq!(hiding(&listed, &paths(&[b"doc"])), paths(&[b"doc"]));
        // Listed means listed as cleaned: `notes/./sub` is below `notes`.
        let listed = hidden(b"./a//b/\n", &[]);
        assert_eq!(hiding(&listed, &paths(&[b"a/b/c", b"a"])), paths(&[b"a"]));
        assert_eq!(hiding(&hidden(b"", &[]), &[]), Vec::<Vec<u8>>::new());
    }

    #[test]
    fn appending_keeps_every_byte_and_ends_a_last_line_first() {
        assert_eq!(
            appended(b"notes/\n\n/docs/x/\n", &paths(&[b"scratch"])),
            b"notes/\n\n/docs/x/\nscratch\n"
        );
        assert_eq!(
            appended(b"notes", &paths(&[b"scratch"])),
            b"notes\nscratch\n"
        );
        assert_eq!(appended(b"", &paths(&[b"a", b"b"])), b"a\nb\n");
        assert_eq!(appended(b"\n", &paths(&[b"a"])), b"\na\n");
        assert_eq!(
            appended(b"x\r\n", &paths(&[b"caf\xe9", b"a b\r"])),
            b"x\r\ncaf\xe9\na b\r\n"
        );
    }

    #[test]
    fn removing_takes_every_line_whose_cleaned_path_is_given_and_no_other() {
        assert_eq!(
            removed(b"notes/\n\n/docs/x/\nscratch\n", &paths(&[b"docs/x"])),
            (b"notes/\n\nscratch\n".to_vec(), paths(&[b"docs/x"]))
        );
        // Equality only: a line below the path stays; every line naming it goes.
        assert_eq!(
            removed(
                b"notes\nnotes/a.md\n/notes/\nnotes/./\n",
                &paths(&[b"notes"])
            ),
            (b"notes/a.md\n".to_vec(), paths(&[b"notes"]))
        );
        // Removed paths come in the order given, each once; one not listed is no change.
        assert_eq!(
            removed(b"a\nb\n", &paths(&[b"typo", b"b", b"a", b"b"])),
            (Vec::new(), paths(&[b"b", b"a"]))
        );
        assert_eq!(
            removed(b"a\nb\n", &paths(&[b"typo"])),
            (b"a\nb\n".to_vec(), Vec::new())
        );
        // A last line without its newline, removed or kept, as it was.
        assert_eq!(
            removed(b"a\nb", &paths(&[b"b"])),
            (b"a\n".to_vec(), paths(&[b"b"]))
        );
        assert_eq!(
            removed(b"a\nb", &paths(&[b"a"])),
            (b"b".to_vec(), paths(&[b"a"]))
        );
        // A line that names the root or outside it names no path.
        assert_eq!(
            removed(b"/\n../x\n.\n", &paths(&[b"x"])),
            (b"/\n../x\n.\n".to_vec(), Vec::new())
        );
    }
}
