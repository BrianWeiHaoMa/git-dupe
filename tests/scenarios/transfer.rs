//! Push, pull and fetch guard public words and every configured private URL before
//! running Git against private history (G18–G20). No refused URL is contacted.

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, PROJECT_URL, Scenario, Transfer, Tree, holds, names, region, under_each_release,
};

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 scenario path")
}

fn refusal(output: &Output, route: &str, place: &str) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, route.as_bytes());
    names(line, place.as_bytes());
}

fn word_refused(s: &Scenario, t: &Transfer, from: &Path, words: &[&str], place: &str) {
    let before = t.everything();
    let output = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(from)
        .run();
    refusal(&output, "git dupe git", place);
    before.unchanged();
}

fn configured_refused(s: &Scenario, t: &Transfer, words: &[&str], remote: &str, place: &str) {
    let before = t.everything();
    let output = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(&t.root)
        .run();
    refusal(&output, "git dupe remote remove", place);
    names(
        output.only_line("fatal"),
        format!("private remote '{remote}'").as_bytes(),
    );
    before.unchanged();
}

fn add_origin(s: &Scenario, t: &Transfer) {
    s.git(["dupe", "remote", "add", "origin", text(&t.private_remote)])
        .from(&t.root)
        .succeeds();
}

fn arrived(s: &Scenario, t: &Transfer, destination: &Path) {
    let head = s.private(&t.root).git(["rev-parse", "HEAD"]).succeeds();
    let remote = s
        .git(["rev-parse", "refs/heads/main"])
        .from(destination)
        .succeeds();
    assert_eq!(head.stdout, remote.stdout);
}

/// Schemes, roots, linked worktrees, Git entries, file URLs and relative words are refused.
#[test]
fn public_words_are_refused_without_effects() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let sub = t.root.join("sub");
        fs::create_dir(&sub).unwrap();
        let dot_git = t.root.join(".git");
        let linked_url = format!("file://{}", t.linked.display());
        let relative = format!("../{}", t.root.file_name().unwrap().to_str().unwrap());
        for (words, from, place) in [
            (
                vec!["push", "http://example.com/team/project.git", "main:leak"],
                &t.root,
                "origin",
            ),
            (
                vec!["push", "--repo=git@example.com:team/project", "main:leak"],
                &t.root,
                "origin",
            ),
            (vec!["push", ".", "main:leak"], &sub, text(&t.root)),
            (vec!["fetch", text(&t.root)], &t.root, text(&t.root)),
            (
                vec!["push", text(&t.linked), "main:leak"],
                &t.root,
                text(&t.linked),
            ),
            (vec!["pull", text(&dot_git)], &t.root, text(&dot_git)),
            (vec!["fetch", &linked_url], &t.root, text(&t.linked)),
            (vec!["pull", &relative], &t.root, text(&t.root)),
            (
                vec!["remote", "add", "origin", PROJECT_URL],
                &t.root,
                "origin",
            ),
        ] {
            word_refused(s, &t, from, &words, place);
        }
    });
}

/// A transfer refusal still repairs the managed region, without changing anything else.
#[test]
fn refused_push_settles_a_stale_region() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let exclude = t.root.join(".git/info/exclude");
        let settled = fs::read(&exclude).unwrap();
        let region_before = region(&t.root).unwrap();
        fs::write(
            &exclude,
            [region_before.before, region_before.after].concat(),
        )
        .unwrap();
        assert!(region(&t.root).is_none());
        let before = Tree::of(&t.root);
        let others = [&t.linked, &t.public_remote, &t.private_remote].map(|p| Tree::of(p));
        let output = s
            .git(["dupe", "push", text(&t.root), "main:leak"])
            .from(&t.root)
            .run();
        refusal(&output, "git dupe git", text(&t.root));
        assert_eq!(fs::read(&exclude).unwrap(), settled);
        assert_eq!(before.changed_in(&Tree::of(&t.root)), [exclude]);
        for (before, place) in others
            .iter()
            .zip([&t.linked, &t.public_remote, &t.private_remote])
        {
            assert!(before.changed_in(&Tree::of(place)).is_empty());
        }
    });
}

/// A configured public URL is checked first, even when the words name a public root.
#[test]
fn configured_url_precedes_words_and_removal_restores_push() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        add_origin(s, &t);
        s.git([
            "dupe",
            "git",
            "remote",
            "add",
            "leak",
            "https://example.com/team/project",
        ])
        .from(&t.root)
        .succeeds();
        for words in [
            &["push"][..],
            &["push", "origin", "main"],
            &["pull"],
            &["fetch", "origin"],
            &["fetch", text(&t.root)],
        ] {
            configured_refused(s, &t, words, "leak", "origin");
        }
        let before = t.everything();
        let output = s.git(["dupe", "fetch", text(&t.root)]).from(&t.root).run();
        refusal(&output, "git dupe remote remove", "leak");
        assert!(!holds(output.only_line("fatal"), text(&t.root).as_bytes()));
        before.unchanged();
        s.git(["dupe", "remote", "remove", "leak"])
            .from(&t.root)
            .succeeds();
        s.git(["dupe", "push", "origin", "main"])
            .from(&t.root)
            .succeeds();
        arrived(s, &t, &t.private_remote);
    });
}

