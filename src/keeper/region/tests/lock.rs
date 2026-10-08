//! The lock at the site: held from before the file is read until after the rename, and,
//! where it cannot be taken, nothing done without it.

use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn the_lock_is_held_from_before_the_file_is_read_until_after_the_rename() {
    let scratch = Scratch::new("held");
    let workspace = scratch.workspace();
    let common = scratch.0.join(".git");
    let exclude = scratch.exclude();
    fs::write(&exclude, b"mine\n").unwrap();
    // What another descriptor's `try_lock` said when the lock was taken and when it was
    // let go, and what the file held at that moment.
    let taken: RefCell<Option<(bool, Vec<u8>)>> = RefCell::new(None);
    let released: RefCell<Option<(bool, Vec<u8>)>> = RefCell::new(None);
    let take = |directory: &File| {
        directory.lock()?;
        *taken.borrow_mut() = Some((lock_is_free(&common), fs::read(&exclude)?));
        // Another worktree's command wrote its region just before this one took the lock:
        // the update reads it now, not before.
        let mut written = fs::read(&exclude)?;
        written.extend_from_slice(&FOREIGN[4..]);
        written.push(b'\n');
        fs::write(&exclude, written)
    };
    let release = |directory: &File| {
        *released.borrow_mut() = Some((lock_is_free(&common), fs::read(&exclude)?));
        directory.unlock()
    };
    let locking = Locking {
        take: &take,
        release: &release,
    };
    let region = paths(&[b".gitdupe"]);

    let updated = update(
        &workspace,
        Change::Set(&region),
        |info| fs::create_dir(info),
        &locking,
    );
    assert!(updated.is_ok_and(|updated| updated.warning.is_none()));
    let composed = b"mine\n\
        # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\nlast\n\
        # BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n";
    assert_eq!(fs::read(&exclude).unwrap(), composed);
    assert_eq!(taken.take(), Some((false, b"mine\n".to_vec())));
    assert_eq!(released.take(), Some((false, composed.to_vec())));
    assert!(lock_is_free(&common));
}

/// A lock that cannot be taken, for a reason other than another holder: no locks left.
fn no_locks(directory: &File) -> io::Result<()> {
    let _ = directory;
    Err(io::Error::from_raw_os_error(37))
}

#[test]
fn a_lock_that_cannot_be_taken_is_no_wait_and_nothing_follows_it() {
    let scratch = Scratch::new("not-locked");
    let workspace = scratch.workspace();
    let common = scratch.0.join(".git");
    let file = [FOREIGN, b"\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n"].concat();
    fs::write(scratch.exclude(), &file).unwrap();
    fs::set_permissions(scratch.exclude(), fs::Permissions::from_mode(0o640)).unwrap();
    let before = identity(&scratch.exclude());
    let released = Cell::new(false);
    let release = |directory: &File| {
        released.set(true);
        directory.unlock()
    };
    let failing = Locking {
        take: &no_locks,
        release: &release,
    };
    let region = paths(&[b".gitdupe", b"notes"]);

    // The replacement: one warning naming the directory, the cause, and what it may
    // expose, and the file as it was.
    let warnings = replaced(
        &workspace,
        update(&workspace, Change::Set(&region), refused, &failing),
    );
    assert_eq!(
        warnings,
        vec![
            [
                b"cannot take the lock on ",
                common.as_os_str().as_bytes(),
                b" to replace the managed region: ",
                io::Error::from_raw_os_error(37).to_string().as_bytes(),
                b"; private files may be visible to public Git",
            ]
            .concat()
        ]
    );
    // The deletion: the cause, and the region standing.
    match deleted(
        &workspace,
        update(&workspace, Change::Remove, refused, &failing),
    ) {
        Err(NotDeleted::NotLocked { directory, cause }) => {
            assert_eq!(directory, common);
            assert_eq!(cause.raw_os_error(), Some(37));
        }
        _ => panic!("the deletion went on without the lock"),
    }
    assert_eq!(fs::read(scratch.exclude()).unwrap(), file);
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(mode(&scratch.exclude()), 0o640);
    assert_eq!(
        fs::read_dir(scratch.0.join(".git/dupe")).unwrap().count(),
        0
    );

    // Not even a missing `.git/info` is made.
    fs::remove_dir_all(scratch.info()).unwrap();
    let make_info = |info: &Path| {
        let _ = info;
        Err(io::Error::other("made after a lock that was not taken"))
    };
    let updated = update(&workspace, Change::Set(&region), make_info, &failing);
    assert!(matches!(updated, Err(NotUpdated::NotLocked(_))));
    assert!(!scratch.info().exists());
    assert!(!released.get());

    // Where the common Git directory cannot be opened, the product's own replacement and
    // deletion say so, and make nothing.
    let gone = Workspace::at_root(&scratch.0.join("gone"));
    let warnings = replace(&gone, &region);
    assert!(
        warnings.len() == 1 && warnings[0].starts_with(b"cannot take the lock on "),
        "{warnings:?}"
    );
    assert!(matches!(
        delete(&gone),
        Err(NotDeleted::NotLocked { cause, .. }) if cause.kind() == io::ErrorKind::NotFound
    ));
    assert!(!scratch.0.join("gone").exists());
}
