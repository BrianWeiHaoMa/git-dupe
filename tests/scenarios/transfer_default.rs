//! Default transfer destinations and the conservative reading of repository words (G18).

use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Transfer, names, private_commit, run_traced, under_each_release, write,
};

fn defaults(s: &Scenario, root: &Path, keys: &[(&str, &str)]) {
    for key in [
        "branch.main.pushRemote",
        "remote.pushDefault",
        "branch.main.remote",
    ] {
        let output = s.private(root).git(["config", "--unset-all", key]).run();
        assert!(matches!(output.end, End::Code(0 | 5)), "{key}: {output:?}");
    }
    for &(key, value) in keys {
        s.private(root).git(["config", key, value]).succeeds();
    }
}

fn refusal(output: &Output, place: &[u8], route: &[u8]) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, place);
    names(line, route);
}

fn refused_default(s: &Scenario, transfer: &Transfer, words: &[&str], key: &str, from: &Path) {
    let before = transfer.everything();
    let (output, runs) = run_traced(
        s.git(["dupe"].into_iter().chain(words.iter().copied()))
            .from(from),
        &s.dir().join("trace"),
    );
    refusal(
        &output,
        transfer.root.to_str().unwrap().as_bytes(),
        b"git dupe git",
    );
    names(
        output.only_line("fatal"),
        key.to_ascii_lowercase().as_bytes(),
    );
    names(output.only_line("fatal"), b"given no repository");
    assert_eq!(runs.own().of(words[0]), 0, "{runs:?}");
    before.unchanged();
}

/// Git's command must be started with every user word intact, regardless of its answer.
fn runs(s: &Scenario, root: &Path, from: &Path, words: &[&str]) -> Output {
    let (output, runs) = run_traced(
        s.git(["dupe"].into_iter().chain(words.iter().copied()))
            .from(from),
        &s.dir().join("trace"),
    );
    let tail: Vec<Vec<u8>> = ["-c", "help.autocorrect=0"]
        .into_iter()
        .chain(words.iter().copied())
        .map(|word| word.as_bytes().to_vec())
        .collect();
    let own = runs.own();
    assert_eq!(own.of(words[0]), 1, "{output:?}: {own:?}");
    assert_eq!(
        own.words()
            .iter()
            .filter(|run| run.ends_with(&tail))
            .count(),
        1,
        "{}: {output:?}: {own:?}",
        root.display()
    );
    output
}

#[test]
fn transfer_default_push_precedence_and_delivery() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("workspace");
        let root = &transfer.root;
        for (key, value) in [
            ("branch.main.pushRemote", "."),
            ("remote.pushDefault", root.to_str().unwrap()),
            ("branch.main.remote", "."),
        ] {
            defaults(s, root, &[(key, value)]);
            refused_default(s, &transfer, &["push"], key, root);
        }
        for key in ["branch.main.pushRemote", "remote.pushDefault"] {
            defaults(s, root, &[("branch.main.remote", "."), (key, "origin")]);
            write(root, "notes/a.md", key.as_bytes());
            s.private(root).git(["add", "notes/a.md"]).succeeds();
            private_commit(s, root);
            let output = runs(s, root, root, &["push"]);
            assert_eq!(output.end, End::Code(0), "{output:?}");
            let head = s.private(root).git(["rev-parse", "HEAD"]).succeeds();
            let received = s
                .git(["rev-parse", "refs/heads/main"])
                .from(&transfer.private_remote)
                .succeeds();
            assert_eq!(head.stdout, received.stdout);
        }
    });
}

#[test]
fn transfer_default_fetch_pull_and_detached_head() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("workspace");
        let root = &transfer.root;
        for command in ["fetch", "pull"] {
            defaults(s, root, &[("branch.main.remote", ".")]);
            refused_default(s, &transfer, &[command], "branch.main.remote", root);
            defaults(s, root, &[("remote.pushDefault", ".")]);
            runs(s, root, root, &[command]);
        }
        s.private(root)
            .git(["checkout", "--detach", "HEAD"])
            .succeeds();
        defaults(s, root, &[("branch.main.remote", ".")]);
        for command in ["push", "fetch", "pull"] {
            runs(s, root, root, &[command]);
        }
        defaults(s, root, &[("remote.pushDefault", ".")]);
        refused_default(s, &transfer, &["push"], "remote.pushdefault", root);
    });
}

