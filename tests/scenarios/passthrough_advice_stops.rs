//! The hint after a private `rebase -i` that stops without a conflict (`Holds/G19`, S12):
//! at an `edit` or a `break`, which exits 0, at a reword whose editor failed, and at a todo
//! list Git cannot read, which exit 1, the rebase directory stands, and one `hint:` follows
//! Git's output whatever the exit, naming `git dupe commit --amend` and
//! `git dupe rebase --edit-todo` beside the conflict's commands; once the rebase has ended,
//! none follows. A `rebase` and an `am` share `rebase-apply`, each marking it with a file
//! of its own: beside the other's stop, neither draws its line. The stop is read by
//! `lstat`, so the hint adds no Git run (G22).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, STOPPED_REBASE_HINT as REBASE, Scenario, hinted, private_add, private_commit, run_traced,
    text, under_each_release, unhinted, write, write_executable,
};

/// An attached workspace whose private `main`, checked out, holds three commits of
/// `notes.txt`, `.gitdupe` committed in the first; the commit editor succeeds.
fn three_commits(s: &Scenario) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
        ("core.editor", "true"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let root = s.dir().join("workspace");
    s.repository(&root);
    s.init(&root);
    write(&root, ".gitdupe", b"notes.txt\n");
    private_add(s, &root, ".gitdupe");
    for content in ["one\n", "two\n", "three\n"] {
        write(&root, "notes.txt", content.as_bytes());
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
    }
    root
}

/// Makes the private sequence editor of the workspace at `root` the script `name`, which
/// rewrites the todo list it is given through `program`, a `sed` or `awk` program.
fn sequence_editor(s: &Scenario, root: &Path, name: &str, program: &str) {
    let script = s.dir().join(name);
    let body = format!("#!/bin/sh\n{program} \"$1\" > \"$1.new\" && mv \"$1.new\" \"$1\"\n");
    write_executable(s, &script, body.as_bytes());
    s.private(root)
        .git(["config", "sequence.editor", text(&script)])
        .succeeds();
}

#[test]
fn a_rebase_stopped_at_an_edit_or_a_break_ends_with_one_hint_after_exit_0() {
    under_each_release(|s| {
        let root = three_commits(s);
        sequence_editor(s, &root, "edit", "sed '1s/^pick /edit /'");
        let edit = hinted(s, &root, &["rebase", "-i", "HEAD~2"], &REBASE);
        assert_eq!(edit.end, End::Code(0), "{edit:?}");
        // Still stopped: a command that leaves the stop standing is followed by it too.
        let shown = hinted(s, &root, &["rebase", "--show-current-patch"], &REBASE);
        assert_eq!(shown.end, End::Code(0), "{shown:?}");
        let continued = unhinted(s, &root, &["rebase", "--continue"]);
        assert_eq!(continued.end, End::Code(0), "{continued:?}");

        sequence_editor(
            s,
            &root,
            "break",
            "awk '{ print } NR == 1 { print \"break\" }'",
        );
        let stopped = hinted(s, &root, &["rebase", "-i", "HEAD~2"], &REBASE);
        assert_eq!(stopped.end, End::Code(0), "{stopped:?}");
        let aborted = unhinted(s, &root, &["rebase", "--abort"]);
        assert_eq!(aborted.end, End::Code(0), "{aborted:?}");

        // The stop is read by `lstat`: no Git run between the rebase and settle's.
        sequence_editor(s, &root, "edit", "sed '1s/^pick /edit /'");
        let (traced, runs) = run_traced(
            s.git(["dupe", "rebase", "-i", "HEAD~2"]).from(&root),
            &s.dir().join("trace"),
        );
        assert_eq!(traced.end, End::Code(0), "{traced:?}");
        assert_eq!(
            runs.own().commands(),
            [
                &b"rev-parse"[..],
                b"config",
                b"rebase",
                b"ls-files",
                b"ls-files",
                b"check-ignore"
            ]
        );
        // Beside the stopped rebase, an `am` that applies its patch has stopped nothing: its
        // own stop keeps `rebase-apply`, not the rebase's `rebase-merge`.
        let patch = s.dir().join("main.patch");
        let made = s
            .private(&root)
            .git(["format-patch", "-1", "--stdout", "main"])
            .succeeds();
        fs::write(&patch, made.stdout).unwrap();
        let applied = unhinted(s, &root, &["am", text(&patch)]);
        assert_eq!(applied.end, End::Code(0), "{applied:?}");
        unhinted(s, &root, &["rebase", "--abort"]);
    });
}

#[test]
fn a_rebase_stopped_by_its_editor_or_its_todo_list_ends_with_one_hint_after_exit_1() {
    under_each_release(|s| {
        let root = three_commits(s);
        sequence_editor(s, &root, "reword", "sed '1s/^pick /reword /'");
        s.private(&root)
            .git(["config", "core.editor", "false"])
            .succeeds();
        let reword = hinted(s, &root, &["rebase", "-i", "HEAD~2"], &REBASE);
        assert_eq!(reword.end, End::Code(1), "{reword:?}");
        s.private(&root)
            .git(["config", "core.editor", "true"])
            .succeeds();
        unhinted(s, &root, &["rebase", "--abort"]);

        sequence_editor(s, &root, "unreadable", "sed '1s/^pick /pickk /'");
        let unreadable = hinted(s, &root, &["rebase", "-i", "HEAD~2"], &REBASE);
        assert_eq!(unreadable.end, End::Code(1), "{unreadable:?}");
        // Mended, the todo list leaves the rebase stopped until it goes on.
        sequence_editor(s, &root, "mend", "sed 's/^pickk /pick /'");
        let mended = hinted(s, &root, &["rebase", "--edit-todo"], &REBASE);
        assert_eq!(mended.end, End::Code(0), "{mended:?}");
        let continued = unhinted(s, &root, &["rebase", "--continue"]);
        assert_eq!(continued.end, End::Code(0), "{continued:?}");
    });
}

#[test]
fn a_stop_the_other_command_keeps_in_rebase_apply_draws_no_hint() {
    under_each_release(|s| {
        let root = three_commits(s);
        // A patch of the second commit no longer applies on the third: the `am` stops.
        let patch = s.dir().join("two.patch");
        let made = s
            .private(&root)
            .git(["format-patch", "-1", "--stdout", "HEAD~1"])
            .succeeds();
        fs::write(&patch, made.stdout).unwrap();
        let applying = s.private(&root).git(["am", text(&patch)]).run();
        assert_eq!(applying.end, End::Code(128), "{applying:?}");
        // Beside it Git refuses a rebase, whose `--continue`, `--skip`, and `--abort` would
        // be refused too.
        let refused = unhinted(s, &root, &["rebase", "HEAD~1"]);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        s.private(&root).git(["am", "--abort"]).succeeds();

        // A rebase of the last two commits onto another first stops on the conflict, in
        // `rebase-apply` under `--apply`: beside it, an `am` is no stopped `am`.
        s.private(&root)
            .git(["switch", "-q", "-c", "side", "HEAD~2"])
            .succeeds();
        write(&root, "notes.txt", b"side\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        s.private(&root).git(["switch", "-q", "main"]).succeeds();
        let stopped = hinted(s, &root, &["rebase", "--apply", "side"], &REBASE);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        let shown = unhinted(s, &root, &["am", "--show-current-patch"]);
        assert_eq!(shown.end, End::Code(0), "{shown:?}");
        unhinted(s, &root, &["rebase", "--abort"]);
    });
}
