//! `.gitdupe`'s line in the private repository's own `info/attributes` (F2, G1):
//! `/.gitdupe -text merge=union`, so that Git converts no line ending of the list whatever
//! `core.autocrlf` says, and merges two branches' changes to it with its own `union`
//! driver (N5).
//!
//! What the code below cannot show:
//!
//! - Whether the line is owed is decided on its own, from the file alone, every time `init`
//!   or `clone` attaches, never from `core.worktree`: a workspace attached by a release that
//!   wrote no line, and one whose `init` was killed between Git's `init` and this write, get
//!   it from the next `init`, and one that holds it gets nothing written.
//! - The file holds the line when one of its lines is the line byte for byte, apart from a
//!   UTF-8 byte order mark before the first line and one carriage return at a line's end,
//!   which Git drops when it reads the file. A line the developer or a template put after
//!   it, which Git would let outrank it, is theirs and stays so (F2).
//! - The line goes after every line the file holds, a newline first where the last one has
//!   none, so that a template's lines stay byte for byte and come before it, and Git, which
//!   lets a later line outrank an earlier one, reads the line last.
//! - The file is replaced whole through the keeper's `replace`, a fresh file inside the
//!   private Git directory renamed onto it with the original's permissions, so that a kill
//!   leaves it as it was or with the whole line (G21). `info/` is made when Git's `init`
//!   left none, as an empty template directory does. Where something other than a
//!   directory stands at `info`, a symbolic link above all, whose target can be shared
//!   outside the private repository, nothing is written through it (R8): the line, where it
//!   is owed, is refused naming that path.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::keeper;

/// The line F2 names.
pub const LINE: &[u8] = b"/.gitdupe -text merge=union";

/// The private repository's `info/attributes` could not be read or written: nothing of it
/// changed.
pub struct Unwritten {
    pub path: PathBuf,
    pub cause: io::Error,
}

/// Adds the line to the `info/attributes` of the private Git directory `private` where the
/// file does not hold it, and returns whether it wrote.
pub(super) fn complete(private: &Path) -> Result<bool, Unwritten> {
    let info = private.join("info");
    let file = info.join("attributes");
    let unwritten = |path: &Path| {
        let path = path.to_path_buf();
        move |cause| Unwritten { path, cause }
    };
    let (held, permissions) = match fs::read(&file) {
        Ok(held) => {
            let permissions = fs::metadata(&file).map_err(unwritten(&file))?.permissions();
            (held, Some(permissions))
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => (Vec::new(), None),
        Err(cause) => return Err(unwritten(&file)(cause)),
    };
    if holds_the_line(&held) {
        return Ok(false);
    }
    match fs::symlink_metadata(&info) {
        Ok(found) if found.is_dir() => {}
        Ok(_) => {
            let cause = io::Error::other(
                "something other than a directory stands there, and git-dupe writes nothing \
                 through it",
            );
            return Err(unwritten(&info)(cause));
        }
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&info).map_err(unwritten(&info))?;
        }
        Err(cause) => return Err(unwritten(&info)(cause)),
    }
    keeper::replace_whole(private, &file, &with_the_line(held), permissions)
        .map_err(unwritten(&file))?;
    Ok(true)
}

/// Whether one line of `content` is the line, a UTF-8 byte order mark before the first and
/// one carriage return at its end dropped.
fn holds_the_line(content: &[u8]) -> bool {
    content
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap_or(content)
        .split(|&byte| byte == b'\n')
        .any(|line| line.strip_suffix(b"\r").unwrap_or(line) == LINE)
}

/// `content` with the line after it, on a line of its own.
fn with_the_line(mut content: Vec<u8>) -> Vec<u8> {
    if !content.is_empty() && !content.ends_with(b"\n") {
        content.push(b'\n');
    }
    content.extend_from_slice(LINE);
    content.push(b'\n');
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_holds_the_line_only_as_a_line_of_its_own() {
        assert!(holds_the_line(b"/.gitdupe -text merge=union\n"));
        assert!(holds_the_line(b"/.gitdupe -text merge=union"));
        assert!(holds_the_line(
            b"*.png binary\n/.gitdupe -text merge=union\r\n"
        ));
        assert!(holds_the_line(
            b"/.gitdupe -text merge=union\n/.gitdupe text\n"
        ));
        assert!(holds_the_line(
            b"\xef\xbb\xbf/.gitdupe -text merge=union\r\n"
        ));
        assert!(!holds_the_line(
            b"*.png binary\n\xef\xbb\xbf/.gitdupe -text merge=union\n"
        ));
        for content in [
            &b""[..],
            b"\n",
            b"/.gitdupe -text\n",
            b"/.gitdupe  -text merge=union\n",
            b" /.gitdupe -text merge=union\n",
            b"/.gitdupe -text merge=union \n",
            b"/.gitdupe -text merge=union\r\r\n",
            b"# /.gitdupe -text merge=union\n",
            b".gitdupe -text merge=union\n",
        ] {
            assert!(!holds_the_line(content), "{content:?}");
        }
    }

    #[test]
    fn the_line_goes_after_every_line_held_on_a_line_of_its_own() {
        assert_eq!(with_the_line(Vec::new()), b"/.gitdupe -text merge=union\n");
        assert_eq!(
            with_the_line(b"*.png binary\n".to_vec()),
            b"*.png binary\n/.gitdupe -text merge=union\n"
        );
        assert_eq!(
            with_the_line(b"*.png binary".to_vec()),
            b"*.png binary\n/.gitdupe -text merge=union\n"
        );
        assert_eq!(
            with_the_line(b"*.png binary\r\n".to_vec()),
            b"*.png binary\r\n/.gitdupe -text merge=union\n"
        );
    }
}
