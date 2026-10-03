//! The public `clean` run as Git receives it and as the user sees it: an alias's prefix
//! and a pathspec setting before it, a count of Git runs that the files do not change,
//! a list too long for one command line, Git's own output and status beside git-dupe's
//! lines, settle after it, and `git dupe git clean` beside it (G19, G22, G23, G25, G6,
//! G9, G20).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::harness::{
    End, Runs, Scenario, Tree, Twin, changed_since, daily_state, names, names_number, private_add,
    private_commit, region_rules, run_traced, under_each_release, write,
};

/// The `-e` options git-dupe gives `clean` without `-X` in `daily_state`'s workspace:
/// one region rule per hidden path not below another.
const DAILY_PATTERNS: [&str; 10] = [
    "-e",
    "/.env.local",
    "-e",
    "/.gitdupe",
    "-e",
    "/.vscode",
    "-e",
    "/docs/notes.md",
    "-e",
    "/notes",
];

/// `daily_state` in `name` below the scenario's directory, settled.
fn daily(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    s.git(["dupe", "status"]).from(&dir).succeeds();
    dir
}

/// The words of each run git-dupe itself started whose command word is `command`.
fn words_of(runs: &Runs, command: &str) -> Vec<Vec<Vec<u8>>> {
    let own = runs.own();
    own.words()
        .iter()
        .zip(own.commands())
        .filter(|(_, word)| *word == command.as_bytes())
        .map(|(words, _)| words.clone())
        .collect()
}

fn bytes(words: &[&str]) -> Vec<Vec<u8>> {
    words.iter().map(|word| word.as_bytes().to_vec()).collect()
}

#[test]
fn an_alias_s_prefix_stands_before_the_clean_run_and_its_command_word() {
    under_each_release(|s| {
        let dir = daily(s, "workspace");
        s.git([
            "config",
            "--global",
            "alias.wipe",
            "-c core.quotePath=false clean -n",
        ])
        .succeeds();
        let (output, runs) = run_traced(s.git(["dupe", "wipe"]).from(&dir), &s.dir().join("trace"));
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let expected: Vec<&str> = [
            "-c",
            "core.quotePath=false",
            "-c",
            "help.autocorrect=0",
            "clean",
            "-n",
        ]
        .into_iter()
        .chain(DAILY_PATTERNS)
        .collect();
        assert_eq!(words_of(&runs, "clean"), [bytes(&expected)], "{runs:?}");
    });
}

/// `--literal-pathspecs` before `dupe` governs the user's pathspecs in the `clean` run,
/// and not the paths git-dupe asks public Git about under `-X`, which it would refuse
/// (G19, S5).
#[test]
fn a_pathspec_setting_before_dupe_governs_the_users_pathspecs_alone() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        s.attached_project(&built);
        write(&built, ".gitdupe", b"");
        write(&built, "build/local.cfg", b"private\n");
        private_add(s, &built, "build/local.cfg");
        private_add(s, &built, ".gitdupe");
        private_commit(s, &built);
        for path in ["a*", "ab", "ac", "build/out.js"] {
            write(&built, path, format!("{path}\n").as_bytes());
        }
        s.git(["dupe", "status"]).from(&built).succeeds();

        let twin = Twin::of(s, &built, &[".gitdupe", "build/local.cfg"]);
        twin.clean(&["-f", "--", "a*"])
            .global_option("--literal-pathspecs")
            .run();
        twin.clean(&["-f", "--", "a*"]).run();
        let output = twin
            .clean(&["-fX"])
            .global_option("--literal-pathspecs")
            .admitting(&["build/out.js"])
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");

        // What plain Git did, said without it: the literal pathspec names one file, the
        // glob all three.
        let before = Tree::working(&built);
        let literal = s
            .git(["--literal-pathspecs", "dupe", "clean", "-f", "--", "a*"])
            .from(&built)
            .run();
        assert_eq!(literal.end, End::Code(0), "{literal:?}");
        assert_eq!(changed_since(&before, &built), ["a*"]);
        write(&built, "a*", b"a*\n");
        let before = Tree::working(&built);
        let glob = s
            .git(["dupe", "clean", "-f", "--", "a*"])
            .from(&built)
            .run();
        assert_eq!(glob.end, End::Code(0), "{glob:?}");
        assert_eq!(changed_since(&before, &built), ["a*", "ab", "ac"]);
    });
}

