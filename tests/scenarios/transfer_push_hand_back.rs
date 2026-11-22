//! The hint after a push into another worktree's private repository (`Holds/G18`, G25,
//! In use 11): a linked worktree attached as `git dupe help clone` shows tracks the branch
//! the main worktree has checked out, which a push given no refspec is refused, and the hint
//! names the push to a branch of another name there and the merge in that worktree, the
//! route it describes working as printed, the path typed in place of a remote included. The
//! hint naming `git dupe pull` still follows where the one URL the push sends to names no
//! other worktree's private repository, after a push given a refspec, and where a word may
//! move the repository Git sends to. Git's own run and output unchanged, then one `hint:`,
//! at no run of its own (G22).

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Worktree, committing, common_private_url, dupe_succeeds,
    gits_then_one_hint, holds, linked_on_branch, private_add, private_commit, text,
    transfer_asked_after, under_each_release, without_maintenance, write,
};

/// A main worktree with private history on `main`, and the linked worktree
/// `project-<name>` on the public branch `<name>`, attached from the main worktree's
/// private repository by the URL `git dupe help clone` shows, expanded by the shell from
/// there, and holding one private commit of its own on `main`.
fn attached_from_the_main_worktree(s: &Scenario, name: &str) -> (Worktree, Worktree) {
    let root = s.dir().join("project");
    s.attached_project(&root);
    for (path, bytes) in [
        ("notes/a.md", &b"private note\n"[..]),
        (".gitdupe", b"notes\n"),
    ] {
        write(&root, path, bytes);
        private_add(s, &root, path);
    }
    private_commit(s, &root);
    let main = Worktree::read(s, &root);
    let linked = s.dir().join(format!("project-{name}"));
    let wt = attached_from(s, &main.root, &linked, name, None);
    (main, wt)
}

/// Adds the linked worktree at `linked` on the public branch `branch` of the project at
/// `main`, attaches it by `git dupe clone <url>`, `common_private_url` there where
/// `url` is `None`, and commits a change of its own.
fn attached_from(
    s: &Scenario,
    main: &Path,
    linked: &Path,
    branch: &str,
    url: Option<&str>,
) -> Worktree {
    let wt = linked_on_branch(s, main, linked, branch);
    let url = url.map_or_else(|| common_private_url(s, &wt.root), str::to_owned);
    dupe_succeeds(s, &wt.root, &["clone", &url]);
    committing(s, &wt.root);
    write(
        &wt.root,
        "notes/a.md",
        format!("{branch} note\n").as_bytes(),
    );
    dupe_succeeds(s, &wt.root, &["add", "notes/a.md"]);
    dupe_succeeds(s, &wt.root, &["commit", "-q", "-m", branch]);
    wt
}

/// `git dupe <words>` in `wt` ends as `git dupe git <words>`, Git's own run guarded by
/// nothing, with every byte of its output, and then one `hint:` naming each of `parts`.
/// The push is refused, so that both runs start from the same state.
fn hinted_in(s: &Scenario, wt: &Worktree, words: &[&str], parts: &[&[u8]]) -> Output {
    let gits = s
        .git(["dupe", "git"].iter().chain(words))
        .from(&wt.root)
        .run();
    let ours = s.git(["dupe"].iter().chain(words)).from(&wt.root).run();
    gits_then_one_hint(&ours, &gits, parts);
    assert_eq!(ours.end, End::Code(1), "{words:?}: {ours:?}");
    ours
}

/// The hand-back hint for the remote `remote` and the worktree named `worktree`.
fn hand_back(remote: &str, worktree: &str) -> [Vec<u8>; 4] {
    [
        format!("where a private push to {remote}, the private repository of {worktree},"),
        format!("is refused because {worktree} has the branch checked out"),
        format!("'git dupe push {remote} HEAD:refs/heads/<name>'"),
        format!("'git dupe merge <name>', run in {worktree}, takes it"),
    ]
    .map(String::into_bytes)
}

fn parts(owned: &[Vec<u8>]) -> Vec<&[u8]> {
    owned.iter().map(Vec::as_slice).collect()
}

#[test]
fn a_push_refused_by_the_main_worktree_names_the_push_to_another_name_and_the_merge() {
    under_each_release(|s| {
        without_maintenance(s);
        let (main, wt) = attached_from_the_main_worktree(s, "agent");
        let expected = hand_back("origin", "the main worktree");
        for words in [&["push"][..], &["push", "origin"]] {
            let ours = hinted_in(s, &wt, words, &parts(&expected));
            assert!(!holds(&ours.stderr, b"git dupe pull"), "{ours:?}");
            // No question of its own: the runs are those of any rejected push (G22).
            transfer_asked_after(s, &wt.root, words, End::Code(1), true, &[]);
        }

        // The path itself in place of a remote: the hint names it.
        let url = common_private_url(s, &wt.root);
        hinted_in(
            s,
            &wt,
            &["push", &url],
            &parts(&hand_back(&url, "the main worktree")),
        );

        // The two commands it names, typed as written with a name that is no branch or tag
        // in the main worktree's private repository, bring the commit there.
        let commit = wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout;
        dupe_succeeds(s, &wt.root, &["push", "origin", "HEAD:refs/heads/task"]);
        dupe_succeeds(s, &main.root, &["merge", "task"]);
        assert_eq!(
            main.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout,
            commit
        );
        assert_eq!(
            fs::read(main.root.join("notes/a.md")).unwrap(),
            b"agent note\n"
        );
    });
}

