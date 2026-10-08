//! The stale check at the site: a region whose worktree's private Git directory is not a
//! directory by `lstat` is left out of what is written, every other byte is kept, and the
//! regions kept are handed back where the region was maintained (G27).

use super::*;

/// The user's text around this worktree's region, the live `agent` region, and the region
/// of `gone`, whose private Git directory the check decides.
const FILE: &[u8] = b"top\n\
    # BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\n\
    # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
    middle\n\
    # BEGIN git-dupe worktree gone\n/gone\n/gone/x\n# END git-dupe worktree gone\n\
    bottom";

/// `FILE` without the region of `gone`.
const PRUNED: &[u8] = b"top\n\
    # BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\n\
    # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
    middle\n\
    bottom";

fn owners(others: &[foreign::Region]) -> Vec<(Owner, Vec<Vec<u8>>)> {
    others
        .iter()
        .map(|other| (other.owner.clone(), other.paths.clone()))
        .collect()
}

fn agent() -> (Owner, Vec<Vec<u8>>) {
    (Owner::Linked(b"agent".to_vec()), paths(&[b"agent"]))
}

#[test]
fn a_region_whose_repository_is_not_a_directory_is_left_out_of_an_equal_composition() {
    let scratch = Scratch::new("stale");
    let workspace = scratch.workspace();
    let gone = scratch.0.join(".git/worktrees/gone");
    let region = paths(&[b".gitdupe", b"notes"]);
    let target = scratch.0.join("a-repository");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("HEAD"), b"ref: refs/heads/main\n").unwrap();
    // Nothing there; a file; a link to nothing; a link to a directory that is a valid
    // repository: each is a repository gone, whatever the link leads to.
    let stand_ins: [&dyn Fn(&Path); 4] = [
        &|_| {},
        &|dupe| fs::write(dupe, b"").unwrap(),
        &|dupe| symlink(scratch.0.join("nothing"), dupe).unwrap(),
        &|dupe| symlink(&target, dupe).unwrap(),
    ];
    for stand_in in stand_ins {
        let _ = fs::remove_dir_all(&gone);
        fs::create_dir_all(&gone).unwrap();
        stand_in(&gone.join("dupe"));
        fs::write(scratch.exclude(), FILE).unwrap();
        fs::set_permissions(scratch.exclude(), fs::Permissions::from_mode(0o640)).unwrap();
        let before = identity(&scratch.exclude());

        // This worktree's own composition is unchanged; the stale region alone is written
        // away, every other byte kept, the permissions too.
        let replaced = replace(&workspace, &region);
        assert!(replaced.warnings.is_empty(), "{:?}", replaced.warnings);
        assert_eq!(fs::read(scratch.exclude()).unwrap(), PRUNED);
        assert_ne!(identity(&scratch.exclude()), before);
        assert_eq!(mode(&scratch.exclude()), 0o640);
        assert_eq!(owners(&replaced.others.unwrap()), [agent()]);
        assert!(scratch.lock_is_free());
    }
    // What the link led to is untouched.
    assert_eq!(
        fs::read(target.join("HEAD")).unwrap(),
        b"ref: refs/heads/main\n"
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 1);

    // A directory there, empty or holding no repository Git could open, is a repository
    // that stands: its region is kept byte for byte, and handed back.
    let _ = fs::remove_dir_all(&gone);
    fs::create_dir_all(gone.join("dupe")).unwrap();
    fs::write(scratch.exclude(), FILE).unwrap();
    let before = identity(&scratch.exclude());
    let replaced = replace(&workspace, &region);
    assert!(replaced.warnings.is_empty());
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(fs::read(scratch.exclude()).unwrap(), FILE);
    assert_eq!(
        owners(&replaced.others.unwrap()),
        [
            agent(),
            (
                Owner::Linked(b"gone".to_vec()),
                paths(&[b"gone", b"gone/x"])
            )
        ]
    );
}

#[test]
fn the_main_worktrees_region_is_stale_where_its_repository_is_gone() {
    let scratch = Scratch::new("stale-main");
    let common = scratch.0.join(".git");
    let linked = Workspace::linked_at(&common, b"agent", &scratch.0.join("agent-root"));
    fs::remove_dir(common.join("dupe")).unwrap();
    fs::write(scratch.exclude(), FILE).unwrap();

    let replaced = replace(&linked, &paths(&[b".gitdupe", b"mine"]));
    assert!(replaced.warnings.is_empty(), "{:?}", replaced.warnings);
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        b"top\n\
          # BEGIN git-dupe worktree agent\n/.gitdupe\n/mine\n# END git-dupe worktree agent\n\
          middle\n\
          bottom"
    );
    assert!(replaced.others.unwrap().is_empty());
}