#[test]
fn transfer_default_configured_remote_cleaned_path_and_explicit_repository() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("workspace");
        let root = &transfer.root;
        for command in ["push", "fetch", "pull"] {
            runs(s, root, root, &[command]);
        }
        let path = root.join("sub/..");
        defaults(s, root, &[("branch.main.remote", path.to_str().unwrap())]);
        refused_default(s, &transfer, &["fetch"], "branch.main.remote", root);
        defaults(s, root, &[("branch.main.remote", ".")]);
        for words in [&["push", "origin", "main"][..], &["fetch", "origin"]] {
            let output = runs(s, root, root, words);
            assert_eq!(output.end, End::Code(0), "{output:?}");
        }
    });
}

#[test]
fn transfer_default_reads_repository_words_left_to_right() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("workspace");
        let root = &transfer.root;
        defaults(s, root, &[("branch.main.remote", ".")]);
        let mut failed = Vec::new();
        for (words, given) in [
            (&["push", "-u", "origin", "main"][..], true),
            (&["push", "-u", "origin"], true),
            (&["fetch", "--jobs", "origin"], false),
            (&["pull", "--jobs", "origin"], false),
            (&["pull", "-j", "1"], false),
            (&["push", "-o", "ci.skip", "origin"], true),
            (&["push", "-o", "--repo=origin"], false),
            (&["push", "--repo=origin", "--no-repo"], false),
            (&["push", "--repo=origin", "--no-rep"], false),
            (&["fetch", "-o", "--", "-q"], false),
            (&["fetch", "--repo=origin"], false),
            (&["push", "-q"], false),
            (&["push", "--no-thin", "-o", "origin"], false),
            (&["push", "--force-with-lease=main", "-o", "origin"], false),
            (&["push", "--repo=origin"], true),
            (&["push", "--", "origin", "main"], true),
        ] {
            // `push -u` rewrites branch.main.remote: restore the premise for every case.
            defaults(s, root, &[("branch.main.remote", ".")]);
            // Finish the walk after a failure so each specified reading is observed.
            let checked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if given {
                    runs(s, root, root, words);
                } else {
                    refused_default(s, &transfer, words, "branch.main.remote", root);
                }
            }));
            if checked.is_err() {
                failed.push(words);
            }
        }
        assert!(failed.is_empty(), "repository readings failed: {failed:?}");
    });
}

#[test]
fn transfer_default_help_inside_git_directory_still_guards_destinations() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("workspace");
        let root = &transfer.root;
        let directory = root.join(".git");
        let before = transfer.everything();
        let output = s
            .git(["dupe", "fetch", "-o", "-h", root.to_str().unwrap(), "main"])
            .from(&directory)
            .run();
        refusal(&output, root.to_str().unwrap().as_bytes(), b"git dupe git");
        assert!(!directory.join("dupe/FETCH_HEAD").exists());
        before.unchanged();
        defaults(s, root, &[("branch.main.remote", ".")]);
        refused_default(
            s,
            &transfer,
            &["fetch", "-o", "-h"],
            "branch.main.remote",
            &directory,
        );
        let words = [
            "fetch",
            "-o",
            "-h",
            transfer.private_remote.to_str().unwrap(),
            "main",
        ];
        let expected = s
            .git(["-c", "help.autocorrect=0"].into_iter().chain(words))
            .variable("GIT_DIR", directory.join("dupe"))
            .from(&directory)
            .run();
        let output = runs(s, root, &directory, &words);
        assert_eq!(output, expected);
        s.git([
            "dupe",
            "git",
            "remote",
            "add",
            "leak",
            "https://example.com/team/project",
        ])
        .from(root)
        .succeeds();
        let before = transfer.everything();
        let output = s
            .git(["dupe"].into_iter().chain(words))
            .from(&directory)
            .run();
        refusal(&output, b"origin", b"git dupe remote remove");
        names(output.only_line("fatal"), b"leak");
        before.unchanged();
    });
}
