//! `git dupe clean` where the region is not the whole story: a `!` rule that re-includes
//! a hidden path, a line added to `.gitdupe` since the last command, hidden paths beyond
//! a file, a `.git` entry in a directory the project tracks files in, and an index that a
//! caller's `GIT_INDEX_FILE` names, as a hook's does, each observed beside plain
//! `git clean`; and a hidden path beyond a symbolic link, which refuses every form
//! (G16, S4, S5, S9).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::harness::{
    End, Scenario, Tree, Twin, changed_since, copy, holds, names, private_add, private_commit,
    run_traced, under_each_release, write, write_executable,
};

/// An attached project whose `.gitignore`, committed, is `ignore`, with an untracked
/// `scratch.txt` and the ignored `build/out.js`.
fn project(s: &Scenario, dir: &Path, ignore: &[u8]) {
    s.attached_project(dir);
    write(dir, ".gitignore", ignore);
    s.git(["add", "--", ".gitignore"]).from(dir).succeeds();
    s.commit_public(dir);
    write(dir, "scratch.txt", b"scratch\n");
    write(dir, "build/out.js", b"out\n");
}

#[test]
fn a_hidden_file_a_rule_re_includes_is_spared_under_the_standard_rules_too() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(
            s,
            &built,
            b".env.local\n.vscode/\nbuild/\n!reincluded.txt\n",
        );
        write(&built, ".gitdupe", b"reincluded.txt\n");
        write(&built, "reincluded.txt", b"private\n");
        // The exposure G6 warns about: public Git does not ignore it.
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        names(settled.lines("warning")[0], b"reincluded.txt");
        let twin = Twin::of(s, &built, &[".gitdupe", "reincluded.txt"]);
        for words in [&["-f"][..], &["-fd"], &["-fdx"], &["-fdX"], &["-n"]] {
            twin.clean(words).run();
        }
    });
}

#[test]
fn a_line_added_to_gitdupe_since_the_last_command_is_spared_before_the_region_holds_it() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(s, &built, b".env.local\n.vscode/\nbuild/\n*.tmp\n");
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        // Added by hand: the region, here and in plain Git's copy, does not hold them.
        write(&built, ".gitdupe", b"by-hand.txt\nbuild/by-hand.cfg\n");
        write(&built, "by-hand.txt", b"by hand\n");
        write(&built, "build/by-hand.cfg", b"by hand\n");
        let twin = Twin::of(s, &built, &[".gitdupe", "build/by-hand.cfg", "by-hand.txt"]);
        for words in [&["-fdx"][..], &["-fd"], &["-fx"], &["-fdX"]] {
            twin.clean(words).run();
        }
        twin.clean(&["-fX"]).admitting(&["build/out.js"]).run();
    });
}

