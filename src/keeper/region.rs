//! The managed region of `.git/info/exclude` (F3): where it lies in the file, what it
//! held when the command began, its replacement, and its deletion for `detach`.
//!
//! The region is the line `# BEGIN git-dupe`, one rule per line, and the line
//! `# END git-dupe`. The first begin marker and the first end marker after it bound it; a
//! begin marker without an end marker runs to the end of the file. Everything else is
//! the user's and is copied byte for byte. A region first created is appended at the end
//! of the file, on a line of its own.
//!
//! Settle's replacement and `detach`'s deletion are one write: `find` takes the `lstat`
//! of `.git/info` and of the file and reads it through a link, and `write` renames a
//! fresh file from the private Git directory over it with the original's permissions.
//! They differ in what they compose and where nothing stands: the deletion keeps the
//! user's bytes around the region and nothing else, and where no begin marker stands, or
//! no file, or `.git/info` is missing, it writes and creates nothing. An exclude file
//! that cannot be read holds no region for `start`; the deletion refuses it instead,
//! because `detach` succeeds only once no region stands.

use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::{replace, rule};
use crate::runner::locate::Workspace;

const BEGIN: &[u8] = b"# BEGIN git-dupe";
const END: &[u8] = b"# END git-dupe";

/// An exclude file's bytes, around and inside its region.
#[derive(Debug, PartialEq, Eq)]
struct Split<'b> {
    /// The user's bytes before the region: the whole file when there is no region.
    before: &'b [u8],
    /// The lines between the markers, without their newlines; `None` without a region.
    rules: Option<Vec<&'b [u8]>>,
    /// The user's bytes after the end marker's line.
    after: &'b [u8],
}

fn split(bytes: &[u8]) -> Split<'_> {
    let mut lines = lines(bytes);
    let Some((begin, _)) = lines.find(|(_, line)| *line == BEGIN) else {
        return Split {
            before: bytes,
            rules: None,
            after: b"",
        };
    };
    let mut rules = Vec::new();
    let mut after: &[u8] = b"";
    for (start, line) in lines {
        if line == END {
            let end = start + line.len();
            after = bytes.get(end + 1..).unwrap_or_default();
            break;
        }
        rules.push(line);
    }
    Split {
        before: &bytes[..begin],
        rules: Some(rules),
        after,
    }
}

/// Each line with the offset it starts at, without its newline; the empty piece after a
/// final newline is no line.
fn lines(bytes: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
    let mut start = 0;
    bytes
        .split(|&byte| byte == b'\n')
        .map(move |line| {
            let at = start;
            start += line.len() + 1;
            (at, line)
        })
        .filter(move |(at, _)| *at < bytes.len())
}

/// The file with its region holding exactly one rule per path, in the order given.
fn compose(split: &Split, paths: &[Vec<u8>]) -> Vec<u8> {
    let mut composed = split.before.to_vec();
    if !composed.is_empty() && !composed.ends_with(b"\n") {
        composed.push(b'\n');
    }
    composed.extend_from_slice(BEGIN);
    composed.push(b'\n');
    for path in paths {
        composed.extend_from_slice(&rule::of(path));
        composed.push(b'\n');
    }
    composed.extend_from_slice(END);
    composed.push(b'\n');
    composed.extend_from_slice(split.after);
    composed
}

/// The region as a command began: the paths its rules were written for. A line that is
/// no rule of the region's is not read, and a file that cannot be read holds none.
#[derive(Default)]
pub struct StartingRegion {
    pub paths: Vec<Vec<u8>>,
}

pub fn start(workspace: &Workspace) -> StartingRegion {
    let bytes = fs::read(exclude_file(workspace)).unwrap_or_default();
    let paths = paths_of(&split(&bytes).rules.unwrap_or_default());
    StartingRegion { paths }
}

/// The paths a region's rules were written for, a line that is no rule read as nothing.
fn paths_of(rules: &[&[u8]]) -> Vec<Vec<u8>> {
    rules.iter().copied().filter_map(rule::path_of).collect()
}

/// `.git/info` where it is a symbolic link by `lstat` and the file read through it holds a
/// region, as `start` reads it: the region `detach` refuses to leave behind (G3).
pub fn beyond_a_link(workspace: &Workspace) -> Option<PathBuf> {
    let info = workspace.common_directory().join("info");
    let link = fs::symlink_metadata(&info).is_ok_and(|found| found.file_type().is_symlink());
    (link && holds_region(&exclude_file(workspace))).then_some(info)
}