#[test]
fn a_push_refused_by_a_linked_worktree_names_that_worktree() {
    under_each_release(|s| {
        without_maintenance(s);
        let (main, agent) = attached_from_the_main_worktree(s, "agent");
        // A second linked worktree attached from the first one's private repository, which
        // has `main` checked out.
        let url = format!("{}/dupe", agent.git_directory.to_str().unwrap());
        let review = s.dir().join("project-review");
        let wt = attached_from(s, &main.root, &review, "review", Some(&url));
        let name = agent.name().unwrap().to_str().unwrap();
        let expected = hand_back("origin", &format!("worktree {name}"));
        hinted_in(s, &wt, &["push"], &parts(&expected));
        let commit = wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout;
        dupe_succeeds(s, &wt.root, &["push", "origin", "HEAD:refs/heads/review"]);
        dupe_succeeds(s, &agent.root, &["merge", "review"]);
        assert_eq!(
            agent
                .private(s)
                .git(["rev-parse", "HEAD"])
                .succeeds()
                .stdout,
            commit
        );
    });
}

#[test]
fn the_pull_hint_still_follows_where_the_push_sends_to_no_other_worktree() {
    under_each_release(|s| {
        without_maintenance(s);
        let (main, wt) = attached_from_the_main_worktree(s, "agent");
        let url = common_private_url(s, &wt.root);
        // A bare repository whose `main` holds a commit the linked worktree lacks, so that
        // a push of the linked worktree's `main` there is rejected too.
        let elsewhere = s.dir().join("elsewhere.git");
        s.bare_repository(&elsewhere);
        s.git(["config", "maintenance.auto", "false"])
            .from(&elsewhere)
            .succeeds();
        committing(s, &main.root);
        write(&main.root, "notes/b.md", b"main moved on\n");
        dupe_succeeds(s, &main.root, &["add", "notes/b.md"]);
        dupe_succeeds(s, &main.root, &["commit", "-q", "-m", "moved on"]);
        dupe_succeeds(s, &main.root, &["push", "-q", text(&elsewhere), "main"]);

        let rejected: [&[u8]; 2] = [b"'git dupe pull'", b"'git dupe fetch'"];
        let set = |words: &[&str]| dupe_succeeds(s, &wt.root, words);
        // The fetch URL alone names the main worktree's private repository: the push goes
        // to the push URL.
        set(&["config", "remote.origin.pushurl", text(&elsewhere)]);
        hinted_in(s, &wt, &["push"], &rejected);
        transfer_asked_after(s, &wt.root, &["push"], End::Code(1), true, &[]);
        // Two push URLs, the main worktree's private repository the first: Git sends to
        // each, so there is no one destination for the hint to name.
        set(&["config", "--unset-all", "remote.origin.pushurl"]);
        for pushed in [url.as_str(), text(&elsewhere)] {
            set(&["config", "--add", "remote.origin.pushurl", pushed]);
        }
        hinted_in(s, &wt, &["push"], &rejected);
        set(&["config", "--unset-all", "remote.origin.pushurl"]);
        // An ordinary repository on disk, given as the remote.
        set(&["remote", "add", "backup", text(&elsewhere)]);
        hinted_in(
            s,
            &wt,
            &["push", "backup"],
            &[
                b"'git dupe pull backup <branch>'",
                b"'git dupe fetch backup'",
            ],
        );
        // A refspec, whose source the hint's `HEAD` would not be.
        hinted_in(
            s,
            &wt,
            &["push", "origin", "main"],
            &[
                b"'git dupe pull origin <branch>'",
                b"'git dupe fetch origin'",
            ],
        );
        // Words that may move the repository Git takes from the one read: a bundle of
        // options, presumed to take `backup` as a value, and an abbreviation of `--repo`.
        for words in [&["push", "-qv", "backup"][..], &["push", "--rep=backup"]] {
            hinted_in(s, &wt, words, &rejected);
        }
        // Back to the URL alone, the hand-back hint again.
        hinted_in(
            s,
            &wt,
            &["push"],
            &parts(&hand_back("origin", "the main worktree")),
        );
    });
}