/// Public Git never looks beyond a symbolic link, and its `clean` deletes the link, and
/// what it leads to inside the workspace, as any untracked entry: while a hidden path has
/// a link among its ancestors, every form is refused, deleting nothing, naming the path
/// and the outermost such link (G16). The link leads inside the workspace or outside it,
/// to a directory or to a repository, is ignored or not, stands where a privately
/// tracked file's directory stood, or leads to a directory holding another link.
#[test]
fn a_hidden_path_beyond_a_symbolic_link_refuses_every_form_deleting_nothing() {
    under_each_release(|s| {
        for case in [
            "inside",
            "inside-ignored",
            "outside",
            "outside-repository",
            "tracked",
            "two-links",
        ] {
            let built = s.dir().join(case);
            let ignore: &[u8] = if case == "inside-ignored" {
                b"build/\nlink\ntarget/\n"
            } else {
                b"build/\n"
            };
            project(s, &built, ignore);
            let (beyond, link): (&[u8], &[u8]) = match case {
                "inside" | "inside-ignored" => {
                    write(&built, "target/secret", b"private data\n");
                    symlink("target", built.join("link")).unwrap();
                    (b"link/secret", b"link")
                }
                "outside" | "outside-repository" => {
                    let elsewhere = s.dir().join(format!("{case}-elsewhere"));
                    write(&elsewhere, "secret", b"private data\n");
                    if case == "outside-repository" {
                        fs::write(elsewhere.join(".git"), b"").unwrap();
                    }
                    symlink(&elsewhere, built.join("link")).unwrap();
                    (b"link/secret", b"link")
                }
                "tracked" => {
                    write(&built, "conf/local.ini", b"private data\n");
                    private_add(s, &built, "conf/local.ini");
                    fs::rename(built.join("conf"), built.join("real-conf")).unwrap();
                    symlink("real-conf", built.join("conf")).unwrap();
                    (b"conf/local.ini", b"conf")
                }
                _ => {
                    write(&built, "real-inner/secret", b"private data\n");
                    fs::create_dir(built.join("real-outer")).unwrap();
                    symlink("../real-inner", built.join("real-outer/inner")).unwrap();
                    symlink("real-outer", built.join("outer")).unwrap();
                    (b"outer/inner/secret", b"outer")
                }
            };
            if case != "tracked" {
                write(&built, ".gitdupe", &[beyond, b"\n"].concat());
            }
            let settled = s.git(["dupe", "status"]).from(&built).run();
            assert_eq!(settled.end, End::Code(0), "{settled:?}");
            let before = Tree::working(&built);
            for words in [
                &["-f"][..],
                &["-fd"],
                &["-fx"],
                &["-fdx"],
                &["-ffdx"],
                &["-fX"],
                &["-fdX"],
                &["-n"],
                &["-fdx", "--", "link"],
                &["-f", "-e"],
                // Refused twice over: the link is the one named.
                &["-fX", "-e", "x"],
            ] {
                let mut command = vec!["dupe", "clean"];
                command.extend(words);
                let output = s.git(command).from(&built).run();
                let context = format!("{case} {words:?}: {output:?}");
                assert_eq!(output.end, End::Code(128), "{context}");
                assert!(output.stdout.is_empty(), "{context}");
                let refusal = output.lines("fatal");
                assert_eq!(refusal.len(), 1, "{context}");
                names(refusal[0], beyond);
                // The link is named apart from the path, and it is the outermost one.
                let rest = without(refusal[0], beyond);
                names(&rest, link);
                assert!(!holds(&rest, &[link, b"/"].concat()), "{context}");
                assert!(changed_since(&before, &built).is_empty(), "{context}");
            }
        }
    });
}

/// `line` with every run of its bytes that is `part` taken out.
fn without(line: &[u8], part: &[u8]) -> Vec<u8> {
    let mut rest = Vec::new();
    let mut at = 0;
    while at < line.len() {
        if line[at..].starts_with(part) {
            at += part.len();
        } else {
            rest.push(line[at]);
            at += 1;
        }
    }
    rest
}

/// A file standing where a directory above a hidden path would stand is no directory to
/// open under `-X`: it is one more ignored file of the ignored directory that holds it,
/// deleted or kept as plain Git takes that directory, and nothing ever stands at the
/// hidden path below it (G16, S4).
#[test]
fn a_file_standing_above_a_hidden_path_is_an_ignored_file_like_any_other() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(s, &built, b"build/\n/ign/\n*.o\n");
        write(&built, ".gitdupe", b"ign/blocked/secret\nall/f.o/secret\n");
        for path in ["ign/blocked", "ign/junk", "all/f.o", "all/a.o"] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        let twin = Twin::of(
            s,
            &built,
            &[".gitdupe", "ign/blocked/secret", "all/f.o/secret"],
        );
        for words in [
            &["-fX"][..],
            &["-nX"],
            &["-fdX"],
            &["-ffdX"],
            &["-fd"],
            &["-fdx"],
        ] {
            twin.clean(words).run();
        }
        twin.clean(&["-fX"]).below("ign").run();
        twin.clean(&["-fX", "--", "ign/blocked"])
            .keeping(&["ign/junk"])
            .run();
    });
}

/// A repository initialized inside a directory the project tracks files in: Git walks
/// that directory as any other, so it is no untracked nested repository, and the hidden
/// path in it is spared alone, while an untracked repository inside it that holds a
/// hidden path is spared whole (G16, S4). Telling the two apart takes one public listing
/// under the directories holding a `.git` entry, made before `clean` and only when one
/// does (`Holds/G22`).
#[test]
fn a_directory_holding_a_git_entry_and_tracked_files_is_entered_as_git_enters_it() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(s, &built, b".env.local\n.vscode/\nbuild/\n*.tmp\n");
        write(&built, "nested/public", b"public\n");
        s.git(["add", "--", "nested/public"])
            .from(&built)
            .succeeds();
        s.commit_public(&built);
        write(&built, ".gitdupe", b"nested/secret\nnested/inner/secret\n");
        for path in [
            "nested/secret",
            "nested/junk",
            "nested/junk.tmp",
            "nested/sub/junk",
            "nested/inner/secret",
            "nested/inner/other",
        ] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        for repository in ["nested", "nested/inner"] {
            s.git(["init", "-q"])
                .from(&built.join(repository))
                .succeeds();
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");

        let twin = Twin::of(s, &built, &[".gitdupe", "nested/secret", "nested/inner"]);
        for words in [
            &["-f"][..],
            &["-fd"],
            &["-fx"],
            &["-fdx"],
            &["-ffdx"],
            &["-fX"],
            &["-fdX"],
            &["-ffdX"],
            &["-ndx"],
            &["-fdx", "--", "nested"],
        ] {
            twin.clean(words).run();
        }

        let log = s.dir().join("trace");
        let (output, runs) = run_traced(s.git(["dupe", "clean", "-n"]).from(&built), &log);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let own = runs.own();
        let commands = own.commands();
        let at = commands.iter().position(|command| *command == b"clean");
        assert_eq!(
            commands[..at.unwrap()],
            [&b"rev-parse"[..], b"ls-files", b"ls-files"],
            "{runs:?}"
        );
        let listing = &own.words()[2];
        let asked = [&b":(top,literal)nested"[..], b":(top,literal)nested/inner"];
        assert!(listing.ends_with(&asked.map(<[u8]>::to_vec)), "{runs:?}");
    });
}

