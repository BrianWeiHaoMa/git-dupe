//! The order before `git worktree remove` that the manual page's WORKTREES section gives
//! (the product's fourth accepted limit, G3, G27, In use 11), followed as written in a
//! linked worktree attached from the main worktree's private repository: each step shows
//! the reader the work it is for, and after the steps every kind of private work that
//! `git worktree remove` would delete without a word is in the main worktree's private
//! repository, so that removing the worktree deletes nothing that exists nowhere else.

use std::fs;

use crate::harness::{
    End, Output, Scenario, Worktree, committing, common_private_url, dupe_succeeds, holds,
    linked_on_branch, private_add, private_commit, under_each_release, without_maintenance, write,
};

/// The main worktree, hiding `notes`, and a linked worktree attached from its private
/// repository holding every kind of work no other copy holds: a new file in no commit, a
/// file the project ignores, a stash entry with a staged version, a commit on a second
/// private branch, a commit only an annotated tag names, and a file under `build`, which the main worktree alone hides and the
/// project's `.gitignore` ignores.
fn worktree_with_unsaved_work(s: &Scenario) -> (Worktree, Worktree) {
    let root = s.dir().join("project");
    s.attached_project(&root);
    for (path, bytes) in [("notes/a.md", &b"first\n"[..]), (".gitdupe", b"notes\n")] {
        write(&root, path, bytes);
        private_add(s, &root, path);
    }
    private_commit(s, &root);
    let main = Worktree::read(s, &root);
    let wt = linked_on_branch(s, &main.root, &s.dir().join("project-agent"), "agent");
    dupe_succeeds(s, &wt.root, &["clone", &common_private_url(s, &wt.root)]);
    committing(s, &wt.root);
    committing(s, &main.root);
    dupe_succeeds(s, &main.root, &["hide", "build"]);

    let here = &wt.root;
    dupe_succeeds(s, here, &["switch", "-q", "-c", "second"]);
    write(here, "notes/second.md", b"on a second branch\n");
    dupe_succeeds(s, here, &["add", "notes/second.md"]);
    dupe_succeeds(s, here, &["commit", "-q", "-m", "second"]);
    dupe_succeeds(s, here, &["switch", "-q", "--detach", "main"]);
    write(here, "notes/tagged.md", b"only a tag names it\n");
    dupe_succeeds(s, here, &["add", "notes/tagged.md"]);
    dupe_succeeds(s, here, &["commit", "-q", "-m", "tagged"]);
    dupe_succeeds(s, here, &["tag", "-a", "-m", "kept", "kept"]);
    dupe_succeeds(s, here, &["switch", "-q", "main"]);
    // A stash entry holding a staged version and a later one.
    write(here, "notes/a.md", b"staged draft\n");
    dupe_succeeds(s, here, &["add", "notes/a.md"]);
    write(here, "notes/a.md", b"stashed\n");
    dupe_succeeds(s, here, &["stash", "-q"]);
    write(here, "notes/new.md", b"in no commit\n");
    // The project's `.gitignore` ignores `build/` wherever it stands.
    write(here, "notes/build/trace.txt", b"ignored by the project\n");
    write(
        here,
        "build/idea.md",
        b"hidden by the main worktree alone\n",
    );
    (main, wt)
}

/// `git dupe <words>` in `wt`, which must exit 0.
fn dupe(s: &Scenario, wt: &Worktree, words: &[&str]) -> Output {
    dupe_succeeds(s, &wt.root, words)
}

