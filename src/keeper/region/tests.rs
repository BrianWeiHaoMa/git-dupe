//! The one site on disk: what it writes, what it leaves, and where it writes nothing.

use super::*;
use std::os::unix::fs::{MetadataExt, symlink};

fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
    paths.iter().map(|path| path.to_vec()).collect()
}

/// A directory of the test's own below the system's temporary one, removed after, with
/// `.git/info` and the private Git directory `.git/dupe` made inside it.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let path =
            std::env::temp_dir().join(format!("git-dupe-region-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".git/info")).unwrap();
        fs::create_dir(path.join(".git/dupe")).unwrap();
        Scratch(path)
    }

    fn workspace(&self) -> Workspace {
        Workspace::at_root(&self.0)
    }

    fn info(&self) -> PathBuf {
        self.0.join(".git/info")
    }

    fn exclude(&self) -> PathBuf {
        self.0.join(".git/info/exclude")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The device and inode of what stands at `path`, by `lstat`: the same pair after a
/// command is the same file, never one renamed over it.
fn identity(path: &Path) -> (u64, u64) {
    let found = fs::symlink_metadata(path).unwrap();
    (found.dev(), found.ino())
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// The user's text and another worktree's region, as another worktree's command leaves
/// them, the user's last line without its newline.
const FOREIGN: &[u8] = b"*.o\n\
    # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
    last";

#[test]
fn a_deletion_that_cannot_write_leaves_the_region_and_one_that_finds_none_writes_nothing() {
    let scratch = Scratch::new("delete");
    let workspace = scratch.workspace();
    fs::remove_dir(scratch.0.join(".git/dupe")).unwrap();
    let exclude = scratch.exclude();
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
    fs::remove_dir_all(scratch.info()).unwrap();
    assert!(delete(&workspace).is_ok());
    assert!(!scratch.info().exists());
    assert_eq!(
        fs::read_dir(scratch.0.join(".git/dupe")).unwrap().count(),
        0
    );
}

#[test]
fn a_replacement_that_cannot_write_leaves_the_file_as_it_was() {
    let scratch = Scratch::new("unwritten");
    let workspace = scratch.workspace();
    fs::remove_dir(scratch.0.join(".git/dupe")).unwrap();
    fs::write(scratch.exclude(), FOREIGN).unwrap();
    fs::set_permissions(scratch.exclude(), fs::Permissions::from_mode(0o640)).unwrap();
    let before = identity(&scratch.exclude());

    let warnings = replace(&workspace, &paths(&[b".gitdupe"]));
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].starts_with(b"cannot replace the managed region of "),
        "{}",
        warnings[0].escape_ascii()
    );
    assert_eq!(fs::read(scratch.exclude()).unwrap(), FOREIGN);
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(mode(&scratch.exclude()), 0o640);
}

#[test]
fn a_region_is_appended_after_the_others_and_an_equal_composition_writes_nothing() {
    let scratch = Scratch::new("equal");
    let workspace = scratch.workspace();
    fs::write(scratch.exclude(), FOREIGN).unwrap();
    fs::set_permissions(scratch.exclude(), fs::Permissions::from_mode(0o640)).unwrap();
    let region = paths(&[b".gitdupe", b"notes"]);

    assert!(replace(&workspace, &region).is_empty());
    let composed = [
        FOREIGN,
        b"\n# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\n",
    ]
    .concat();
    assert_eq!(fs::read(scratch.exclude()).unwrap(), composed);
    assert_eq!(mode(&scratch.exclude()), 0o640);

    // The same region again: the file is the one already there, not a copy of it.
    let written = identity(&scratch.exclude());
    assert!(replace(&workspace, &region).is_empty());
    assert_eq!(identity(&scratch.exclude()), written);
    assert_eq!(fs::read(scratch.exclude()).unwrap(), composed);
    assert_eq!(
        fs::read_dir(scratch.0.join(".git/dupe")).unwrap().count(),
        0
    );
}

#[test]
fn a_deletion_where_this_worktrees_region_does_not_stand_writes_nothing() {
    let scratch = Scratch::new("absent");
    let workspace = scratch.workspace();
    fs::write(scratch.exclude(), FOREIGN).unwrap();
    let before = identity(&scratch.exclude());
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion failed");
    };
    assert!(deleted.paths.is_empty());
    assert_eq!(deleted.warning, None);
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(fs::read(scratch.exclude()).unwrap(), FOREIGN);

    // Between other regions, the region goes and the bytes around it are joined in order.
    let file = b"top\n\
        # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
        # BEGIN git-dupe\n/.gitdupe\n/mine\n# END git-dupe\n\
        # BEGIN git-dupe worktree other\n/other\n# END git-dupe worktree other\n\
        bottom";
    fs::write(scratch.exclude(), file).unwrap();
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion failed");
    };
    assert_eq!(deleted.paths, paths(&[b".gitdupe", b"mine"]));
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        b"top\n\
          # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
          # BEGIN git-dupe worktree other\n/other\n# END git-dupe worktree other\n\
          bottom"
    );

    // A link to nothing at the file is left as it is.
    fs::remove_file(scratch.exclude()).unwrap();
    symlink(scratch.0.join("nowhere"), scratch.exclude()).unwrap();
    assert!(delete(&workspace).is_ok_and(|deleted| deleted.paths.is_empty()));
    assert!(
        fs::symlink_metadata(scratch.exclude())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!scratch.0.join("nowhere").exists());
}

