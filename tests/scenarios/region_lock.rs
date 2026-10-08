//! The lock on the common Git directory under which `.git/info/exclude` is replaced and
//! its region deleted (G28, `Holds/G28`, `State` "Region replacement"). The harness holds
//! the lock as another worktree's command would: settle's replacement and `detach`'s
//! deletion wait for it. A command killed while it holds the lock holds up none after it,
//! and leaves the file as it was. A write that fails leaves the file and the private
//! repository as they were and is named: settle keeps the command's status and still asks
//! about exposure, and `detach` refuses.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;

use crate::harness::{
    End, Output, Point, Scenario, commands_wait_for_the_lock, hold_the_lock, lock_is_free, region,
    region_rules, under_each_release, write,
};

/// How long a command is given to end once the lock is let go.
const LIMIT: Duration = Duration::from_secs(30);

/// The device and inode of `path`, by `lstat`.
fn identity(path: &Path) -> (u64, u64) {
    let found = fs::symlink_metadata(path).unwrap();
    (found.dev(), found.ino())
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// The names in the directory `directory`.
fn entries(directory: &Path) -> BTreeSet<OsString> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

#[test]
fn a_replacement_and_a_deletion_wait_while_another_holds_the_lock() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let common = dir.join(".git");
        let exclude = common.join("info/exclude");

        let held = hold_the_lock(&common);
        let as_it_was = fs::read(&exclude).unwrap();
        let mut hiding = s.git(["dupe", "hide", "x"]).from(&dir).start();
        commands_wait_for_the_lock(&common, 1);
        assert!(!hiding.finished());
        assert_eq!(fs::read(&exclude).unwrap(), as_it_was);
        drop(held);
        let hidden = hiding.wait_within(LIMIT);
        assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/x"]);

        let held = hold_the_lock(&common);
        let mut detaching = s.git(["dupe", "detach", "--force"]).from(&dir).start();
        commands_wait_for_the_lock(&common, 1);
        assert!(!detaching.finished());
        assert!(region(&dir).is_some() && common.join("dupe/HEAD").is_file());
        drop(held);
        let detached = detaching.wait_within(LIMIT);
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        assert_eq!(region(&dir), None);
        assert!(!common.join("dupe").exists());
    });
}

#[test]
fn a_command_killed_while_it_holds_the_lock_holds_up_none_after_it() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let common = dir.join(".git");
        let exclude = common.join("info/exclude");
        // The user's part of the file is larger than a block of either size, so that the
        // write is killed under either limit before the rename.
        let user: Vec<u8> = (0..200)
            .flat_map(|n| format!("# the user's line {n}\n").into_bytes())
            .collect();
        fs::write(&exclude, &user).unwrap();
        s.git(["dupe", "status"]).from(&dir).succeeds();
        fs::set_permissions(&exclude, fs::Permissions::from_mode(0o640)).unwrap();
        // `.gitdupe` edited by hand: the next `status` replaces the region.
        write(&dir, ".gitdupe", b"notes\n");
        let as_it_was = fs::read(&exclude).unwrap();
        let link = s.dir().join("exclude-as-it-was");
        fs::hard_link(&exclude, &link).unwrap();
        let private = common.join("dupe");
        let private_before = entries(&private);

        let killing = s.killing("control", &["status"]);
        for blocks in [0, 1] {
            let run = killing.start_killed(&dir, Point::InsideWrite { blocks });
            // While it runs, the file it replaces is never written.
            assert_eq!(fs::read(&link).unwrap(), as_it_was);
            run.wait();
            assert_eq!(fs::read(&exclude).unwrap(), as_it_was);
            assert_eq!(identity(&exclude), identity(&link));
            assert_eq!(fs::read(&link).unwrap(), as_it_was);
            assert_eq!(mode(&exclude), 0o640);
            for left in entries(&private).difference(&private_before) {
                let name = left.to_string_lossy();
                assert!(
                    name.starts_with("git-dupe-") && name.ends_with(".new"),
                    "{name} was left in the private Git directory"
                );
            }
            assert!(
                lock_is_free(&common),
                "the killed status left the lock held"
            );
        }

        let status = s.git(["dupe", "status"]).from(&dir).start();
        let status = status.wait_within(LIMIT);
        assert_eq!(status.end, End::Code(0), "{status:?}");
        let replaced = [
            &user[..],
            b"# BEGIN git-dupe\n/.gitdupe\n/notes\n# END git-dupe\n",
        ]
        .concat();
        assert_eq!(fs::read(&exclude).unwrap(), replaced);
        assert_eq!(mode(&exclude), 0o640);
    });
}