fn holds_region(exclude: &Path) -> bool {
    split(&fs::read(exclude).unwrap_or_default())
        .rules
        .is_some()
}

fn exclude_file(workspace: &Workspace) -> PathBuf {
    workspace.common_directory().join("info").join("exclude")
}

/// The exclude file as a write finds it.
enum Found {
    /// `.git/info` is a symbolic link by `lstat`.
    InfoLink,
    /// `.git/info` does not exist.
    NoInfo,
    /// `.git/info/exclude` is a symbolic link to nothing.
    DanglingLink,
    /// The file's bytes, read through a link at it, with the permissions of the file or of
    /// the link's target; no bytes and no permissions where no file stands.
    File {
        bytes: Vec<u8>,
        mode: Option<u32>,
        link: bool,
    },
}

/// What stands at `.git/info` and at the exclude file inside it, by `lstat`, and the file's
/// bytes. A failure to look or to read is the error.
fn find(info: &Path, exclude: &Path) -> io::Result<Found> {
    match fs::symlink_metadata(info) {
        Ok(found) if found.file_type().is_symlink() => return Ok(Found::InfoLink),
        Ok(_) => {}
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(Found::NoInfo),
        Err(cause) => return Err(cause),
    }
    let found = match fs::symlink_metadata(exclude) {
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => {
            return Ok(Found::File {
                bytes: Vec::new(),
                mode: None,
                link: false,
            });
        }
        Err(cause) => return Err(cause),
        Ok(found) => found,
    };
    let link = found.file_type().is_symlink();
    // A link is read through; its target's permissions are the original's.
    let read = fs::metadata(exclude)
        .and_then(|target| Ok((fs::read(exclude)?, target.permissions().mode())));
    match read {
        Ok((bytes, mode)) => Ok(Found::File {
            bytes,
            mode: Some(mode),
            link,
        }),
        Err(cause) if link && cause.kind() == io::ErrorKind::NotFound => Ok(Found::DanglingLink),
        Err(cause) => Err(cause),
    }
}

/// Renames a fresh file holding `composed` over the exclude file, and returns, where a
/// link stood at it, the warning that it is now a regular file `holding` what it holds,
/// its target untouched.
fn write(
    workspace: &Workspace,
    exclude: &Path,
    composed: &[u8],
    mode: Option<u32>,
    link: bool,
    holding: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    replace::whole(&workspace.private_directory(), exclude, composed, mode)?;
    Ok(link.then(|| {
        [
            exclude.as_os_str().as_bytes(),
            b" was a symbolic link; it is now a regular file holding ",
            holding,
            b", and its target is untouched",
        ]
        .concat()
    }))
}

/// Replaces the region with one rule per path, in the order given, and returns the
/// warnings: that the region cannot be maintained where a symbolic link stands, that a
/// write failed and why, or that a link at the exclude file became a regular file.
pub fn replace(workspace: &Workspace, paths: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let info = workspace.common_directory().join("info");
    let exclude = exclude_file(workspace);
    let failed = |cause: io::Error| vec![not_written(&exclude, &cause.to_string())];

    let (current, mode, link) = match find(&info, &exclude) {
        Err(cause) => return failed(cause),
        Ok(Found::InfoLink) => return vec![unmaintainable(&info, b"is a symbolic link")],
        Ok(Found::DanglingLink) => {
            return vec![unmaintainable(&exclude, b"is a symbolic link to nothing")];
        }
        Ok(Found::NoInfo) => {
            if let Err(cause) = fs::create_dir(&info) {
                return failed(cause);
            }
            (Vec::new(), None, false)
        }
        Ok(Found::File { bytes, mode, link }) => (bytes, mode, link),
    };

    let composed = compose(&split(&current), paths);
    if mode.is_some() && !link && composed == current {
        return Vec::new();
    }
    let holding = b"what its target held and the managed region";
    match write(workspace, &exclude, &composed, mode, link, holding) {
        Err(cause) => failed(cause),
        Ok(replaced) => replaced.into_iter().collect(),
    }
}

/// What the deletion of the region did: the paths its rules were written for, and the
/// warning that a link at the exclude file became a regular file.
#[derive(Default)]
pub struct Deleted {
    pub paths: Vec<Vec<u8>>,
    pub warning: Option<Vec<u8>>,
}

