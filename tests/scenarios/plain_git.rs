//! Plain Git can remove or overwrite private files; private restore recovers staged or
//! committed content, but never an edit that was not staged (the product's accepted limits).

use std::ffi::OsStr;
use std::fs;

use crate::harness::{Scenario, private_add, private_commit, under_each_release, write};

#[test]
fn stash_all_removes_private_files_and_root_restore_recovers_staged_content() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"notes\n");
        for path in [".env.local", "notes/a.md"] {
            write(&dir, path, b"committed\n");
            private_add(s, &dir, path);
        }
        private_add(s, &dir, ".gitdupe");
        private_commit(s, &dir);
        for path in [".env.local", "notes/a.md"] {
            write(&dir, path, b"staged\n");
            private_add(s, &dir, path);
            write(&dir, path, b"never staged\n");
        }
        s.git(["dupe", "status"]).from(&dir).succeeds();

        s.git(["stash", "-a"]).from(&dir).succeeds();
        for path in [".env.local", "notes/a.md"] {
            assert!(!dir.join(path).exists(), "{path} survived stash -a");
            assert_eq!(
                s.git(["show", &format!("stash^3:{path}")])
                    .from(&dir)
                    .succeeds()
                    .stdout,
                b"never staged\n",
                "{path} was not stashed"
            );
        }
        s.git(["dupe", "restore", "."]).from(&dir).succeeds();
        for path in [".env.local", "notes/a.md"] {
            assert_eq!(fs::read(dir.join(path)).unwrap(), b"staged\n", "{path}");
        }
    });
}

/// A teammate's public commit of `.env.local`, privately tracked here, pulled with plain
/// `git pull`, then `git dupe restore`: the private edit is committed, `stage_edit` stages
/// one after the commit, `unstaged_edit` leaves one never staged.
fn pull_then_restore(s: &Scenario, stage_edit: bool, unstaged_edit: bool) {
    let dir = s.dir().join("project");
    s.attached_project(&dir);
    let teammate = s.dir().join("teammate");
    s.git([
        OsStr::new("clone"),
        OsStr::new("-q"),
        dir.as_os_str(),
        teammate.as_os_str(),
    ])
    .succeeds();
    write(&teammate, ".env.local", b"public\n");
    s.git(["add", "-f", "--", ".env.local"])
        .from(&teammate)
        .succeeds();
    s.commit_public(&teammate);

    write(&dir, ".env.local", b"committed\n");
    private_add(s, &dir, ".env.local");
    private_commit(s, &dir);
    if stage_edit {
        write(&dir, ".env.local", b"staged\n");
        private_add(s, &dir, ".env.local");
    }
    if unstaged_edit {
        write(&dir, ".env.local", b"never staged\n");
    }
    s.git(["dupe", "status"]).from(&dir).succeeds();

    s.git([
        OsStr::new("-c"),
        OsStr::new("maintenance.auto=false"),
        OsStr::new("pull"),
        OsStr::new("-q"),
        OsStr::new("--ff-only"),
        teammate.as_os_str(),
        OsStr::new("main"),
    ])
    .from(&dir)
    .succeeds();
    assert_eq!(fs::read(dir.join(".env.local")).unwrap(), b"public\n");

    s.git(["dupe", "restore", "--", ".env.local"])
        .from(&dir)
        .succeeds();
    let expected: &[u8] = if stage_edit {
        b"staged\n"
    } else {
        b"committed\n"
    };
    assert_eq!(fs::read(dir.join(".env.local")).unwrap(), expected);
    assert_eq!(
        s.private(&dir)
            .git(["show", ":.env.local"])
            .succeeds()
            .stdout,
        expected
    );
    assert_eq!(
        s.git(["show", ":.env.local"]).from(&dir).succeeds().stdout,
        b"public\n"
    );
}

#[test]
fn pull_over_private_file_then_restore_recovers_committed_content() {
    under_each_release(|s| {
        pull_then_restore(s, false, false);
    });
}

#[test]
fn pull_over_private_file_then_restore_recovers_edit_staged_after_commit() {
    under_each_release(|s| {
        pull_then_restore(s, true, true);
    });
}

#[test]
fn pull_over_private_file_then_restore_loses_edit_never_staged() {
    under_each_release(|s| {
        pull_then_restore(s, false, true);
    });
}

#[test]
fn checkout_back_deletes_private_file_and_restore_recovers_private_content() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        s.git(["switch", "-q", "-c", "teammate"])
            .from(&dir)
            .succeeds();
        write(&dir, "conf.toml", b"public\n");
        s.git(["add", "conf.toml"]).from(&dir).succeeds();
        s.commit_public(&dir);
        s.git(["switch", "-q", "main"]).from(&dir).succeeds();
        write(&dir, "conf.toml", b"private\n");
        private_add(s, &dir, "conf.toml");
        private_commit(s, &dir);
        s.git(["dupe", "status"]).from(&dir).succeeds();

        s.git(["checkout", "-q", "teammate"]).from(&dir).succeeds();
        assert_eq!(fs::read(dir.join("conf.toml")).unwrap(), b"public\n");
        s.git(["checkout", "-q", "main"]).from(&dir).succeeds();
        assert!(!dir.join("conf.toml").exists());
        s.git(["dupe", "restore", "--", "conf.toml"])
            .from(&dir)
            .succeeds();
        assert_eq!(fs::read(dir.join("conf.toml")).unwrap(), b"private\n");
    });
}