/// Runs `git dupe <words>` in `dir` with every write of git-dupe's own failing, through
/// a control directory of its own named `name`.
fn writes_failing(s: &Scenario, name: &str, dir: &Path, words: &[&str]) -> Output {
    s.killing(name, words).writes_failing(dir)
}

#[test]
fn a_write_that_fails_leaves_the_file_and_the_private_repository_and_is_named() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let common = dir.join(".git");
        let exclude = common.join("info/exclude");
        fs::write(&exclude, b"*.o\n").unwrap();
        s.git(["dupe", "status"]).from(&dir).succeeds();
        fs::set_permissions(&exclude, fs::Permissions::from_mode(0o640)).unwrap();
        write(&dir, ".gitdupe", b"scratch\n");
        let as_it_was = fs::read(&exclude).unwrap();
        let file = identity(&exclude);
        let private = common.join("dupe");
        let private_before = entries(&private);
        let not_written = |output: &Output| {
            let named = [
                b"cannot replace the managed region of ",
                exclude.as_os_str().as_encoded_bytes(),
                b": ",
            ]
            .concat();
            let warnings = output.lines("warning");
            assert!(
                warnings.iter().any(|line| line.starts_with(&named)
                    && line.ends_with(b"; private files may be visible to public Git")),
                "{output:?}"
            );
            // The exposure question was still asked: the new hidden path is not ignored.
            assert!(
                warnings.contains(&&b"scratch is hidden but public Git does not ignore it"[..]),
                "{output:?}"
            );
            assert_eq!(fs::read(&exclude).unwrap(), as_it_was);
            assert_eq!(identity(&exclude), file);
            assert_eq!(mode(&exclude), 0o640);
            assert_eq!(entries(&private), private_before);
        };

        // A command that succeeds keeps its status.
        let status = writes_failing(s, "status", &dir, &["status"]);
        assert_eq!(status.end, End::Code(0), "{status:?}");
        not_written(&status);
        // A command that fails keeps Git's.
        let words = ["rev-parse", "--verify", "-q", "refs/heads/nothing"];
        let failed = writes_failing(s, "rev-parse", &dir, &words);
        let gits = s.private(&dir).git(words).run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(failed.end, gits.end, "{failed:?}");
        not_written(&failed);

        // `detach` refuses, and the region and the private repository stand.
        let detach = writes_failing(s, "detach", &dir, &["detach", "--force"]);
        assert_eq!(detach.end, End::Code(128), "{detach:?}");
        let named = [
            b"cannot delete the managed region of ",
            exclude.as_os_str().as_encoded_bytes(),
            b": ",
        ]
        .concat();
        let fatal = detach.only_line("fatal");
        assert!(
            fatal.starts_with(&named) && fatal.ends_with(b"; nothing is detached"),
            "{detach:?}"
        );
        assert_eq!(fs::read(&exclude).unwrap(), as_it_was);
        assert_eq!(identity(&exclude), file);
        assert!(private.join("HEAD").is_file());
        assert_eq!(entries(&private), private_before);

        // Then every command writes the region as it would have.
        s.git(["dupe", "status"]).from(&dir).succeeds();
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/scratch"]);
        assert_eq!(mode(&exclude), 0o640);
        s.git(["dupe", "hide", "notes"]).from(&dir).succeeds();
        assert_eq!(
            region_rules(&dir),
            [&b"/.gitdupe"[..], b"/notes", b"/scratch"]
        );
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/scratch"]);
        write(&dir, "scratch/plan.md", b"plan\n");
        s.git(["dupe", "add", "scratch/"]).from(&dir).succeeds();
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/scratch"]);
        s.git(["dupe", "detach", "--force"]).from(&dir).succeeds();
        assert_eq!(region(&dir), None);
        assert_eq!(fs::read(&exclude).unwrap(), b"*.o\n");
        assert!(!private.exists());
    });
}