/// Under `-X`, one `check-ignore` over every ancestor of every hidden path, however many
/// there are, comes before the one `clean`, and, without `-d` or a pathspec, one public
/// listing before it under the hidden paths at which a directory stands, however many
/// there are, and none when no directory stands at one (`Holds/G22`, R4).
#[test]
fn under_x_one_ignore_question_comes_before_clean_whatever_the_hidden_files() {
    under_each_release(|s| {
        for count in [5, 500] {
            // Each file hidden alone, or the directory that holds it.
            for directories in [false, true] {
                let dir = s.dir().join(format!("workspace-{count}-{directories}"));
                s.attached_project(&dir);
                let listed: String = (0..count)
                    .filter(|_| directories)
                    .map(|index| format!("build/d{index}\n"))
                    .collect();
                write(&dir, ".gitdupe", listed.as_bytes());
                for index in 0..count {
                    write(&dir, &format!("build/d{index}/private"), b"private\n");
                }
                private_add(s, &dir, "build");
                private_add(s, &dir, ".gitdupe");
                private_commit(s, &dir);
                write(&dir, "build/out.js", b"out\n");
                s.git(["dupe", "status"]).from(&dir).succeeds();
                for (words, listing) in [("-nX", directories), ("-ndX", false)] {
                    let (output, runs) = run_traced(
                        s.git(["dupe", "clean", words]).from(&dir),
                        &s.dir().join("trace"),
                    );
                    assert_eq!(output.end, End::Code(0), "{output:?}");
                    // The locate, the hidden paths, the listing when it is due, the `-X`
                    // question, the `clean`, and settle's three: the private listing, the
                    // public listing, and the exposure question.
                    let own = runs.own();
                    let mut expected = vec![&b"rev-parse"[..], b"ls-files"];
                    if listing {
                        expected.push(b"ls-files");
                    }
                    expected.extend([
                        &b"check-ignore"[..],
                        b"clean",
                        b"ls-files",
                        b"ls-files",
                        b"check-ignore",
                    ]);
                    assert_eq!(
                        own.commands(),
                        expected,
                        "{words} with {count} hidden files, directories {directories}: {runs:?}"
                    );
                }
            }
        }
    });
}

