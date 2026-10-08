//! The managed region of `.git/info/exclude` (F3): what it held when the command began,
//! its replacement by settle, and its deletion for `detach`.
//!
//! `sections` says where this worktree's region lies and composes the file around it,
//! every other byte copied as it stands; this worktree's markers are named from the
//! runner's facts alone (`Composition/Runner`). This module looks at the file, reads it,
//! and writes it.
//!
//! Settle's replacement and `detach`'s deletion are one `update`, the one place the file
//! is read to be written (R10): the `lstat` of `.git/info` and of the file, its bytes read
//! through a link at it, the composition, and a fresh file from the private Git directory
//! renamed over it with the original's permissions. They differ in what they compose and
//! where nothing stands. The replacement creates `.git/info` when it is missing — one
//! that another command made first counting as made, and looked at again — makes no write
//! when the composition equals a regular file as read, and replaces a link at the file by
//! a regular file. The deletion writes and creates nothing where this worktree's region
//! does not stand: no `.git/info`, no file, a dangling link at it, or a link at
//! `.git/info` through which no such region is read. An exclude file that cannot be read
//! holds no region for `start`; the deletion refuses it instead, because `detach` succeeds
//! only once no region stands.

mod sections;

use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::replace;
use crate::runner::locate::Workspace;
use sections::Worktree;

/// This worktree's markers: the main worktree's, or the linked worktree's of its name.
fn own(workspace: &Workspace) -> Worktree<'_> {
    match workspace.worktree_name() {
        Some(name) => Worktree::Linked(name.as_bytes()),
        None => Worktree::Main,
    }
}

/// The region as a command began: the paths its rules were written for. A line that is
/// no rule of the region's is not read, and a file that cannot be read holds none.
#[derive(Default)]
pub struct StartingRegion {
    pub paths: Vec<Vec<u8>>,
}

pub fn start(workspace: &Workspace) -> StartingRegion {
    let bytes = fs::read(exclude_file(workspace)).unwrap_or_default();
    let paths = sections::split(&bytes, own(workspace)).paths();
    StartingRegion { paths }
}

/// `.git/info` where it is a symbolic link by `lstat` and the file read through it holds
/// this worktree's region, as `start` reads it: the region `detach` refuses to leave
/// behind (G3). Another worktree's region there is not this one.
pub fn beyond_a_link(workspace: &Workspace) -> Option<PathBuf> {
    let info = info_directory(workspace);
    let link = fs::symlink_metadata(&info).is_ok_and(|found| found.file_type().is_symlink());
    (link && holds_region(workspace)).then_some(info)
}

fn holds_region(workspace: &Workspace) -> bool {
    let bytes = fs::read(exclude_file(workspace)).unwrap_or_default();
    sections::split(&bytes, own(workspace)).rules.is_some()
}

fn info_directory(workspace: &Workspace) -> PathBuf {
    workspace.common_directory().join("info")
}

fn exclude_file(workspace: &Workspace) -> PathBuf {
    info_directory(workspace).join("exclude")
}

