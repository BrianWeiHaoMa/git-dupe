//! A linked worktree's state as one command leaves it and later commands consume it,
//! beside an attached main worktree whose region applies there too: the linked worktree
//! `clone` attached is cleaned, moved with `git worktree move` and completed by `init`;
//! `pull` brings it a `.gitdupe` that hides one directory more and one less, which the
//! main worktree already hides and no longer hides; then `detach`, refused while a commit
//! or a stash entry is on no remote, leaves it without `--force` once none is, naming
//! what the main worktree's region still hides there, and `init` attaches it again from
//! the `.gitdupe` left on disk, naming what only the main worktree hides (F2, G1, G2, G3,
//! G7, G9, G16, G27). Throughout, no command in the linked worktree changes the main
//! worktree's region. `workspace_lifetime` is the same story on a second machine.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, Worktree, changed_since, names, unchanged, under_each_release,
    warnings_in_any_order, write,
};

/// `git dupe <words>` from `dir`, which must exit 0.
fn dupe(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let output = s.git(["dupe"].iter().chain(words)).from(dir).run();
    assert_eq!(output.end, End::Code(0), "git dupe {words:?}: {output:?}");
    output
}

/// Every file public Git lists as untracked in the worktree at `dir`.
fn publicly_untracked(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git(["status", "--porcelain", "--untracked-files=all"])
        .from(dir)
        .succeeds()
        .stdout
}

/// The root that a private repository's `core.worktree`, read by plain Git, names.
fn recorded_root(s: &Scenario, wt: &Worktree) -> PathBuf {
    let private = wt.private_directory();
    let config = private.join("config");
    let words = [
        OsStr::new("config"),
        OsStr::new("--file"),
        config.as_os_str(),
        OsStr::new("core.worktree"),
    ];
    let value = s.git(words).succeeds().stdout;
    let value = value.strip_suffix(b"\n").unwrap_or(&value);
    private
        .join(OsStr::from_bytes(value))
        .canonicalize()
        .unwrap()
}