#[test]
fn a_deletion_without_a_region_of_its_own_still_leaves_out_a_stale_one() {
    let scratch = Scratch::new("stale-delete");
    let workspace = scratch.workspace();
    let file = b"top\n\
        # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
        # BEGIN git-dupe worktree gone\n/gone\n# END git-dupe worktree gone\n\
        bottom";
    fs::write(scratch.exclude(), file).unwrap();
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion failed");
    };
    assert!(deleted.paths.is_empty());
    assert_eq!(deleted.warning, None);
    assert_eq!(owners(&deleted.others), [agent()]);
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        b"top\n\
          # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
          bottom"
    );

    // With nothing stale and no region of its own, nothing is written, and the regions
    // are handed back all the same.
    let before = identity(&scratch.exclude());
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion failed");
    };
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(owners(&deleted.others), [agent()]);

    // Its own region deleted, a stale one goes with it, and the live ones come back.
    fs::write(scratch.exclude(), FILE).unwrap();
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion failed");
    };
    assert_eq!(deleted.paths, paths(&[b".gitdupe", b"notes"]));
    assert_eq!(owners(&deleted.others), [agent()]);
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        b"top\n\
          # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
          middle\n\
          bottom"
    );
}

#[test]
fn a_replacement_that_maintains_nothing_hands_back_nothing_and_drops_nothing() {
    // The write fails: the stale region stands with every other byte, and no region is
    // handed back for G27 to name.
    let scratch = Scratch::new("stale-unwritten");
    let workspace = scratch.workspace();
    fs::remove_dir(scratch.0.join(".git/dupe")).unwrap();
    fs::write(scratch.exclude(), FILE).unwrap();
    fs::set_permissions(scratch.exclude(), fs::Permissions::from_mode(0o640)).unwrap();
    let before = identity(&scratch.exclude());
    let replaced = replace(&workspace, &paths(&[b".gitdupe", b"other"]));
    assert_eq!(replaced.warnings.len(), 1);
    assert!(replaced.others.is_none());
    assert_eq!(fs::read(scratch.exclude()).unwrap(), FILE);
    assert_eq!(identity(&scratch.exclude()), before);
    assert_eq!(mode(&scratch.exclude()), 0o640);
    // The retry, once it can write, makes the whole new composition.
    fs::create_dir(scratch.0.join(".git/dupe")).unwrap();
    let replaced = replace(&workspace, &paths(&[b".gitdupe", b"other"]));
    assert!(replaced.warnings.is_empty());
    assert_eq!(owners(&replaced.others.unwrap()), [agent()]);
    assert_eq!(
        fs::read(scratch.exclude()).unwrap(),
        b"top\n\
          # BEGIN git-dupe\n/.gitdupe\n/other\n# END git-dupe\n\
          # BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
          middle\n\
          bottom"
    );

    // A link at `.git/info`, and one to nothing at the file: left as they are, nothing
    // handed back.
    let scratch = Scratch::new("stale-links");
    let workspace = scratch.workspace();
    let elsewhere = scratch.0.join("shared");
    fs::create_dir(&elsewhere).unwrap();
    fs::write(elsewhere.join("exclude"), FILE).unwrap();
    fs::remove_dir(scratch.info()).unwrap();
    symlink(&elsewhere, scratch.info()).unwrap();
    let replaced = replace(&workspace, &paths(&[b".gitdupe"]));
    assert_eq!(replaced.warnings.len(), 1);
    assert!(replaced.others.is_none());
    assert_eq!(fs::read(elsewhere.join("exclude")).unwrap(), FILE);
    fs::remove_file(scratch.info()).unwrap();
    fs::create_dir(scratch.info()).unwrap();
    symlink(scratch.0.join("nothing"), scratch.exclude()).unwrap();
    let replaced = replace(&workspace, &paths(&[b".gitdupe"]));
    assert_eq!(replaced.warnings.len(), 1);
    assert!(replaced.others.is_none());
}

#[test]
fn a_deletion_beyond_a_link_without_its_region_writes_nothing_and_hands_back_the_live_ones() {
    let scratch = Scratch::new("stale-beyond");
    let workspace = scratch.workspace();
    let elsewhere = scratch.0.join("shared");
    fs::create_dir(&elsewhere).unwrap();
    let file = b"# BEGIN git-dupe worktree agent\n/agent\n# END git-dupe worktree agent\n\
                 # BEGIN git-dupe worktree gone\n/gone\n# END git-dupe worktree gone\n";
    fs::write(elsewhere.join("exclude"), file).unwrap();
    fs::remove_dir(scratch.info()).unwrap();
    symlink(&elsewhere, scratch.info()).unwrap();
    let Ok(deleted) = delete(&workspace) else {
        panic!("the deletion refused");
    };
    assert!(deleted.paths.is_empty());
    assert_eq!(owners(&deleted.others), [agent()]);
    assert_eq!(fs::read(elsewhere.join("exclude")).unwrap(), file);
}