/// Why a region may stand after its deletion was asked for: nothing was written.
pub enum NotDeleted {
    /// `.git/info` is a symbolic link, and the file read through it holds the region.
    BeyondALink(PathBuf),
    /// The exclude file could not be looked at or read, or the fresh file could not be
    /// written or renamed over it.
    Failed { file: PathBuf, cause: io::Error },
}

/// Deletes the region with its markers, the user's bytes before and after it kept byte
/// for byte, by the same write as `replace`. Where no region stands, nothing is written,
/// created, or warned about: no `.git/info`, no file, a dangling link at it, or a link at
/// `.git/info` through which no region is read.
pub fn delete(workspace: &Workspace) -> Result<Deleted, NotDeleted> {
    let info = workspace.common_directory().join("info");
    let exclude = exclude_file(workspace);
    let failed = |cause| NotDeleted::Failed {
        file: exclude_file(workspace),
        cause,
    };
    let (bytes, mode, link) = match find(&info, &exclude) {
        Ok(Found::File { bytes, mode, link }) => (bytes, mode, link),
        Ok(Found::InfoLink) => {
            return match fs::read(&exclude) {
                Ok(bytes) if split(&bytes).rules.is_some() => Err(NotDeleted::BeyondALink(info)),
                Ok(_) => Ok(Deleted::default()),
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(Deleted::default()),
                Err(cause) => Err(failed(cause)),
            };
        }
        Ok(Found::NoInfo | Found::DanglingLink) => return Ok(Deleted::default()),
        Err(cause) => return Err(failed(cause)),
    };
    let found = split(&bytes);
    let Some(rules) = &found.rules else {
        return Ok(Deleted::default());
    };
    let paths = paths_of(rules);
    let composed = deleted(&found);
    let holding = b"what its target held without the managed region";
    match write(workspace, &exclude, &composed, mode, link, holding) {
        Ok(warning) => Ok(Deleted { paths, warning }),
        Err(cause) => Err(failed(cause)),
    }
}

/// The file without its region: the user's bytes before the begin marker, then those
/// after the end marker's line.
fn deleted(split: &Split) -> Vec<u8> {
    [split.before, split.after].concat()
}

fn unmaintainable(path: &Path, what: &[u8]) -> Vec<u8> {
    [
        b"the managed region cannot be maintained: ",
        path.as_os_str().as_bytes(),
        b" ",
        what,
        b"; private files may be visible to public Git",
    ]
    .concat()
}