#[test]
fn a_link_at_the_file_becomes_a_regular_file_even_when_its_target_holds_the_composition() {
    let scratch = Scratch::new("link");
    let workspace = scratch.workspace();
    let region = paths(&[b".gitdupe"]);
    let composed = b"mine\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n";
    let target = scratch.0.join("shared-exclude");
    fs::write(&target, composed).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let target_before = identity(&target);
    symlink(&target, scratch.exclude()).unwrap();

    let warnings = replace(&workspace, &region);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].ends_with(b", and its target is untouched"));
    assert!(fs::symlink_metadata(scratch.exclude()).unwrap().is_file());
    assert_eq!(fs::read(scratch.exclude()).unwrap(), composed);
    assert_eq!(mode(&scratch.exclude()), 0o600);
    assert_eq!(fs::read(&target).unwrap(), composed);
    assert_eq!(identity(&target), target_before);
}

/// Makes `.git/info` as another worktree's command would, with an exclude file holding
/// `FOREIGN`, just before this one tries.
fn another_made_info_with_a_file(info: &Path) -> io::Result<()> {
    fs::create_dir(info)?;
    fs::write(info.join("exclude"), FOREIGN)?;
    fs::create_dir(info)
}

/// Leaves a regular file at `.git/info` just before this command tries to make it.
fn another_left_a_file_at_info(info: &Path) -> io::Result<()> {
    fs::write(info, b"not a directory\n")?;
    fs::create_dir(info)
}

/// Leaves a symbolic link at `.git/info`, to a directory beside it holding an exclude
/// file, just before this command tries to make it.
fn another_left_a_link_at_info(info: &Path) -> io::Result<()> {
    let elsewhere = info.with_file_name("elsewhere");
    fs::create_dir(&elsewhere)?;
    fs::write(elsewhere.join("exclude"), FOREIGN)?;
    symlink(&elsewhere, info)?;
    fs::create_dir(info)
}

fn refused(info: &Path) -> io::Result<()> {
    let _ = info;
    Err(io::Error::from(io::ErrorKind::PermissionDenied))
}

