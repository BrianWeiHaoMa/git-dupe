//! `git dupe remote`: run as Git's own against the private repository once no word of it
//! names a public place, and refused, configuring nothing, when one does (G18, G19).

use std::fs;
use std::io::Write;

use crate::harness::{
    End, Output, PROJECT_URL, Tree, lines_in_order, names, region, run_traced, under_each_release,
    warnings_in_any_order, write,
};

fn refused(output: &Output, place: &[u8]) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, b"git dupe git");
    names(line, place);
}

/// Adding and listing a private remote preserves the public remote's configuration.
#[test]
fn add_and_list_use_the_private_repository() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        let destination = transfer.private_remote.to_str().unwrap();
        s.git(["dupe", "remote", "add", "origin", destination])
            .from(root)
            .succeeds();
        let ours = s.git(["dupe", "remote", "-v"]).from(root).succeeds();
        let gits = s.private(root).git(["remote", "-v"]).succeeds();
        assert_eq!(ours, gits);
        let private = s
            .private(root)
            .git(["config", "--get", "remote.origin.url"])
            .succeeds();
        assert_eq!(private.stdout, format!("{destination}\n").as_bytes());
        let public = s
            .git(["config", "--get", "remote.origin.url"])
            .from(root)
            .succeeds();
        assert_eq!(public.stdout, format!("{PROJECT_URL}\n").as_bytes());
    });
}

/// Bare remote, usage, and invalid words keep Git's answer and restore the region anywhere below root.
#[test]
fn remote_words_keep_gits_answer_and_settle() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        let below = root.join("sub/dir");
        fs::create_dir_all(&below).unwrap();
        s.private(root)
            .git(["config", "alias.remote-fdx", "remote -fdx"])
            .succeeds();
        let exclude = root.join(".git/info/exclude");
        let settled = region(root).unwrap();
        let without_region = [settled.before, settled.after].concat();
        let before = Tree::of(root).without(&[&exclude]);
        for words in [
            &["remote"][..],
            &["remote", "-h"],
            &["remote", "-fdx"],
            &["remote-fdx"],
        ] {
            for directory in [root, &below] {
                fs::write(&exclude, &without_region).unwrap();
                assert!(region(root).is_none());
                let gits = s
                    .private(root)
                    .git(
                        ["-c", "help.autocorrect=0"]
                            .into_iter()
                            .chain(words.iter().copied()),
                    )
                    .from(directory)
                    .run();
                let ours = s
                    .git(["dupe"].into_iter().chain(words.iter().copied()))
                    .from(directory)
                    .run();
                assert_eq!(ours, gits, "{words:?} from {}", directory.display());
                assert!(region(root).is_some(), "{words:?}: settle wrote no region");
                assert!(
                    before
                        .changed_in(&Tree::of(root).without(&[&exclude]))
                        .is_empty()
                );
            }
        }
    });
}

/// A refused public destination refreshes a stale region and changes only the exclude file.
#[test]
fn a_refusal_settles_the_stale_region() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        fs::OpenOptions::new()
            .append(true)
            .open(root.join(".gitdupe"))
            .unwrap()
            .write_all(b"extra\n")
            .unwrap();
        let before = Tree::of(root);
        let linked = Tree::of(&transfer.linked);
        let public_remote = Tree::of(&transfer.public_remote);
        let private_remote = Tree::of(&transfer.private_remote);
        let output = s
            .git(["dupe", "remote", "add", "leak", root.to_str().unwrap()])
            .from(root)
            .run();
        refused(&output, root.to_str().unwrap().as_bytes());
        assert!(region(root).unwrap().rules.contains(&b"/extra".to_vec()));
        assert_eq!(
            before.changed_in(&Tree::of(root)),
            [root.join(".git/info/exclude")]
        );
        for (before, place) in [
            (linked, &transfer.linked),
            (public_remote, &transfer.public_remote),
            (private_remote, &transfer.private_remote),
        ] {
            assert!(before.changed_in(&Tree::of(place)).is_empty());
        }
    });
}

/// Settle's warnings follow the refusal's one `fatal:` line.
#[test]
fn settle_warns_after_the_refusal() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        write(
            root,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!notes\n",
        );
        let output = s
            .git(["dupe", "remote", "add", "leak", root.to_str().unwrap()])
            .from(root)
            .run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        lines_in_order(&output, "fatal", &[b"git dupe git"]);
        warnings_in_any_order(&output, &[&[b"notes", b".gitignore"], &[b"notes/a.md"]]);
    });
}

/// A private alias reaching remote add is guarded just like the typed command.
#[test]
fn a_private_alias_cannot_add_a_public_destination() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        s.private(root)
            .git(["config", "alias.r", "remote add"])
            .succeeds();
        for words in [
            vec!["dupe", "remote", "add", "leak", root.to_str().unwrap()],
            vec!["dupe", "r", "leak", root.to_str().unwrap()],
        ] {
            let before = transfer.everything();
            let output = s.git(words).from(root).run();
            refused(&output, root.to_str().unwrap().as_bytes());
            before.unchanged();
        }
    });
}