#[test]
fn a_linked_worktree_attached_by_clone_is_moved_completed_pulled_and_left() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let first = s.pushed_workspace("project");
        s.git(["symbolic-ref", "HEAD", "refs/heads/main"])
            .from(&first.private_remote)
            .succeeds();
        let main = Worktree::read(s, &first.root);
        let url = first.private_remote.to_str().unwrap();
        // What the main worktree hides, this worktree hides too once `clone` has brought
        // the same `.gitdupe` and files: nothing is named as hidden elsewhere (G27).
        let cloned = dupe(s, &first.linked, &["clone", url]);
        assert!(cloned.lines("warning").is_empty(), "{cloned:?}");
        let main_region = main.region_bytes();

        // `clean` spares what the `.gitdupe` that `clone` brought hides (G16).
        write(&first.linked, "scratch.txt", b"scratch\n");
        write(&first.linked, "notes/scratch.md", b"mine\n");
        let before = Tree::working(&first.linked);
        dupe(s, &first.linked, &["clean", "-fdx"]);
        assert_eq!(changed_since(&before, &first.linked), ["scratch.txt"]);
        assert_eq!(main.region_bytes(), main_region);

        // Moved, it stays attached under the same Git directory (F2).
        let moved = s.dir().join("moved");
        let words = [
            "worktree",
            "move",
            first.linked.to_str().unwrap(),
            moved.to_str().unwrap(),
        ];
        s.git(words).from(&first.root).succeeds();
        let wt = Worktree::read(s, &moved);
        let status = dupe(s, &moved, &["status", "--porcelain"]);
        assert_eq!(status.stdout, b"?? notes/scratch.md\n", "{status:?}");

        // `init` records the moved root for plain Git, says so, and leaves the history
        // `clone` made, which plain Git then reads (F2, G1).
        let refs = wt.private(s).git(["for-each-ref"]).succeeds().stdout;
        let init = dupe(s, &moved, &["init"]);
        assert_eq!(init.lines("hint").len(), 1, "{init:?}");
        assert_eq!(recorded_root(s, &wt), moved.canonicalize().unwrap());
        assert_eq!(wt.private(s).git(["for-each-ref"]).succeeds().stdout, refs);
        let private = wt.private_directory();
        let words = [
            OsStr::new("--git-dir"),
            private.as_os_str(),
            OsStr::new("log"),
            OsStr::new("-1"),
            OsStr::new("--format=%H"),
        ];
        let log = s.git(words).from(&moved).succeeds();
        let head = main.private(s).git(["rev-parse", "HEAD"]).succeeds();
        assert_eq!(log.stdout, head.stdout);
        assert_eq!(main.region_bytes(), main_region);

        // The main worktree hides a new directory and stops hiding `notes`, whose
        // privately tracked file stays hidden, and sends the change. Its region applies
        // here at once: a file under the new directory is invisible to public Git here,
        // and every command here names the directory as hidden by the main worktree
        // alone (G27).
        dupe(s, &first.root, &["hide", "newdir"]);
        dupe(s, &first.root, &["unhide", "notes"]);
        dupe(s, &first.root, &["commit", "-qm", "hidden paths"]);
        dupe(s, &first.root, &["push", "-q"]);
        let main_region = main.region_bytes();
        write(&moved, "newdir/x", b"new\n");
        assert_eq!(publicly_untracked(s, &moved), b"");
        let named = dupe(s, &moved, &["status"]);
        warnings_in_any_order(&named, &[&[b"newdir", b"the main worktree"]]);

        // Through the tracking `clone` configured: once `pull` has ended, the new
        // directory is hidden here too and no longer named (G7, G27), and `notes`, which
        // neither worktree hides any longer and where an untracked file stands, is named
        // as visible (G9).
        let pulled = dupe(s, &moved, &["pull", "-q"]);
        warnings_in_any_order(&pulled, &[&[b"notes", b"visible"]]);
        assert_eq!(publicly_untracked(s, &moved), b"?? notes/scratch.md\n");
        assert_eq!(main.region_bytes(), main_region);

        // A commit no remote-tracking branch holds, then a stash entry, each keeps
        // `detach` from leaving without `--force`, changing nothing (G3).
        for (unsafe_state, undo) in [
            (
                &["commit", "--allow-empty", "-qm", "local"][..],
                &["push", "-q"][..],
            ),
            (&["stash", "push", "-q"], &["stash", "drop", "-q"]),
        ] {
            if unsafe_state[0] == "stash" {
                write(&moved, ".env.local", b"changed\n");
            }
            dupe(s, &moved, unsafe_state);
            let private = Tree::of(&wt.private_directory());
            let exclude = fs::read(main.common_directory.join("info/exclude")).unwrap();
            let refused = s.git(["dupe", "detach"]).from(&moved).run();
            assert_eq!(refused.end, End::Code(128), "{refused:?}");
            names(refused.only_line("fatal"), b"git dupe detach --force");
            unchanged(&private, &wt.private_directory());
            assert_eq!(
                fs::read(main.common_directory.join("info/exclude")).unwrap(),
                exclude
            );
            dupe(s, &moved, undo);
        }

        // Every commit is on the remote `clone` added and nothing is uncommitted:
        // `detach` leaves without `--force`, every file kept, its own region gone and the
        // main worktree's untouched, naming each formerly hidden path the main worktree's
        // region still hides here (G3).
        let working = Tree::working(&moved);
        let left = dupe(s, &moved, &["detach"]);
        assert!(!wt.private_directory().exists());
        assert!(wt.region().is_none());
        assert!(changed_since(&working, &moved).is_empty());
        assert_eq!(main.region_bytes(), main_region);
        warnings_in_any_order(
            &left,
            &[
                &[b".env.local", b"the main worktree"],
                &[b".gitdupe", b"the main worktree"],
                &[b".vscode ", b"the main worktree"],
                &[b".vscode/settings.json", b"the main worktree"],
                &[b"docs/notes.md", b"the main worktree"],
                &[b"newdir", b"the main worktree"],
                &[b"notes/a.md", b"the main worktree"],
            ],
        );
        assert_eq!(publicly_untracked(s, &moved), b"?? notes/scratch.md\n");

        // Attached again, the `.gitdupe` left on disk hides its paths at once, and the
        // privately tracked files only the main worktree's region still hides here are
        // named as hidden by it alone (G27).
        let again = dupe(s, &moved, &["init"]);
        let rules = wt.region().expect("a region").rules;
        assert_eq!(rules, [b"/.gitdupe".as_slice(), b"/.vscode", b"/newdir"]);
        warnings_in_any_order(
            &again,
            &[
                &[b".env.local", b"the main worktree"],
                &[b"docs/notes.md", b"the main worktree"],
                &[b"notes/a.md", b"the main worktree"],
            ],
        );
        assert_eq!(publicly_untracked(s, &moved), b"?? notes/scratch.md\n");
        assert_eq!(main.region_bytes(), main_region);
    });
}