/// Push URLs and earlier URL records cannot be hidden behind a safe private URL.
#[test]
fn pushurl_and_every_url_record_are_guarded() {
    under_each_release(|s| {
        for shape in ["pushurl", "two-urls"] {
            let t = s.transfer_workspace(shape);
            add_origin(s, &t);
            if shape == "pushurl" {
                s.git(["dupe", "git", "config", "remote.leak.pushurl", PROJECT_URL])
                    .from(&t.root)
                    .succeeds();
            } else {
                s.git([
                    "dupe",
                    "git",
                    "config",
                    "--add",
                    "remote.leak.url",
                    PROJECT_URL,
                ])
                .from(&t.root)
                .succeeds();
                s.git([
                    "dupe",
                    "git",
                    "config",
                    "--add",
                    "remote.leak.url",
                    text(&t.private_remote),
                ])
                .from(&t.root)
                .succeeds();
            }
            for words in [
                &["push"][..],
                &["push", "origin", "main"],
                &["pull"],
                &["fetch", "origin"],
                &["fetch"],
                &["fetch", text(&t.root)],
            ] {
                configured_refused(s, &t, words, "leak", "origin");
            }
            s.git(["dupe", "remote", "remove", "leak"])
                .from(&t.root)
                .succeeds();
            s.git(["dupe", "push", "origin", "main"])
                .from(&t.root)
                .succeeds();
            arrived(s, &t, &t.private_remote);
        }
    });
}

/// Git's command-line configuration participates in the private URL check.
#[test]
fn command_line_remote_url_is_guarded() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        add_origin(s, &t);
        let setting = format!("remote.extra.url={}", t.root.display());
        let before = t.everything();
        let output = s
            .git(["-c", &setting, "dupe", "fetch", "origin"])
            .from(&t.root)
            .run();
        refusal(&output, "git dupe remote remove", text(&t.root));
        names(output.only_line("fatal"), b"private remote 'extra'");
        before.unchanged();
    });
}

/// An empty configured URL denotes the public root and is refused for all transfers.
#[test]
fn empty_remote_url_is_guarded() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        s.git(["dupe", "git", "config", "remote.empty.url", ""])
            .from(&t.root)
            .succeeds();
        for command in ["push", "pull", "fetch"] {
            configured_refused(s, &t, &[command], "empty", text(&t.root));
        }
    });
}

/// The ordinary private transfer sequence preserves Git's complete repeatable answers.
#[test]
fn private_transfers_keep_gits_answers_and_deliver_history() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let words = ["remote", "add", "origin", text(&t.private_remote)];
        let expected = s
            .private(&t.root)
            .git(["-c", "help.autocorrect=0"].into_iter().chain(words))
            .succeeds();
        s.private(&t.root)
            .git(["remote", "remove", "origin"])
            .succeeds();
        let ours = s
            .git(["dupe"].into_iter().chain(words))
            .from(&t.root)
            .succeeds();
        assert_eq!(ours, expected);
        let expected = s
            .private(&t.root)
            .git(["-c", "help.autocorrect=0", "push", "-u", "origin", "main"])
            .succeeds();
        // Restore the branch and upstream state so both initial pushes see no main ref.
        s.git(["update-ref", "-d", "refs/heads/main"])
            .from(&t.private_remote)
            .succeeds();
        for key in ["branch.main.remote", "branch.main.merge"] {
            s.private(&t.root)
                .git(["config", "--unset", key])
                .succeeds();
        }
        let ours = s
            .git(["dupe", "push", "-u", "origin", "main"])
            .from(&t.root)
            .succeeds();
        assert_eq!(ours, expected);
        arrived(s, &t, &t.private_remote);
        for words in [
            &["push", "-u", "origin", "main"][..],
            &["fetch"],
            &["pull"],
            &["remote", "-v"],
        ] {
            let expected = s
                .private(&t.root)
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .run();
            let ours = s
                .git(["dupe"].into_iter().chain(words.iter().copied()))
                .from(&t.root)
                .run();
            assert_eq!(ours, expected, "{words:?}");
            assert_eq!(ours.end, End::Code(0), "{ours:?}");
        }
        arrived(s, &t, &t.private_remote);
    });
}

/// Paths that merely start like a public place remain valid destinations.
#[test]
fn local_destination_neighbors_receive_private_history() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let root_neighbor = s.dir().join("workspace-mine.git");
        let remote_neighbor = s.dir().join("workspace-public-x.git");
        for destination in [&t.private_remote, &root_neighbor, &remote_neighbor] {
            if destination != &t.private_remote {
                s.bare_repository(destination);
            }
            s.git(["dupe", "push", text(destination), "main"])
                .from(&t.root)
                .succeeds();
            arrived(s, &t, destination);
        }
    });
}

/// Aliases reaching push are guarded; the explicit git route can deliberately leak history.
#[test]
fn push_alias_is_guarded_and_git_push_is_unguarded() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        s.git(["dupe", "git", "config", "alias.p", "push"])
            .from(&t.root)
            .succeeds();
        for command in ["push", "p"] {
            word_refused(
                s,
                &t,
                &t.root,
                &[command, text(&t.root), "main:leak"],
                text(&t.root),
            );
        }
        s.git(["dupe", "git", "push", text(&t.root), "main:leak"])
            .from(&t.root)
            .succeeds();
        let head = s.private(&t.root).git(["rev-parse", "HEAD"]).succeeds();
        let leaked = s
            .git(["rev-parse", "refs/heads/leak"])
            .from(&t.root)
            .succeeds();
        assert_eq!(head.stdout, leaked.stdout);
    });
}