#[test]
fn the_order_before_git_worktree_remove_leaves_no_private_work_only_there() {
    under_each_release(|s| {
        without_maintenance(s);
        let (main, wt) = worktree_with_unsaved_work(s);

        // 1. What no commit holds under the hidden paths, then add, with -f for `!!`, and
        // commit.
        let status = dupe(s, &wt, &["status", "--short", "-uall"]);
        assert!(holds(&status.stdout, b"?? notes/new.md\n"), "{status:?}");
        assert!(holds(&status.stdout, b"!! notes/build/"), "{status:?}");
        // 2. The file under a path the main worktree alone hides, which that status does
        // not list, is named by the warning; the project ignores it too, so -f adds it.
        assert!(!holds(&status.stdout, b" build/"), "{status:?}");
        let warning = status
            .lines("warning")
            .into_iter()
            .find(|line| line.starts_with(b"build stands here and is hidden by"))
            .unwrap_or_else(|| panic!("no warning for build: {status:?}"));
        assert!(holds(warning, b"the main worktree alone"), "{status:?}");
        dupe(s, &wt, &["add", "notes/new.md"]);
        dupe(s, &wt, &["add", "-f", "notes/build/trace.txt"]);
        dupe(s, &wt, &["commit", "-q", "-m", "files in no commit"]);
        let plain = s.git(["dupe", "add", "build"]).from(&wt.root).run();
        assert_eq!(
            plain.end,
            End::Code(1),
            "Git says the project ignores it: {plain:?}"
        );
        dupe(s, &wt, &["add", "-f", "build"]);
        dupe(s, &wt, &["commit", "-q", "-m", "build"]);
        let after = dupe(s, &wt, &["status", "--short", "-uall"]);
        assert!(after.stdout.is_empty(), "{after:?}");
        assert!(after.lines("warning").is_empty(), "{after:?}");

        // 3. The stash entries, which a push does not send: pop with the index, commit what
        // is staged, then the rest.
        let stashes = dupe(s, &wt, &["stash", "list"]);
        assert_eq!(
            stashes.stdout.split(|&b| b == b'\n').count(),
            2,
            "{stashes:?}"
        );
        dupe(s, &wt, &["stash", "pop", "-q", "--index"]);
        dupe(s, &wt, &["commit", "-q", "-m", "the staged draft"]);
        dupe(s, &wt, &["commit", "-q", "-a", "-m", "the stash"]);
        assert!(dupe(s, &wt, &["stash", "list"]).stdout.is_empty());

        // 4. The commits no remote-tracking branch holds, on HEAD, a branch, or a tag, then
        // the push of each: another branch by its name, and the tag's commit and the branch
        // the main worktree has checked out each to a branch of a new name there, the merge
        // in the main worktree taking the second.
        let unpushed: &[&str] = &[
            "log",
            "HEAD",
            "--branches",
            "--tags",
            "--not",
            "--remotes",
            "--format=%s",
        ];
        dupe(s, &wt, &["fetch", "-q", "--all", "--prune"]);
        let listed = dupe(s, &wt, unpushed);
        for subject in [
            &b"second\n"[..],
            b"tagged\n",
            b"files in no commit\n",
            b"build\n",
            b"the staged draft\n",
            b"the stash\n",
        ] {
            assert!(holds(&listed.stdout, subject), "{listed:?}");
        }
        dupe(s, &wt, &["push", "-q", "origin", "second"]);
        dupe(s, &wt, &["push", "-q", "origin", "kept~0:refs/heads/kept"]);
        dupe(
            s,
            &wt,
            &["push", "-q", "origin", "HEAD:refs/heads/handback"],
        );
        dupe_succeeds(s, &main.root, &["merge", "-q", "handback"]);
        dupe(s, &wt, &["fetch", "-q", "--all", "--prune"]);
        let left = dupe(s, &wt, unpushed);
        assert!(left.stdout.is_empty(), "{left:?}");

        // Every kind of work is in the main worktree's private repository.
        for (revision, bytes) in [
            ("second:notes/second.md", &b"on a second branch\n"[..]),
            ("HEAD:notes/new.md", b"in no commit\n"),
            ("HEAD:notes/build/trace.txt", b"ignored by the project\n"),
            ("HEAD:notes/a.md", b"stashed\n"),
            ("HEAD^:notes/a.md", b"staged draft\n"),
            ("HEAD:build/idea.md", b"hidden by the main worktree alone\n"),
            ("refs/heads/kept:notes/tagged.md", b"only a tag names it\n"),
        ] {
            let shown = main.private(s).git(["show", revision]).succeeds();
            assert_eq!(shown.stdout, bytes, "{revision}");
        }

        // So `git worktree remove`, which the private files on disk do not stop, deletes
        // nothing that exists nowhere else.
        s.git(["worktree", "remove", wt.root.to_str().unwrap()])
            .from(&main.root)
            .succeeds();
        assert!(!wt.root.exists());
        assert!(!wt.git_directory.exists());
        assert_eq!(
            fs::read(main.root.join("notes/new.md")).unwrap(),
            b"in no commit\n"
        );
    });
}
