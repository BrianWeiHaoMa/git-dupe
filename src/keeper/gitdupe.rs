//! Reading `.gitdupe` as F5 reads it: one literal path per line, relative to the root.
//!
//! The file is the one at the root, read through a symbolic link; when nothing stands
//! there, its staged version, from one private `git cat-file blob :.gitdupe`, whose
//! failure of any kind means no listed path. Either is unreadable when it begins with a
//! UTF-16 byte order mark, and lists nothing. A UTF-8 byte order mark at its start is
//! dropped, and one carriage return at the end of a line, so that a file Git checks out
//! with CRLF line endings lists what it listed. A line is never a pattern or a comment:
//! one leading and one trailing `/` are dropped, an empty line is skipped, and the rest is
//! cleaned lexically relative to the root, so that `notes/./x` lists `notes/x`. A line
//! that cleans to the root or outside it hides nothing and is set aside with its 1-based
//! number. Every other byte, a carriage return elsewhere in the line and a byte order
//! mark on a later line included, is part of the path. The bytes read are kept whole with
//! their source, for `hide` and `unhide` to edit.

use std::fs::{self, Permissions};
use std::io;
use std::path::PathBuf;

use crate::bytes;
use crate::guards::operand::{self, Cleaned};
use crate::runner::locate::Workspace;
use crate::runner::{End, Run};

/// The name of the file, at the root.
pub const NAME: &[u8] = b".gitdupe";

/// The UTF-8 byte order mark, dropped at the start of the file alone (F5).
const UTF_8_MARK: &[u8] = b"\xef\xbb\xbf";

/// The UTF-16 byte order marks: Windows PowerShell 5.1's `>` writes the first (S23).
const UTF_16_MARKS: [&[u8]; 2] = [b"\xff\xfe", b"\xfe\xff"];

/// The file's path, at the root.
pub fn file(workspace: &Workspace) -> PathBuf {
    bytes::under(workspace.root(), NAME).expect("`.gitdupe` is a name")
}

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