/// Replaces the region with one rule per path, in the order given, and returns the
/// warnings: that the region cannot be maintained where a symbolic link stands, that a
/// write failed and why, or that a link at the exclude file became a regular file.
pub fn replace(workspace: &Workspace, paths: &[Vec<u8>]) -> Vec<Vec<u8>> {
    match update(workspace, Change::Set(paths), |info| fs::create_dir(info)) {
        Ok(updated) => updated.warning.into_iter().collect(),
        Err(NotUpdated::InfoLink) => {
            vec![unmaintainable(
                &info_directory(workspace),
                b"is a symbolic link",
            )]
        }
        Err(NotUpdated::DanglingLink) => vec![unmaintainable(
            &exclude_file(workspace),
            b"is a symbolic link to nothing",
        )],
        Err(NotUpdated::Failed(cause)) => {
            vec![not_written(&exclude_file(workspace), &cause.to_string())]
        }
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

/// Deletes the region with its markers, every other byte kept as it stands, by the same
/// update as `replace`. Where the region does not stand, nothing is written, created, or
/// warned about.
pub fn delete(workspace: &Workspace) -> Result<Deleted, NotDeleted> {
    match update(workspace, Change::Remove, |info| fs::create_dir(info)) {
        Ok(Updated { paths, warning }) => Ok(Deleted { paths, warning }),
        Err(NotUpdated::InfoLink) => Err(NotDeleted::BeyondALink(info_directory(workspace))),
        // Nothing is read through a link to nothing: no region stands there.
        Err(NotUpdated::DanglingLink) => Ok(Deleted::default()),
        Err(NotUpdated::Failed(cause)) => Err(NotDeleted::Failed {
            file: exclude_file(workspace),
            cause,
        }),
    }
}

/// What `update` is asked to do with this worktree's region.
enum Change<'p> {
    /// Hold exactly one rule per path, in the order given.
    Set(&'p [Vec<u8>]),
    /// Go, with its markers.
    Remove,
}

/// What `update` read and wrote: the paths the region's rules were written for as the file
/// was read, none where it held no region, and the warning that a link at the exclude file
/// is now a regular file.
#[derive(Default)]
struct Updated {
    paths: Vec<Vec<u8>>,
    warning: Option<Vec<u8>>,
}

/// Why `update` left the file as it stands.
enum NotUpdated {
    /// `.git/info` is a symbolic link by `lstat`: for the deletion, one through which the
    /// file holds the region.
    InfoLink,
    /// `.git/info/exclude` is a symbolic link to nothing.
    DanglingLink,
    /// Looking, reading, making `.git/info`, or writing or renaming the fresh file failed.
    Failed(io::Error),
}

/// Makes `.git/info`: `fs::create_dir`, except in a unit check that has another command
/// make it first.
type MakeInfo = fn(&Path) -> io::Result<()>;

/// The one site that reads `.git/info/exclude` to write it: looks, reads, composes the
/// change, and writes a fresh file renamed over it, or writes nothing where the change
/// leaves the file as it is.
fn update(
    workspace: &Workspace,
    change: Change,
    make_info: MakeInfo,
) -> Result<Updated, NotUpdated> {
    let info = info_directory(workspace);
    let exclude = exclude_file(workspace);
    let own = own(workspace);
    let creates = matches!(change, Change::Set(_)).then_some(make_info);
    let (bytes, mode, link) = match inspect(&info, &exclude, creates).map_err(NotUpdated::Failed)? {
        Found::File { bytes, mode, link } => (bytes, mode, link),
        Found::InfoLink => {
            return match change {
                Change::Set(_) => Err(NotUpdated::InfoLink),
                // The link and what it leads to stay as they are; the deletion only asks
                // whether the region stands beyond it, as `start` would read it.
                Change::Remove => match fs::read(&exclude) {
                    Ok(bytes) if sections::split(&bytes, own).rules.is_some() => {
                        Err(NotUpdated::InfoLink)
                    }
                    Ok(_) => Ok(Updated::default()),
                    Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(Updated::default()),
                    Err(cause) => Err(NotUpdated::Failed(cause)),
                },
            };
        }
        Found::DanglingLink => return Err(NotUpdated::DanglingLink),
        // Only the deletion finds no `.git/info`: the replacement has made it.
        Found::NoInfo => return Ok(Updated::default()),
    };

    let found = sections::split(&bytes, own);
    let paths = found.paths();
    let (composed, holding) = match change {
        Change::Set(set) => {
            let composed = found.set(own, set);
            if mode.is_some() && !link && composed == bytes {
                return Ok(Updated {
                    paths,
                    warning: None,
                });
            }
            (
                composed,
                &b"what its target held and the managed region"[..],
            )
        }
        Change::Remove => {
            if found.rules.is_none() {
                return Ok(Updated::default());
            }
            (
                found.removed(),
                &b"what its target held without the managed region"[..],
            )
        }
    };
    replace::whole(&workspace.private_directory(), &exclude, &composed, mode)
        .map_err(NotUpdated::Failed)?;
    Ok(Updated {
        paths,
        warning: link.then(|| link_replaced(&exclude, holding)),
    })
}

/// The exclude file as `update` finds it.
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

/// What stands at `.git/info` and at the exclude file inside it, by `lstat`, and the
/// file's bytes; with `make_info`, a missing `.git/info` is made first and holds no file.
/// When another command made it first, it is looked at again and what it holds is read:
/// a directory counts as made, and anything else there is what is found. A failure to
/// look, to read, or to make the directory is the error.
fn inspect(info: &Path, exclude: &Path, make_info: Option<MakeInfo>) -> io::Result<Found> {
    let found = find(info, exclude)?;
    let (Found::NoInfo, Some(make_info)) = (&found, make_info) else {
        return Ok(found);
    };
    match make_info(info) {
        Ok(()) => Ok(Found::File {
            bytes: Vec::new(),
            mode: None,
            link: false,
        }),
        Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => match find(info, exclude)? {
            Found::NoInfo => Err(cause),
            found => Ok(found),
        },
        Err(cause) => Err(cause),
    }
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

/// The warning that the link at the exclude file is now a regular file `holding` what it
/// holds, its target untouched.
fn link_replaced(exclude: &Path, holding: &[u8]) -> Vec<u8> {
    [
        exclude.as_os_str().as_bytes(),
        b" was a symbolic link; it is now a regular file holding ",
        holding,
        b", and its target is untouched",
    ]
    .concat()
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
mod tests;