#[test]
fn info_made_by_another_command_first_is_looked_at_again_and_read() {
    let region = paths(&[b".gitdupe"]);
    let appended = b"\n# BEGIN git-dupe\n/.gitdupe\n# END git-dupe\n";

    // A directory: it counts as made, and the file in it is read, not taken for empty.
    let scratch = Scratch::new("race-directory");
    fs::remove_dir(scratch.info()).unwrap();
    let made = update(
        &scratch.workspace(),
        Change::Set(&region),
        another_made_info_with_a_file,
    );
    assert!(made.is_ok());
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        [FOREIGN, appended].concat()
    );

    // A regular file: it is left as it is, and the write fails naming the cause.
    let scratch = Scratch::new("race-file");
    fs::remove_dir(scratch.info()).unwrap();
    let made = update(
        &scratch.workspace(),
        Change::Set(&region),
        another_left_a_file_at_info,
    );
    assert!(matches!(made, Err(NotUpdated::Failed(_))));
    assert_eq!(fs::read(scratch.info()).unwrap(), b"not a directory\n");

    // A symbolic link: it is not followed, and its target is untouched.
    let scratch = Scratch::new("race-link");
    fs::remove_dir(scratch.info()).unwrap();
    let made = update(
        &scratch.workspace(),
        Change::Set(&region),
        another_left_a_link_at_info,
    );
    assert!(matches!(made, Err(NotUpdated::InfoLink)));
    assert!(
        fs::symlink_metadata(scratch.info())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read(scratch.0.join(".git/elsewhere/exclude")).unwrap(),
        FOREIGN
    );

    // Any other failure to make it is the cause, and nothing is written.
    let scratch = Scratch::new("race-refused");
    fs::remove_dir(scratch.info()).unwrap();
    let made = update(&scratch.workspace(), Change::Set(&region), refused);
    assert!(
        matches!(made, Err(NotUpdated::Failed(cause)) if cause.kind() == io::ErrorKind::PermissionDenied)
    );
    assert!(!scratch.info().exists());

    // Made here, it holds the region alone.
    let scratch = Scratch::new("race-none");
    fs::remove_dir(scratch.info()).unwrap();
    assert!(replace(&scratch.workspace(), &region).is_empty());
    assert_eq!(fs::read(scratch.exclude()).unwrap(), &appended[1..]);
}

#[test]
fn only_this_worktrees_region_is_read_and_refused_beyond_a_link() {
    let scratch = Scratch::new("beyond");
    let main = scratch.workspace();
    let linked = Workspace::linked_at(
        &scratch.0.join(".git"),
        b"agent",
        &scratch.0.join("elsewhere"),
    );
    let elsewhere = scratch.0.join("shared");
    fs::create_dir(&elsewhere).unwrap();
    fs::remove_dir(scratch.info()).unwrap();
    symlink(&elsewhere, scratch.info()).unwrap();

    // Another worktree's region beyond the link is not the main worktree's.
    fs::write(elsewhere.join("exclude"), FOREIGN).unwrap();
    assert_eq!(beyond_a_link(&main), None);
    assert!(start(&main).paths.is_empty());
    assert!(delete(&main).is_ok_and(|deleted| deleted.paths.is_empty()));
    assert_eq!(beyond_a_link(&linked), Some(scratch.info()));
    assert_eq!(start(&linked).paths, paths(&[b"agent"]));
    assert!(matches!(delete(&linked), Err(NotDeleted::BeyondALink(_))));

    // The main worktree's region there is the main worktree's alone.
    fs::write(
        elsewhere.join("exclude"),
        b"# BEGIN git-dupe\n/.gitdupe\n/mine\n# END git-dupe\n",
    )
    .unwrap();
    assert_eq!(beyond_a_link(&main), Some(scratch.info()));
    assert_eq!(start(&main).paths, paths(&[b".gitdupe", b"mine"]));
    assert!(matches!(delete(&main), Err(NotDeleted::BeyondALink(_))));
    assert_eq!(beyond_a_link(&linked), None);
    assert!(start(&linked).paths.is_empty());
    // Nothing beyond the link was written.
    assert_eq!(
        fs::read(elsewhere.join("exclude")).unwrap(),
        b"# BEGIN git-dupe\n/.gitdupe\n/mine\n# END git-dupe\n"
    );
}