impl Listed {
    /// Why the file is unreadable, where it is (F5).
    pub fn unreadable(&self) -> Option<&io::Error> {
        match &self.source {
            Source::Unreadable(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Where the bytes of `.gitdupe` came from.
pub enum Source {
    /// A regular file at the root, with its permissions.
    File(Permissions),
    /// A symbolic link at the root, read through, with its target's permissions.
    Link(Permissions),
    /// Nothing at the root: the staged version, or nothing when none is staged.
    Absent,
    /// Something that cannot be read as a file, or bytes that begin with a UTF-16 byte
    /// order mark, and why: it lists nothing.
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
    let file = file(workspace);
    let (bytes, source) = match fs::symlink_metadata(&file) {
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            (absent(workspace), Source::Absent)
        }
        Err(cause) => return unreadable(cause),
        Ok(found) => {
            let read =
                fs::metadata(&file).and_then(|target| Ok((fs::read(&file)?, target.permissions())));
            match read {
                Ok((bytes, permissions)) if found.file_type().is_symlink() => {
                    (bytes, Source::Link(permissions))
                }
                Ok((bytes, permissions)) => (bytes, Source::File(permissions)),
                Err(cause) => return unreadable(cause),
            }
        }
    };
    if UTF_16_MARKS.iter().any(|mark| bytes.starts_with(mark)) {
        return unreadable(io::Error::other(
            "it begins with a UTF-16 byte order mark; save it as UTF-8",
        ));
    }
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
    let (_, listed) = marked(bytes);
    for (index, line) in listed.split(|&byte| byte == b'\n').enumerate() {
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

/// The file's bytes as the byte order mark at their start, when there is one, and their
/// lines after it.
pub fn marked(bytes: &[u8]) -> (&[u8], &[u8]) {
    let mark = if bytes.starts_with(UTF_8_MARK) {
        UTF_8_MARK.len()
    } else {
        0
    };
    bytes.split_at(mark)
}

/// `lines`, bytes with no mark of their own that will stand at the file's start, with a
/// UTF-8 mark before them where their first bytes would read as one, so that the first
/// line stays the path it is: no line is changed, and the mark is the one a read drops.
pub fn unmistaken(lines: Vec<u8>) -> Vec<u8> {
    if [UTF_8_MARK, UTF_16_MARKS[0], UTF_16_MARKS[1]]
        .iter()
        .any(|mark| lines.starts_with(mark))
    {
        [UTF_8_MARK, &lines].concat()
    } else {
        lines
    }
}

/// What one line, without its newline and the file's mark, names: nothing when it is empty
/// without one carriage return at its end, else what is left without one leading and one
/// trailing `/`, cleaned.
pub fn path_of(line: &[u8]) -> Option<Cleaned> {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
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
            listed(b"#x\n*.log\n!keep\n trail \ncaf\xe9\n"),
            [&b"#x"[..], b"*.log", b"!keep", b" trail ", b"caf\xe9"]
        );
    }

    #[test]
    fn one_carriage_return_is_dropped_at_a_lines_end_and_none_elsewhere() {
        assert_eq!(
            listed(b"notes\r\ndocs/plan.md\r\n"),
            [&b"notes"[..], b"docs/plan.md"]
        );
        assert_eq!(
            listed(b"a\r\r\nb\rc\n\rd\n"),
            [&b"a\r"[..], b"b\rc", b"\rd"]
        );
        // A last line has no newline to lose and still loses its carriage return.
        assert_eq!(listed(b"x\r"), [b"x"]);
        // An empty line of a file with CRLF line endings is an empty line.
        assert_eq!(listed(b"\r\n\r\nx\r\n\r\n"), [b"x"]);
        // The carriage return goes first: the slash before it is a trailing one.
        assert_eq!(listed(b"/notes/\r\n"), [b"notes"]);
    }

    #[test]
    fn a_utf_8_mark_is_dropped_at_the_files_start_and_is_part_of_a_path_elsewhere() {
        assert_eq!(
            listed(b"\xef\xbb\xbfnotes\r\ndocs/plan.md\r\n"),
            [&b"notes"[..], b"docs/plan.md"]
        );
        assert_eq!(listed(b"a\n\xef\xbb\xbfd\n"), [&b"a"[..], b"\xef\xbb\xbfd"]);
        // One mark: the bytes after it are the first line.
        assert_eq!(
            listed(b"\xef\xbb\xbf\xef\xbb\xbfd\n"),
            [&b"\xef\xbb\xbfd"[..]]
        );
        assert_eq!(listed(b"\xef\xbb\xbf"), Vec::<Vec<u8>>::new());
        // The mark leaves the first line's number as it was.
        let (_, set_aside) = lines(b"\xef\xbb\xbf../x\n");
        assert_eq!(set_aside[0].line, 1);
    }

    /// A workspace whose root holds nothing but what a check writes, removed after.
    struct Root(PathBuf);

    impl Root {
        fn new(name: &str) -> Root {
            let path =
                std::env::temp_dir().join(format!("git-dupe-list-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Root(path)
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_that_begins_with_a_utf_16_mark_is_unreadable_and_lists_nothing() {
        let root = Root::new("utf-16");
        let workspace = Workspace::at_root(&root.0);
        for bytes in [
            &b"\xff\xfes\0c\0r\0a\0t\0c\0h\0\r\0\n\0"[..],
            b"\xfe\xff\0s\0c\0r\0a\0t\0c\0h\0\r\0\n",
            b"\xff\xfe",
        ] {
            fs::write(root.0.join(".gitdupe"), bytes).unwrap();
            let listed = on_disk(&workspace);
            assert!(matches!(listed.source, Source::Unreadable(_)));
            assert!(listed.paths.is_empty() && listed.set_aside.is_empty());
        }
        // The staged version is read the same way.
        fs::remove_file(root.0.join(".gitdupe")).unwrap();
        let listed = reading(&workspace, |_| b"\xff\xfex\0\n\0".to_vec());
        assert!(matches!(listed.source, Source::Unreadable(_)));
        assert!(listed.paths.is_empty());
        // Such bytes later in the file are bytes of a path.
        fs::write(root.0.join(".gitdupe"), b"a\n\xff\xfe\n").unwrap();
        assert_eq!(on_disk(&workspace).paths, [&b"a"[..], b"\xff\xfe"]);
    }

    /// A path whose bytes begin as a mark does stands in the file after a mark of its own.
    #[test]
    fn lines_that_would_read_as_a_mark_are_given_one_and_read_back_whole() {
        for path in [&b"\xef\xbb\xbfd"[..], b"\xff\xfed", b"\xfe\xffd"] {
            let file = unmistaken([path, b"\nsafe\n"].concat());
            assert_eq!(listed(&file), [path, b"safe"]);
            assert!(!UTF_16_MARKS.iter().any(|mark| file.starts_with(mark)));
        }
        assert_eq!(unmistaken(b"notes\n".to_vec()), b"notes\n");
        assert_eq!(unmistaken(Vec::new()), b"");
    }

    #[test]
    fn a_mark_and_carriage_returns_stay_among_the_bytes_read_for_an_edit() {
        let root = Root::new("kept");
        let workspace = Workspace::at_root(&root.0);
        let bytes = b"\xef\xbb\xbfnotes\r\n";
        fs::write(root.0.join(".gitdupe"), bytes).unwrap();
        let listed = on_disk(&workspace);
        assert_eq!(listed.paths, [b"notes"]);
        assert_eq!(listed.bytes, bytes);
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