/// A hidden directory into which a publicly tracked file arrives afterwards, as a pull
/// brings a teammate's file (G7): `notes` at the root, and `ign/keep` inside the ignored
/// `ign/`, where the file is tracked by force. Git enters a directory that holds a path of
/// its index (S4), whatever the patterns say of it, so every form reaches what lies in
/// both, and every hidden path there stays. Beside them, `pile/h`, a hidden directory
/// under which the index holds nothing, is still one ignored entry under `-X` without
/// `-d` or a pathspec, so the ignored `pile/` stays whole, as plain Git keeps it (G16).
#[test]
fn a_hidden_directory_the_project_tracks_a_file_in_is_spared_as_git_enters_it() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(
            s,
            &built,
            b".env.local\n.vscode/\nbuild/\nign/\npile/\n*.tmp\n",
        );
        write(&built, ".gitdupe", b"notes\nign/keep\npile/h\n");
        write(&built, "notes/today.md", b"today\n");
        write(&built, "ign/keep/mine", b"mine\n");
        write(&built, "pile/h/mine", b"mine\n");
        for path in [".gitdupe", "notes/today.md", "ign/keep/mine", "pile/h/mine"] {
            private_add(s, &built, path);
        }
        private_commit(s, &built);
        write(&built, "notes/shared.md", b"shared\n");
        write(&built, "ign/keep/shared", b"shared\n");
        s.git(["add", "-f", "--", "notes/shared.md", "ign/keep/shared"])
            .from(&built)
            .succeeds();
        s.commit_public(&built);
        for path in [
            "notes/scratch.md",
            "notes/junk.tmp",
            "notes/sub/deep.md",
            "ign/keep/scratch",
            "ign/junk",
            "ign/other/junk",
            "pile/h/scratch",
            "pile/junk",
        ] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");

        let twin = Twin::of(s, &built, &[".gitdupe", "notes", "ign/keep", "pile/h"]);
        for words in [
            &["-f"][..],
            &["-fd"],
            &["-fx"],
            &["-fdx"],
            &["-ffdx"],
            &["-fX"],
            &["-nX"],
            &["-fdX"],
            &["-ffdX"],
            &["-fX", "--", "notes", "ign"],
        ] {
            twin.clean(words).run();
        }
        // `1` chooses Git's own "clean" in its menu.
        twin.clean(&["-iX"]).input(b"1\n").run();
        for below in ["notes", "ign"] {
            twin.clean(&["-fX"]).below(below).run();
            twin.clean(&["-fdX"]).below(below).run();
        }
    });
}

#[test]
fn without_a_git_entry_above_a_hidden_path_no_public_listing_comes_before_clean() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        project(s, &dir, b".env.local\n.vscode/\nbuild/\n*.tmp\n");
        write(&dir, ".gitdupe", b"build/local.cfg\nnotes\n");
        write(&dir, "build/local.cfg", b"local\n");
        write(&dir, "notes/today.md", b"today\n");
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-n"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let own = runs.own();
        let commands = own.commands();
        let at = commands.iter().position(|command| *command == b"clean");
        assert_eq!(
            commands[..at.unwrap()],
            [&b"rev-parse"[..], b"ls-files"],
            "{runs:?}"
        );
    });
}