fn not_written(exclude: &Path, cause: &str) -> Vec<u8> {
    [
        b"cannot replace the managed region of ",
        exclude.as_os_str().as_bytes(),
        b": ",
        cause.as_bytes(),
        b"; private files may be visible to public Git",
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    #[test]
    fn a_file_without_a_region_is_all_the_users_and_the_region_is_appended() {
        for (file, composed) in [
            (
                &b""[..],
                &b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"[..],
            ),
            (
                b"*.o\n",
                b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
            // The begin marker starts a line; the user's bytes stay a prefix.
            (
                b"*.o",
                b"*.o\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
            (
                b"a\r\n",
                b"a\r\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n",
            ),
        ] {
            let found = split(file);
            assert_eq!(found.before, file);
            assert_eq!(found.rules, None);
            assert_eq!(compose(&found, &paths(&[b".gitdupe"])), composed);
        }
    }

    #[test]
    fn a_region_is_rewritten_where_it_stands_with_the_users_bytes_around_it() {
        let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/old\n# END git-dupe\nafter\nlast";
        let found = split(file);
        assert_eq!(found.before, b"*.o\n");
        assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
        assert_eq!(found.after, b"after\nlast");
        assert_eq!(
            compose(&found, &paths(&[b".gitdupe", b"new"])),
            b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/new\n# END git-dupe\nafter\nlast"
        );
    }

    #[test]
    fn a_begin_marker_without_an_end_marker_runs_to_the_end_of_the_file() {
        for file in [
            &b"x\n# BEGIN git-dupe\n/.gitdupe\n/old\n"[..],
            b"x\n# BEGIN git-dupe\n/.gitdupe\n/old",
        ] {
            let found = split(file);
            assert_eq!(found.before, b"x\n");
            assert_eq!(found.rules, Some(vec![&b"/.gitdupe"[..], b"/old"]));
            assert_eq!(found.after, b"");
            assert_eq!(
                compose(&found, &paths(&[b".gitdupe"])),
                b"x\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"
            );
        }
    }

    #[test]
    fn the_first_begin_marker_and_the_first_end_marker_after_it_bound_the_region() {
        let file = b"# END git-dupe\n# BEGIN git-dupe\n/a\n# END git-dupe\n# BEGIN git-dupe\n# END git-dupe\n";
        let found = split(file);
        assert_eq!(found.before, b"# END git-dupe\n");
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(found.after, b"# BEGIN git-dupe\n# END git-dupe\n");
    }

    #[test]
    fn a_marker_is_a_whole_line() {
        for file in [
            &b"  # BEGIN git-dupe\n/a\n"[..],
            b"# BEGIN git-dupe \n/a\n",
            b"# BEGIN git-dupe\r\n/a\n",
        ] {
            assert_eq!(split(file).rules, None, "{}", file.escape_ascii());
        }
        let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe\r\nx\n");
        assert_eq!(
            found.rules,
            Some(vec![&b"/a"[..], b"# END git-dupe\r", b"x"])
        );
    }

    #[test]
    fn an_end_marker_without_a_final_newline_ends_the_file() {
        let found = split(b"# BEGIN git-dupe\n/a\n# END git-dupe");
        assert_eq!(found.before, b"");
        assert_eq!(found.rules, Some(vec![&b"/a"[..]]));
        assert_eq!(found.after, b"");
    }

    #[test]
    fn deleting_the_region_keeps_the_users_bytes_around_it_and_nothing_else() {
        let file = b"*.o\n# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\nafter\nlast";
        assert_eq!(deleted(&split(file)), b"*.o\nafter\nlast");
        assert_eq!(
            deleted(&split(b"# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n")),
            b""
        );
        // A begin marker without an end marker: everything from it goes.
        assert_eq!(
            deleted(&split(b"x\n# BEGIN git-dupe\n/a\n# END git-dupe \ny\n")),
            b"x\n"
        );
        // The newline composing put before the begin marker stays: it cannot be told from
        // the user's own.
        let composed = compose(&split(b"*.o"), &paths(&[b".gitdupe"]));
        assert_eq!(deleted(&split(&composed)), b"*.o\n");
    }

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_deletion_that_cannot_write_leaves_the_region_and_one_that_finds_none_writes_nothing() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-region-{}", std::process::id())));
        let _ = fs::remove_dir_all(&scratch.0);
        let workspace = Workspace::at_root(&scratch.0);
        let info = scratch.0.join(".git/info");
        fs::create_dir_all(&info).unwrap();
        let exclude = info.join("exclude");
        let file = b"mine\n# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\nafter\n";
        fs::write(&exclude, file).unwrap();

        // No private Git directory to make the fresh file in: the write fails, and the
        // region stands as it was.
        match delete(&workspace) {
            Err(NotDeleted::Failed { file: named, .. }) => assert_eq!(named, exclude),
            _ => panic!("the deletion wrote without a fresh file"),
        }
        assert_eq!(fs::read(&exclude).unwrap(), file);

        fs::create_dir(scratch.0.join(".git/dupe")).unwrap();
        let Ok(found) = delete(&workspace) else {
            panic!("the deletion failed");
        };
        assert_eq!(found.paths, paths(&[b".gitdupe", b"notes"]));
        assert_eq!(found.warning, None);
        assert_eq!(fs::read(&exclude).unwrap(), b"mine\nafter\n");

        // With no region left, and with no `.git/info`, nothing is written or created.
        let Ok(none) = delete(&workspace) else {
            panic!("the deletion failed");
        };
        assert!(none.paths.is_empty());
        assert_eq!(fs::read(&exclude).unwrap(), b"mine\nafter\n");
        // A file that cannot be read may hold a region: the deletion refuses it.
        fs::remove_file(&exclude).unwrap();
        fs::create_dir(&exclude).unwrap();
        assert!(matches!(delete(&workspace), Err(NotDeleted::Failed { .. })));
        assert!(exclude.is_dir());
        fs::remove_dir_all(&info).unwrap();
        assert!(delete(&workspace).is_ok());
        assert!(!info.exists());
        assert_eq!(
            fs::read_dir(scratch.0.join(".git/dupe")).unwrap().count(),
            0
        );
    }

    #[test]
    fn composing_what_was_composed_changes_nothing() {
        let region = paths(&[b".gitdupe", b"has space", b"cr\rx"]);
        let once = compose(&split(b"user text"), &region);
        assert_eq!(compose(&split(&once), &region), once);
        let read_back: Vec<Vec<u8>> = split(&once)
            .rules
            .unwrap()
            .into_iter()
            .filter_map(rule::path_of)
            .collect();
        assert_eq!(read_back, region);
    }
}
