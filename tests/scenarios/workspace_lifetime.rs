//! A workspace's state as one command leaves it and later commands consume it, on the
//! second machine: the workspace `clone` attached is cleaned, moved, and completed by
//! `init`; `pull` brings it a `.gitdupe` that hides one directory more and one less; then
//! `detach` leaves it without `--force`, the remote-tracking state `clone` made deciding,
//! and `init` attaches it again from the `.gitdupe` left on disk (F2, G1, G2, G3, G7, G9,
//! G16). Each command has scenarios of its own; here each meets what the one before it
//! left.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, changed_since, region, region_rules, unchanged,
    under_each_release, warnings_in_any_order, write,
};

/// `git dupe <words>` from `dir`, which must exit 0.
fn dupe(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let output = s.git(["dupe"].iter().chain(words)).from(dir).run();
    assert_eq!(output.end, End::Code(0), "git dupe {words:?}: {output:?}");
    output
}

/// Every file public Git lists as untracked in the workspace at `dir`.
fn publicly_untracked(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git(["status", "--porcelain", "--untracked-files=all"])
        .from(dir)
        .succeeds()
        .stdout
}

#[test]
fn a_second_machine_attached_by_clone_is_moved_completed_pulled_and_left() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let m = s.second_machine("project");
        let first = &m.first.root;
        let url = m.first.private_remote.to_str().unwrap();
        dupe(s, &m.root, &["clone", url]);

        // `clean` spares what the `.gitdupe` that `clone` brought hides (G16).
        write(&m.root, "scratch.txt", b"scratch\n");
        write(&m.root, "notes/scratch.md", b"mine\n");
        let before = Tree::working(&m.root);
        dupe(s, &m.root, &["clean", "-fdx"]);
        assert_eq!(changed_since(&before, &m.root), ["scratch.txt"]);

        // Moved, it stays attached, and plain Git reads its private history (F2).
        let second = s.dir().join("moved");
        fs::rename(&m.root, &second).unwrap();
        let status = dupe(s, &second, &["status", "--porcelain"]);
        assert_eq!(status.stdout, b"?? notes/scratch.md\n", "{status:?}");
        let private = second.join(".git/dupe");
        let words = [
            OsStr::new("--git-dir"),
            private.as_os_str(),
            OsStr::new("log"),
            OsStr::new("-1"),
            OsStr::new("--format=%H"),
        ];
        let log = s.git(words).from(&second).succeeds();
        let head = s.private(first).git(["rev-parse", "HEAD"]).succeeds();
        assert_eq!(log.stdout, head.stdout);

        // `init` finds nothing a killed `init` left undone, says so, and changes nothing
        // in the private repository `clone` made (G1).
        let before = Tree::of(&private);
        let init = dupe(s, &second, &["init"]);
        assert_eq!(init.lines("hint").len(), 1, "{init:?}");
        unchanged(&before, &private);

        // The first machine hides a new directory and stops hiding `notes`, whose
        // privately tracked file stays hidden, and sends the change.
        dupe(s, first, &["hide", "newdir"]);
        dupe(s, first, &["unhide", "notes"]);
        dupe(s, first, &["commit", "-qm", "hidden paths"]);
        dupe(s, first, &["push", "-q"]);

        // On the second machine, through the tracking `clone` configured: once `pull`
        // has ended, a file already under the new hidden directory is ignored without
        // another command (G7), and `notes`, where an untracked file stands, is named as
        // visible (G9).
        write(&second, "newdir/x", b"new\n");
        assert_eq!(publicly_untracked(s, &second), b"?? newdir/x\n");
        let pulled = dupe(s, &second, &["pull", "-q"]);
        warnings_in_any_order(&pulled, &[&[b"notes", b"visible"]]);
        assert_eq!(publicly_untracked(s, &second), b"?? notes/scratch.md\n");

        // Every commit is on the remote `clone` added and nothing is uncommitted:
        // `detach` leaves without `--force`, every file kept, naming each formerly hidden
        // path public Git can now see (G3).
        let working = Tree::working(&second);
        let left = dupe(s, &second, &["detach"]);
        assert!(!private.exists());
        assert!(region(&second).is_none());
        assert!(changed_since(&working, &second).is_empty());
        warnings_in_any_order(
            &left,
            &[
                &[b".gitdupe"],
                &[b"newdir"],
                &[b"notes/a.md"],
                &[b"docs/notes.md"],
            ],
        );

        // Attached again, the `.gitdupe` left on disk hides its paths at once.
        dupe(s, &second, &["init"]);
        assert_eq!(
            region_rules(&second),
            [b"/.gitdupe".as_slice(), b"/.vscode", b"/newdir"]
        );
        assert_eq!(
            publicly_untracked(s, &second),
            b"?? docs/notes.md\n?? notes/a.md\n?? notes/scratch.md\n"
        );
    });
}