/// `getconf ARG_MAX`: what one command line may carry here.
fn command_line_limit() -> usize {
    let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
    assert!(limit.status.success());
    std::str::from_utf8(&limit.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// Hidden paths that only `.gitdupe` lists, nothing at them, all directly inside the
/// ignored `build/`, so long and so many that their three patterns each under `-X`
/// without `-d` overflow one command line while settle's listing, one pathspec each,
/// fits on it: the refusal is the `clean`'s, never settle's (G23). Beyond E8's thousand
/// paths, the only way to stage it without an environment over 1 MiB.
#[test]
fn a_clean_whose_patterns_do_not_fit_is_refused_naming_their_count_and_settles() {
    under_each_release(|s| {
        // A path of 250 bytes costs settle's listing about 274 bytes of the command line
        // and the `-X` run about 818: half the limit for the one is half again over it
        // for the other.
        let count = command_line_limit() / 2 / 274;
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        write(&dir, "build/out.js", b"out\n");
        let listed: Vec<String> = (0..count)
            .map(|index| format!("build/{index:08}-{}", "x".repeat(235)))
            .collect();
        assert_eq!(listed[0].len(), 250);
        write(&dir, ".gitdupe", (listed.join("\n") + "\n").as_bytes());
        let before = Tree::working(&dir);
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-nX"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        // Every spared path, `.gitdupe` among them, and the one ignored ancestor.
        names_number(output.only_line("fatal"), count + 2);
        assert_eq!(runs.of("clean"), 0, "{runs:?}");
        assert!(changed_since(&before, &dir).is_empty());
        assert_eq!(region_rules(&dir).len(), count + 1);

        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-n"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(runs.own().of("clean"), 1, "{runs:?}");
    });
}

/// A workspace whose hidden paths have nothing at them, `.gitdupe` itself gone from disk
/// and read from the private index, so that plain `git clean` deletes and lists what the
/// command does; and a `!` rule that re-includes `notes`, so that every command warns.
fn nothing_hidden_on_disk(s: &Scenario, dir: &Path) {
    s.attached_project(dir);
    write(
        dir,
        ".gitignore",
        b".env.local\n.vscode/\nbuild/\n!/notes\n",
    );
    s.git(["add", "--", ".gitignore"]).from(dir).succeeds();
    s.commit_public(dir);
    write(dir, ".gitdupe", b"notes\n");
    private_add(s, dir, ".gitdupe");
    private_commit(s, dir);
    fs::remove_file(dir.join(".gitdupe")).unwrap();
    for path in [
        "scratch.txt",
        ".env.local",
        "build/out.js",
        "untracked-dir/f.txt",
    ] {
        write(dir, path, format!("{path}\n").as_bytes());
    }
    let settled = s.git(["dupe", "status"]).from(dir).run();
    assert_eq!(settled.end, End::Code(0), "{settled:?}");
}

#[test]
fn gits_output_and_status_are_the_users_and_git_dupes_lines_stay_on_standard_error() {
    under_each_release(|s| {
        let built = s.dir().join("workspace");
        nothing_hidden_on_disk(s, &built);
        let twin = Twin::of(s, &built, &[".gitdupe", "notes"]);
        for words in [
            &["-fdx"][..],
            &["-n"],
            &["-ndx"],
            &["-nX"],
            &["-ndX"],
            &["-fX"],
            &["-fdq"],
        ] {
            let output = twin.clean(words).same_output().run();
            for level in ["fatal: ", "error: ", "warning: ", "hint: "] {
                let printed = output.stdout.split(|&byte| byte == b'\n');
                assert!(
                    !printed
                        .clone()
                        .any(|line| line.starts_with(level.as_bytes())),
                    "{words:?}: {output:?}"
                );
            }
            names(output.only_line("warning"), b"notes");
            if words == ["-fdq"] {
                assert!(output.stdout.is_empty(), "{output:?}");
            }
        }
    });
}

/// Without `-f`, `-n`, or `-i`, Git's own `clean` refuses under its defaults: the command
/// ends with Git's lines and status, deletes nothing, and settles (G16, R3).
#[test]
fn gits_own_refusal_ends_clean_with_gits_status_and_still_settles() {
    under_each_release(|s| {
        let dir = daily(s, "workspace");
        write(&dir, "scratch.txt", b"scratch\n");
        let listed = fs::read(dir.join(".gitdupe")).unwrap();
        write(&dir, ".gitdupe", &[&listed[..], b"by-hand\n"].concat());
        // Plain Git refuses too, and deletes nothing either.
        let expected = s.git(["clean", "-d"]).from(&dir).run();
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        let before = Tree::working(&dir);
        let output = s.git(["dupe", "clean", "-d"]).from(&dir).run();
        assert_eq!(output, expected);
        assert!(changed_since(&before, &dir).is_empty());
        assert!(region_rules(&dir).contains(&b"/by-hand".to_vec()));
    });
}

/// A path whose line was removed from `.gitdupe` by hand is released by the next `clean`
/// like by any command, which warns once that public Git can see it; the `clean` after
/// that deletes it as it deletes any untracked path (G6, G9, G16).
#[test]
fn a_path_released_by_hand_is_warned_about_after_clean_and_then_cleaned() {
    under_each_release(|s| {
        let dir = daily(s, "workspace");
        let listed = fs::read(dir.join(".gitdupe")).unwrap();
        write(&dir, ".gitdupe", &[&listed[..], b"scratch\n"].concat());
        write(&dir, "scratch/x.txt", b"scratch\n");
        s.git(["dupe", "status"]).from(&dir).succeeds();
        assert!(region_rules(&dir).contains(&b"/scratch".to_vec()));

        write(&dir, ".gitdupe", &listed);
        let before = Tree::working(&dir);
        let listing = s.git(["dupe", "clean", "-n"]).from(&dir).run();
        assert_eq!(listing.end, End::Code(0), "{listing:?}");
        names(listing.only_line("warning"), b"scratch");
        assert!(!region_rules(&dir).contains(&b"/scratch".to_vec()));
        assert!(changed_since(&before, &dir).is_empty());

        let cleaned = s.git(["dupe", "clean", "-fd"]).from(&dir).run();
        assert_eq!(cleaned.end, End::Code(0), "{cleaned:?}");
        assert!(cleaned.stderr.is_empty(), "{cleaned:?}");
        assert_eq!(changed_since(&before, &dir), ["scratch", "scratch/x.txt"]);
    });
}

/// `git dupe git clean` is Git's own `clean` against the private repository, where every
/// file of the project is untracked: no pattern is added (G20).
#[test]
fn git_dupe_git_clean_is_gits_clean_in_the_private_repository_unchanged() {
    under_each_release(|s| {
        let dir = daily(s, "workspace");
        write(&dir, "scratch.txt", b"scratch\n");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "clean", "-n"])
            .run();
        let (output, runs) = run_traced(
            s.git(["dupe", "git", "clean", "-n"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(output, expected);
        assert_eq!(
            words_of(&runs, "clean"),
            [bytes(&["-c", "help.autocorrect=0", "clean", "-n"])],
            "{runs:?}"
        );
    });
}
