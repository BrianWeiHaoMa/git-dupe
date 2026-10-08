//! Foreign paths join settle's existing public listing and exposure question alone:
//! no handler scope or Git run grows with another worktree's hidden paths (G22, G27).

use std::fs;
use std::os::unix::fs::symlink;

use crate::harness::{
    End, Output, Runs, Scenario, Tree, Worktree, names, private_add, private_commit, run_traced,
    stash_untracked_line, under_each_release, write,
};

fn workspace(s: &Scenario) -> Worktree {
    let root = s.dir().join("project");
    s.attached_project(&root);
    write(&root, ".gitdupe", b"mine\n");
    write(&root, "mine/note", b"private\n");
    private_add(s, &root, ".gitdupe");
    private_add(s, &root, "mine");
    private_commit(s, &root);
    s.private(&root)
        .git(["config", "alias.sa", "status"])
        .succeeds();
    s.git(["dupe", "status"]).from(&root).succeeds();
    fs::create_dir(root.join("sub")).unwrap();
    Worktree::read(s, &root)
}

/// A real attached worktree supplies the rules and their literal escaping. Its foreign
/// paths need not stand there: they are a promise in that worktree's `.gitdupe`.
fn owner(s: &Scenario, main: &Worktree, name: &str, paths: &[String]) -> Worktree {
    let root = s.dir().join(name);
    s.linked_worktree(&main.root, &root);
    s.init(&root);
    write(
        &root,
        ".gitdupe",
        format!("{}\n", paths.join("\n")).as_bytes(),
    );
    let settled = s.git(["dupe", "status"]).from(&root).succeeds();
    assert!(settled.lines("warning").is_empty(), "{settled:?}");
    Worktree::read(s, &root)
}

fn warning(path: &str, owners: &str, hideable: bool) -> Vec<u8> {
    let mut line = format!(
        "{path} stands here and is hidden by {owners} alone: public Git ignores it here, \
         and this worktree does not hide it"
    );
    if hideable {
        line.push_str(&format!(
            "; run from the root, 'git dupe hide -- {}' hides it here too",
            one_word(path)
        ));
    }
    line.into_bytes()
}