/// An alias's global options reach both public-place readers and allow a private destination.
#[test]
fn alias_options_reach_the_guard_runs() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        s.private(root)
            .git(["config", "alias.rc", "-c color.ui=never remote add"])
            .succeeds();
        let before = transfer.everything();
        let (output, runs) = run_traced(
            s.git(["dupe", "rc", "leak", root.to_str().unwrap()])
                .from(root),
            &s.dir().join("trace"),
        );
        refused(&output, root.to_str().unwrap().as_bytes());
        before.unchanged();
        let own = runs.own();
        for tail in [
            [
                "config",
                "-z",
                "--get-regexp",
                r"^remote\..*\.(url|pushurl)$",
            ],
            ["worktree", "list", "--porcelain", "-z"],
        ] {
            let tail = tail.map(|word| word.as_bytes().to_vec());
            let readers: Vec<_> = own
                .words()
                .iter()
                .filter(|words| words.ends_with(&tail))
                .collect();
            assert_eq!(readers.len(), 1, "{own:?}");
            assert!(
                readers[0].starts_with(&[b"-c".to_vec(), b"color.ui=never".to_vec()]),
                "{own:?}"
            );
        }
        let destination = transfer.private_remote.to_str().unwrap();
        s.git(["dupe", "rc", "near", destination])
            .from(root)
            .succeeds();
        let configured = s
            .private(root)
            .git(["config", "--get", "remote.near.url"])
            .succeeds();
        assert_eq!(configured.stdout, format!("{destination}\n").as_bytes());
    });
}

/// The explicit git route adds even a public destination without the remote guard.
#[test]
fn git_remote_add_is_unguarded() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        let destination = root.to_str().unwrap();
        s.git(["dupe", "git", "remote", "add", "leak", destination])
            .from(root)
            .succeeds();
        let configured = s
            .private(root)
            .git(["config", "--get", "remote.leak.url"])
            .succeeds();
        assert_eq!(configured.stdout, format!("{destination}\n").as_bytes());
    });
}

/// One and twenty public remotes require the same runs, with two readers before remote.
#[test]
fn the_git_run_sequence_does_not_follow_the_remote_count() {
    under_each_release(|s| {
        let mut sequences = Vec::new();
        for count in [1, 20] {
            let root = s.dir().join(format!("project-{count}"));
            s.attached_project(&root);
            for index in 0..count {
                let name = format!("r{index}");
                let url = format!("https://example.com/{name}.git");
                s.git(["remote", "add", &name, &url]).from(&root).succeeds();
            }
            let (output, runs) = run_traced(
                s.git(["dupe", "remote", "-v"]).from(&root),
                &s.dir().join("trace"),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            let own = runs.own();
            let commands = own.commands();
            let remote = commands.iter().position(|word| *word == b"remote").unwrap();
            assert_eq!(own.of("remote"), 1, "{own:?}");
            for command in [b"config".as_slice(), b"worktree"] {
                assert_eq!(
                    commands.iter().filter(|word| **word == command).count(),
                    1,
                    "{own:?}"
                );
                assert_eq!(
                    commands[..remote]
                        .iter()
                        .filter(|word| **word == command)
                        .count(),
                    1,
                    "{own:?}"
                );
            }
            sequences.push(commands.into_iter().map(<[u8]>::to_vec).collect::<Vec<_>>());
        }
        assert_eq!(sequences[0], sequences[1]);
    });
}

/// GIT_CONFIG cannot hide the public remote from the guard's config reader.
#[test]
fn git_config_cannot_hide_public_remotes() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let config = s.dir().join("empty-config");
        fs::write(&config, b"").unwrap();
        let before = transfer.everything();
        let output = s
            .git(["dupe", "remote", "add", "leak", PROJECT_URL])
            .from(&transfer.root)
            .variable("GIT_CONFIG", &config)
            .run();
        refused(&output, b"origin");
        before.unchanged();
    });
}

/// Remote's help page and short usage remain Git's own responses in an attached workspace.
#[test]
fn remote_help_is_gits_own() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let root = &transfer.root;
        let before = transfer.everything();
        let gits = s.git(["help", "remote"]).from(root).run();
        let ours = s.git(["dupe", "help", "remote"]).from(root).run();
        assert_eq!(ours, gits);
        let gits = s
            .private(root)
            .git(["-c", "help.autocorrect=0", "remote", "-h"])
            .run();
        let ours = s.git(["dupe", "remote", "-h"]).from(root).run();
        assert_eq!(ours, gits);
        before.unchanged();
    });
}