/// Git's `clean` reads the index `GIT_INDEX_FILE` names, a hook's temporary one among
/// them (S9), and decides by it which directory holding a `.git` entry is an untracked
/// repository it never enters (S4); the listing that tells a nested repository from a
/// directory the project tracks files in reads the same index (`Holds/G16`). In the named
/// index here, the tree of `HEAD`, `nested` holds no entry though the public index has a
/// file staged in it, and `entered` holds one though the public index has its deletion
/// staged: the hidden path in `nested` is spared with the whole repository, the one in
/// `entered` alone, and the rest goes as plain `git clean` under the same index takes it
/// (G16). Without the variable, the same workspace is decided by the public index.
#[test]
fn under_a_named_index_the_nested_repositories_are_those_of_that_index() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        project(s, &built, b".env.local\n.vscode/\nbuild/\n*.tmp\n");
        write(&built, "entered/public", b"public\n");
        s.git(["add", "--", "entered/public"])
            .from(&built)
            .succeeds();
        s.commit_public(&built);
        s.git(["read-tree", "HEAD"])
            .from(&built)
            .variable("GIT_INDEX_FILE", built.join(".git/hook-index"))
            .succeeds();
        write(&built, "nested/public", b"public\n");
        s.git(["add", "--", "nested/public"])
            .from(&built)
            .succeeds();
        s.git(["rm", "-q", "--cached", "--", "entered/public"])
            .from(&built)
            .succeeds();
        write(&built, ".gitdupe", b"nested/secret\nentered/secret\n");
        for path in [
            "nested/secret",
            "nested/junk",
            "entered/secret",
            "entered/junk",
            "entered/junk.tmp",
        ] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        for repository in ["nested", "entered"] {
            s.git(["init", "-q"])
                .from(&built.join(repository))
                .succeeds();
        }
        let settled = s.git(["dupe", "status"]).from(&built).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");

        let twin = Twin::of(s, &built, &[".gitdupe", "nested", "entered/secret"]);
        for words in [
            &["-f"][..],
            &["-fd"],
            &["-fdx"],
            &["-ffd"],
            &["-ffdx"],
            &["-fX"],
            &["-ffdX"],
            &["-ndx"],
            &["-ffdx", "--", "nested", "entered"],
        ] {
            twin.clean(words)
                .variable("GIT_INDEX_FILE", ".git/hook-index")
                .run();
        }
        let unnamed = s.dir().join("unnamed");
        copy(&built, &unnamed);
        let twin = Twin::of(s, &unnamed, &[".gitdupe", "nested/secret", "entered"]);
        for words in [&["-fdx"][..], &["-ffdx"]] {
            twin.clean(words).run();
        }
    });
}

/// The route a hook takes: `git commit -o` hands its `pre-commit` hook a temporary index
/// built from `HEAD` and the paths it commits (S9), which lacks the file staged in
/// `nested`, so that to Git's `clean` there `nested` is an untracked repository, which
/// `-ff` deletes whole. `git dupe clean -ffdx` from the hook spares it whole, the hidden
/// path in it included, and deletes the rest (G16, G10).
#[test]
fn from_a_pre_commit_hook_clean_spares_what_the_hooks_index_makes_a_nested_repository() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        project(s, &dir, b".env.local\n.vscode/\nbuild/\n*.tmp\n");
        write(&dir, "nested/public", b"public\n");
        write(&dir, "committed.txt", b"committed\n");
        s.git(["add", "--", "nested/public", "committed.txt"])
            .from(&dir)
            .succeeds();
        write(&dir, ".gitdupe", b"nested/secret\n");
        write(&dir, "nested/secret", b"secret\n");
        write(&dir, "nested/junk", b"junk\n");
        s.git(["init", "-q"]).from(&dir.join("nested")).succeeds();
        let settled = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        let hook = dir.join(".git/hooks/pre-commit");
        write_executable(s, &hook, b"#!/bin/sh\ngit dupe clean -ffdx\n");

        let before = Tree::of(&dir);
        let output = s
            .git([
                "-c",
                "maintenance.auto=false",
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "commit",
                "-qm",
                "only",
                "-o",
                "--",
                "committed.txt",
            ])
            .from(&dir)
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("fatal").is_empty(), "{output:?}");
        let after = Tree::of(&dir);
        assert!(
            before.same_at_or_below(&after, &dir.join("nested")),
            "nested changed; {output:?}"
        );
        for gone in ["scratch.txt", "build/out.js"] {
            assert!(!after.holds(&dir.join(gone)), "{gone} stands; {output:?}");
        }
        let committed = s
            .git(["show", "--name-only", "--format=", "HEAD"])
            .from(&dir)
            .succeeds();
        assert_eq!(committed.stdout, b"committed.txt\n", "{committed:?}");
    });
}