/// The path as the offered command shows it, so that a shell takes it as one word (G25):
/// as it stands when no shell reads a byte of it specially, else double-quoted.
fn one_word(path: &str) -> String {
    let plain = |byte: u8| byte.is_ascii_alphanumeric() || b"._/@%+=:,-".contains(&byte);
    if path.bytes().all(plain) {
        return path.to_string();
    }
    let escaped: String = path
        .chars()
        .flat_map(|c| match c {
            '\\' | '"' | '$' | '`' => vec!['\\', c],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

fn warnings(output: &Output, paths: &[String], owners: &str, unhideable: &[String]) {
    let mut found = output.lines("warning");
    found.sort();
    let mut expected: Vec<_> = paths
        .iter()
        .map(|path| warning(path, owners, !unhideable.contains(path)))
        .collect();
    expected.sort();
    assert_eq!(found, expected, "{output:?}");
}

/// The final three runs are settle's private listing, public listing, and ignore
/// question (Composition/Keeper). Only that public listing carries foreign pathspecs.
fn listing(runs: &Runs, paths: &[String], absent: &[String]) -> Vec<Vec<Vec<u8>>> {
    let own = runs.own();
    let commands = own.commands();
    let count = commands.len();
    assert_eq!(
        &commands[count - 3..],
        &[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]
    );
    let mut words = own.words().to_vec();
    let public = &words[count - 2];
    assert!(public.iter().any(|word| word == b"--"), "{public:?}");
    for path in paths {
        let literal = format!(":(top,literal){path}").into_bytes();
        assert_eq!(
            public.iter().filter(|word| **word == literal).count(),
            1,
            "{public:?}"
        );
        for (at, run) in words.iter().enumerate() {
            if at != count - 2 {
                assert!(
                    !run.iter()
                        .any(|word| word == &literal || word == path.as_bytes()),
                    "foreign path in handler or private listing: {run:?}"
                );
            }
        }
    }
    for path in absent {
        let literal = format!(":(top,literal){path}").into_bytes();
        assert!(!public.contains(&literal), "{public:?}");
    }
    words[count - 2].retain(|word| {
        !paths
            .iter()
            .any(|path| word == format!(":(top,literal){path}").as_bytes())
    });
    words
}

#[test]
fn standing_foreign_paths_add_no_git_run_or_handler_pathspec_to_settling_commands() {
    under_each_release(|s| {
        let main = workspace(s);
        let paths: Vec<String> = ["foreign/note", "foreign/deep/a", "foreign/deep/b"]
            .map(String::from)
            .to_vec();
        let other = owner(s, &main, "agent", &paths);
        let other_region = other.region_bytes();
        let main_region = main.region_bytes();
        let public = main.public_git();
        let trace = s.dir().join("trace");
        let commands: &[(&[&str], i32, bool)] = &[
            (&["log", "-1"], 0, false),
            (&["stash", "-u"], 128, false),
            (&["status"], 0, false),
            (&["add", "."], 0, false),
            (&["clean", "-n"], 0, false),
            (&["init"], 0, false),
            (&["clone", "unused-url"], 128, false),
            (&["sa"], 0, false),
            (&["git", "status"], 0, false),
            (&["status", "--porcelain", "-z"], 0, true),
        ];
        for &(words, status, below) in commands {
            let from = if below {
                main.root.join("sub")
            } else {
                main.root.clone()
            };
            let command = || {
                s.git(["dupe"].into_iter().chain(words.iter().copied()))
                    .from(&from)
            };
            let (before, before_runs) = run_traced(command(), &trace);
            assert_eq!(before.end, End::Code(status), "{words:?}: {before:?}");
            assert!(before.lines("warning").is_empty(), "{before:?}");
            if words[0] == "stash" {
                assert_eq!(before.lines("fatal"), [stash_untracked_line(s).as_slice()]);
            } else if words[0] == "clone" {
                names(
                    before.only_line("fatal"),
                    b"this workspace is already attached to a private repository",
                );
            }
            for path in &paths {
                write(&main.root, path, b"foreign\n");
            }
            let working = Tree::working(&main.root);
            let index = main
                .private(s)
                .git(["ls-files", "--stage", "-z"])
                .succeeds();
            let (output, runs) = run_traced(command(), &trace);
            assert_eq!(output.end, End::Code(status), "{words:?}: {output:?}");
            assert_eq!(output.lines("fatal"), before.lines("fatal"));
            assert_eq!(output.lines("hint"), before.lines("hint"));
            assert_eq!(runs.commands(), before_runs.commands(), "{words:?}");
            assert_eq!(runs.count(), before_runs.count(), "{words:?}");
            assert_eq!(
                listing(&runs, &paths, &[]),
                before_runs.own().words(),
                "{words:?}"
            );
            warnings(&output, &paths, "worktree agent", &[]);
            assert_eq!(output.stdout, before.stdout, "{words:?}: {output:?}");
            if below {
                let expected = main
                    .private(s)
                    .git(["status", "--porcelain", "-z"])
                    .from(&from)
                    .succeeds();
                assert_eq!(output.stdout, expected.stdout);
            }
            assert_eq!(main.region_bytes(), main_region);
            assert_eq!(other.region_bytes(), other_region);
            assert_eq!(
                main.private(s)
                    .git(["ls-files", "--stage", "-z"])
                    .succeeds(),
                index
            );
            assert!(working.changed_in(&Tree::working(&main.root)).is_empty());
            assert!(public.changed_in(&main.public_git()).is_empty());
            for path in &paths {
                fs::remove_file(main.root.join(path)).unwrap();
            }
        }
    });
}

#[test]
fn many_literal_foreign_paths_are_listed_once_and_name_both_worktrees_without_more_runs() {
    under_each_release(|s| {
        let main = workspace(s);
        let marks = ["*", "?", "[", "\\", " ", "#", "!", "-"];
        let paths: Vec<String> = (0..128)
            .map(|index| {
                format!(
                    "-foreign/shared/deep/{}name-{index:03}",
                    marks[index % marks.len()]
                )
            })
            .collect();
        // G27: the literal *, ?, and [ paths are listed and warned, but hide
        // refuses these operand spellings, so their warnings offer no remedy.
        let unhideable: Vec<_> = paths
            .iter()
            .enumerate()
            .filter(|(index, _)| index % marks.len() < 3)
            .map(|(_, path)| path.clone())
            .collect();
        let beyond = String::from("bridge/leaf");
        let all: Vec<_> = paths.iter().cloned().chain([beyond.clone()]).collect();
        let first = owner(s, &main, "agent", &all);
        let second = owner(s, &main, "other", &all);
        let first_region = first.region_bytes();
        let second_region = second.region_bytes();
        let main_region = main.region_bytes();
        let outside = s.dir().join("outside");
        write(&outside, "leaf", b"beyond\n");
        symlink(&outside, main.root.join("bridge")).unwrap();
        write(&main.root, &paths[0], b"foreign\n");
        let trace = s.dir().join("trace");
        let (one, one_runs) = run_traced(s.git(["dupe", "status"]).from(&main.root), &trace);
        assert_eq!(one.end, End::Code(0), "{one:?}");
        warnings(
            &one,
            &paths[..1],
            "worktree agent and worktree other",
            &unhideable,
        );
        let one_words = listing(&one_runs, &paths[..1], std::slice::from_ref(&beyond));
        for path in &paths[1..] {
            write(&main.root, path, b"foreign\n");
        }
        let working = Tree::working(&main.root);
        let public = main.public_git();
        let index = main
            .private(s)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let (many, many_runs) = run_traced(s.git(["dupe", "status"]).from(&main.root), &trace);
        assert_eq!(many.end, End::Code(0), "{many:?}");
        assert_eq!(many_runs.count(), one_runs.count());
        assert_eq!(many_runs.commands(), one_runs.commands());
        assert_eq!(listing(&many_runs, &paths, &[beyond]), one_words);
        warnings(
            &many,
            &paths,
            "worktree agent and worktree other",
            &unhideable,
        );
        assert_eq!(many.stdout, one.stdout);
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(first.region_bytes(), first_region);
        assert_eq!(second.region_bytes(), second_region);
        assert_eq!(
            main.private(s)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            index
        );
        assert!(working.changed_in(&Tree::working(&main.root)).is_empty());
        assert!(public.changed_in(&main.public_git()).is_empty());
        assert_eq!(fs::read(outside.join("leaf")).unwrap(), b"beyond\n");
    });
}

#[test]
fn dropping_a_stale_region_and_asking_foreign_paths_starts_no_git_under_the_lock() {
    under_each_release(|s| {
        let main = workspace(s);
        let paths = vec![String::from("foreign/deep/note")];
        let live = owner(s, &main, "agent", &paths);
        let stale = owner(s, &main, "gone", &[String::from("stale-note")]);
        let live_region = live.region_bytes();
        let main_region = main.region_bytes();
        let exclude = main.common_directory.join("info/exclude");
        let before = fs::read(&exclude).unwrap();
        let stale_region = stale.region_bytes();
        let at = before
            .windows(stale_region.len())
            .position(|bytes| bytes == stale_region)
            .unwrap();
        let expected = [&before[..at], &before[at + stale_region.len()..]].concat();
        fs::remove_dir_all(stale.private_directory()).unwrap();
        write(&main.root, &paths[0], b"foreign\n");
        write(&main.root, "stale-note", b"released\n");
        let working = Tree::working(&main.root);
        let public = main.public_git();
        let index = main
            .private(s)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let watch = s.watching_the_lock("watch", &main.common_directory);
        let output = watch.run(&main.root, &["status"]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        warnings(&output, &paths, "worktree agent", &[]);
        assert_eq!(fs::read(&exclude).unwrap(), expected);
        assert!(stale.region().is_none());
        assert!(!stale.private_directory().exists());
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(live.region_bytes(), live_region);
        assert_eq!(
            main.private(s)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            index
        );
        assert!(working.changed_in(&Tree::working(&main.root)).is_empty());
        assert!(public.changed_in(&main.public_git()).is_empty());
        assert_eq!(
            s.git(["check-ignore", "--", "stale-note"])
                .from(&main.root)
                .run()
                .end,
            End::Code(1)
        );
    });
}
