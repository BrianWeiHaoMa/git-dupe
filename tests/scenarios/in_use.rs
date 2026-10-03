//! The product's `In use` steps 1 to 4, in order, on one workspace.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, changed_since, holds, leaving_public_git, names, region_rules,
    staged_gitdupe, unchanged, under_each_release, warnings_in_any_order, write,
};

const DATE: &str = "2000-01-01T00:00:00Z";

fn dupe(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    leaving_public_git(
        dir,
        s.git(["dupe"].into_iter().chain(words.iter().copied()))
            .from(dir)
            .variable("GIT_AUTHOR_DATE", DATE)
            .variable("GIT_COMMITTER_DATE", DATE),
    )
}

fn succeeds(output: Output) -> Output {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    output
}

fn direct(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    s.private(dir)
        .git(
            ["-c", "help.autocorrect=0"]
                .into_iter()
                .chain(words.iter().copied()),
        )
        .variable("GIT_AUTHOR_DATE", DATE)
        .variable("GIT_COMMITTER_DATE", DATE)
        .run()
}

fn staged(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir)
        .git(["diff", "--cached", "--name-status", "-z"])
        .succeeds()
        .stdout
}

fn public_status(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git(["status", "--porcelain", "--untracked-files=all"])
        .from(dir)
        .succeeds()
        .stdout
}

fn head(s: &Scenario, dir: &Path) -> String {
    String::from_utf8(s.private(dir).git(["rev-parse", "HEAD"]).succeeds().stdout)
        .unwrap()
        .trim()
        .to_owned()
}

#[test]
fn cloned_project_through_first_commit_and_daily_private_work() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let origin = s.dir().join("origin");
        let dir = s.dir().join("workspace");
        s.unattached_project(&origin);
        s.git(["clone", origin.to_str().unwrap(), dir.to_str().unwrap()])
            .succeeds();
        let public_log = s.git(["log", "--format=%H"]).from(&dir).succeeds();

        // Step 1: attach in the clone, preserving the project's branch and index.
        let init = succeeds(dupe(s, &dir, &["init"]));
        names(&init.stdout, dir.join(".git/dupe").as_os_str().as_bytes());
        for level in [b"hint: ".as_slice(), b"warning: ", b"fatal: ", b"error: "] {
            assert!(!holds(&init.stdout, level), "{init:?}");
        }
        let expected = succeeds(direct(
            s,
            &dir,
            &[
                "status",
                "--untracked-files=normal",
                "--ignored",
                "--",
                ":(top,literal).gitdupe",
            ],
        ));
        let status = succeeds(dupe(s, &dir, &["status"]));
        assert_eq!(status.stdout, expected.stdout);
        assert!(expected.stderr.is_empty(), "{expected:?}");
        names(status.only_line("hint"), b"git dupe add <path>");
        names(status.only_line("hint"), b"nothing is private yet");
        assert_eq!(
            s.private(&dir)
                .git(["symbolic-ref", "HEAD"])
                .succeeds()
                .stdout,
            b"refs/heads/main\n"
        );
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout
                .is_empty()
        );
        for (key, value) in [
            ("status.showUntrackedFiles", b"no\n".as_slice()),
            ("advice.statusHints", b"false\n"),
            ("core.worktree", b"../..\n"),
        ] {
            assert_eq!(
                s.private(&dir).git(["config", key]).succeeds().stdout,
                value
            );
        }

        // Step 2: directories declare future privacy; individual files are tracked.
        let original = (1..=30)
            .map(|n| format!("private line {n}\n"))
            .collect::<String>();
        write(&dir, "notes/a.md", original.as_bytes());
        write(&dir, "notes/b.md", b"private second note\n");
        let add = succeeds(dupe(s, &dir, &["add", "notes/"]));
        names(add.only_line("hint"), b"notes");
        names(add.only_line("hint"), b"git dupe unhide");
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n");
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"notes\n");
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b".gitdupe\0notes/a.md\0notes/b.md\0"
        );
        write(&dir, ".env.local", b"private environment\n");
        let before = s
            .private(&dir)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let expected = direct(s, &dir, &["add", "--", ":(top,literal).env.local"]);
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        assert_eq!(dupe(s, &dir, &["add", ".env.local"]), expected);
        assert_eq!(
            s.private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            before
        );
        write(&dir, ".vscode/settings.json", b"private editor settings\n");
        let forced = succeeds(dupe(s, &dir, &["add", "-f", ".env.local", ".vscode/"]));
        names(forced.only_line("hint"), b".vscode");
        names(forced.only_line("hint"), b"git dupe unhide");
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"notes\n.vscode\n");
        write(&dir, "docs/notes.md", b"private design notes\n");
        succeeds(dupe(s, &dir, &["add", "docs/notes.md"]));
        write(&dir, "docs/design.md", b"public design edit\n");
        assert_eq!(public_status(s, &dir), b" M docs/design.md\n");
        let private_paths =
            b".env.local\0.gitdupe\0.vscode/settings.json\0docs/notes.md\0notes/a.md\0notes/b.md\0";
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            private_paths
        );
        succeeds(dupe(s, &dir, &["commit", "-m", "Local settings"]));
        assert_eq!(
            s.private(&dir)
                .git(["ls-tree", "-r", "--name-only", "-z", "HEAD"])
                .succeeds()
                .stdout,
            private_paths
        );
        assert_eq!(
            s.private(&dir)
                .git(["show", "HEAD:.gitdupe"])
                .succeeds()
                .stdout,
            b"notes\n.vscode\n"
        );
        assert_eq!(public_status(s, &dir), b" M docs/design.md\n");
        s.git(["add", "-A"]).from(&dir).succeeds();
        assert_eq!(
            s.git(["ls-files", "-z"]).from(&dir).succeeds().stdout,
            b".gitignore\0README.md\0docs/design.md\0"
        );
        assert_eq!(
            s.git(["log", "--format=%H"]).from(&dir).succeeds(),
            public_log
        );
        let fresh = s.dir().join("fresh");
        s.git(["clone", dir.to_str().unwrap(), fresh.to_str().unwrap()])
            .succeeds();
        for path in private_paths.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            assert!(!fresh.join(std::ffi::OsStr::from_bytes(path)).exists());
        }

        // Step 3: daily changes stay confined even from a public subdirectory.
        write(&dir, ".env.local", b"private changed environment\n");
        write(&dir, "notes/today.md", b"private today\n");
        write(&dir, ".vscode/launch.json", b"private launch\n");
        let status = succeeds(dupe(s, &dir, &["status", "--porcelain"]));
        assert_eq!(
            status,
            succeeds(direct(
                s,
                &dir,
                &[
                    "status",
                    "--porcelain",
                    "--untracked-files=normal",
                    "--ignored",
                    "--",
                    ":(top,literal).env.local",
                    ":(top,literal).gitdupe",
                    ":(top,literal).vscode",
                    ":(top,literal)docs/notes.md",
                    ":(top,literal)notes",
                ]
            ))
        );
        assert_eq!(
            status.stdout,
            b" M .env.local\n?? notes/today.md\n!! .vscode/launch.json\n"
        );
        assert_eq!(public_status(s, &dir), b"M  docs/design.md\n");
        let all = leaving_public_git(&dir, s.git(["dupe", "add", "-A"]).from(&dir.join("docs")));
        succeeds(all);
        assert_eq!(staged(s, &dir), b"M\0.env.local\0A\0notes/today.md\0");
        // The root spelling has the same scope; inside notes it cannot stage the env edit.
        s.private(&dir).git(["reset", "-q", "HEAD"]).succeeds();
        succeeds(dupe(s, &dir, &["add", "."]));
        assert_eq!(staged(s, &dir), b"M\0.env.local\0A\0notes/today.md\0");
        s.private(&dir).git(["reset", "-q", "HEAD"]).succeeds();
        succeeds(leaving_public_git(
            &dir,
            s.git(["dupe", "add", "."]).from(&dir.join("notes")),
        ));
        assert_eq!(staged(s, &dir), b"A\0notes/today.md\0");
        succeeds(dupe(s, &dir, &["add", "-A"]));
        succeeds(dupe(s, &dir, &["add", "-f", ".vscode/launch.json"]));
        assert_eq!(
            staged(s, &dir),
            b"M\0.env.local\0A\0.vscode/launch.json\0A\0notes/today.md\0"
        );
        let first = original.replace("private line 2\n", "private first change\n");
        let both = first.replace("private line 29\n", "private second change\n");
        write(&dir, "notes/a.md", both.as_bytes());
        succeeds(leaving_public_git(
            &dir,
            s.git(["dupe", "add", "-p"]).from(&dir).input(b"y\nn\n"),
        ));
        assert_eq!(
            s.private(&dir)
                .git(["show", ":notes/a.md"])
                .succeeds()
                .stdout,
            first.as_bytes()
        );
        assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), both.as_bytes());

        let previous = head(s, &dir);
        let expected = succeeds(direct(s, &dir, &["commit", "-m", "Daily work"]));
        let committed = head(s, &dir);
        s.private(&dir)
            .git(["reset", "--soft", &previous])
            .succeeds();
        assert_eq!(
            succeeds(dupe(s, &dir, &["commit", "-m", "Daily work"])),
            expected
        );
        assert_eq!(head(s, &dir), committed);
        // `blame` of a revision: the working tree's uncommitted lines carry the clock.
        for words in [
            &["diff"][..],
            &["log", "-p"],
            &["blame", "HEAD", "--", "notes/a.md"],
        ] {
            assert_eq!(
                succeeds(dupe(s, &dir, words)),
                succeeds(direct(s, &dir, words))
            );
        }
        let expected = succeeds(direct(s, &dir, &["restore", "notes/a.md"]));
        write(&dir, "notes/a.md", both.as_bytes());
        assert_eq!(
            succeeds(dupe(s, &dir, &["restore", "notes/a.md"])),
            expected
        );
        assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), first.as_bytes());
        write(&dir, "notes/a.md", both.as_bytes());
        let expected = succeeds(direct(s, &dir, &["stash"]));
        s.private(&dir).git(["stash", "pop"]).succeeds();
        assert_eq!(succeeds(dupe(s, &dir, &["stash"])), expected);
        assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), first.as_bytes());
        assert!(
            !s.private(&dir)
                .git(["stash", "list"])
                .succeeds()
                .stdout
                .is_empty()
        );

        let expected = succeeds(direct(s, &dir, &["switch", "-c", "experiment"]));
        s.private(&dir).git(["switch", "main"]).succeeds();
        s.private(&dir)
            .git(["branch", "-D", "experiment"])
            .succeeds();
        assert_eq!(
            succeeds(dupe(s, &dir, &["switch", "-c", "experiment"])),
            expected
        );
        assert_eq!(
            s.private(&dir)
                .git(["symbolic-ref", "HEAD"])
                .succeeds()
                .stdout,
            b"refs/heads/experiment\n"
        );
        let private_before = Tree::of(&dir.join(".git/dupe"));
        s.git(["switch", "-c", "public-experiment"])
            .from(&dir)
            .succeeds();
        unchanged(&private_before, &dir.join(".git/dupe"));
        write(&dir, "notes/a.md", b"private experiment\n");
        succeeds(dupe(s, &dir, &["add", "notes/a.md"]));
        succeeds(dupe(s, &dir, &["commit", "-m", "Experiment"]));
        let experiment = head(s, &dir);
        succeeds(dupe(s, &dir, &["switch", "main"]));
        let main = head(s, &dir);
        let expected = succeeds(direct(s, &dir, &["merge", "experiment"]));
        s.private(&dir).git(["reset", "--hard", &main]).succeeds();
        assert_eq!(succeeds(dupe(s, &dir, &["merge", "experiment"])), expected);
        assert_eq!(head(s, &dir), experiment);

        write(&dir, "src/scratch.py", b"public candidate\n");
        names(&public_status(s, &dir), b"?? src/scratch.py\n");
        let status = succeeds(dupe(s, &dir, &["status", "--porcelain"]));
        assert!(!holds(&status.stdout, b"src/scratch.py"));
        s.git(["add", "src/scratch.py"]).from(&dir).succeeds();
        let before = s
            .private(&dir)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let refused = dupe(s, &dir, &["add", "src/scratch.py"]);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names(refused.only_line("fatal"), b"git rm --cached");
        names(refused.only_line("fatal"), b"src/scratch.py");
        assert_eq!(
            s.private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            before
        );
        // Take the private choice on the same candidate after undoing public staging.
        s.git(["rm", "--cached", "src/scratch.py"])
            .from(&dir)
            .succeeds();
        succeeds(dupe(s, &dir, &["add", "src/scratch.py"]));
        assert!(!holds(&public_status(s, &dir), b"src/scratch.py"));
        let released = succeeds(dupe(s, &dir, &["rm", "--cached", "src/scratch.py"]));
        warnings_in_any_order(&released, &[&[b"src/scratch.py"]]);
        names(&public_status(s, &dir), b"?? src/scratch.py\n");
        write(&dir, "src/mine.py", b"private candidate\n");
        succeeds(dupe(s, &dir, &["add", "src/mine.py"]));
        names(
            &s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b"src/mine.py\0",
        );
        assert!(!holds(&public_status(s, &dir), b"src/mine.py"));

        let unhide = succeeds(dupe(s, &dir, &["unhide", "notes/"]));
        assert_eq!(unhide.lines("hint").len(), 1, "{unhide:?}");
        names(unhide.lines("hint")[0], b"notes");
        warnings_in_any_order(&unhide, &[&[b"notes"]]);
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b".vscode\n");
        let rules = region_rules(&dir);
        assert!(!rules.contains(&b"/notes".to_vec()));
        for path in [
            b"/notes/a.md".as_slice(),
            b"/notes/b.md",
            b"/notes/today.md",
        ] {
            assert!(rules.contains(&path.to_vec()), "{rules:?}");
        }
        write(&dir, "notes/public.md", b"public new note\n");
        let status = public_status(s, &dir);
        names(&status, b"?? notes/public.md\n");
        for path in [b"notes/a.md".as_slice(), b"notes/b.md", b"notes/today.md"] {
            assert!(!holds(&status, path));
        }
        assert!(!dir.join("scratch").exists());
        let hide = succeeds(dupe(s, &dir, &["hide", "scratch/"]));
        names(hide.only_line("hint"), b"scratch");
        names(hide.only_line("hint"), b"git dupe unhide");
        assert!(region_rules(&dir).contains(&b"/scratch".to_vec()));
        write(&dir, "scratch/later.md", b"private future scratch\n");
        assert!(!holds(&public_status(s, &dir), b"scratch/later.md"));
        write(&dir, ".gitdupe", b".vscode\nscratch\nmanual\n");
        write(&dir, "manual/later.md", b"private manual path\n");
        names(&public_status(s, &dir), b"?? manual/later.md\n");
        succeeds(dupe(s, &dir, &["status", "--porcelain"]));
        assert!(region_rules(&dir).contains(&b"/manual".to_vec()));
        assert!(!holds(&public_status(s, &dir), b"manual/later.md"));

        // Step 4: resetting the tree deletes build products and every other untracked
        // file, `notes/public.md` and the released `src/scratch.py` among them, and no
        // hidden path.
        write(&dir, "build/app.js", b"build product\n");
        write(&dir, "tmp.log", b"untracked\n");
        let before = Tree::working(&dir);
        succeeds(dupe(s, &dir, &["clean", "-fdx"]));
        assert_eq!(
            changed_since(&before, &dir),
            [
                "build",
                "build/app.js",
                "notes/public.md",
                "src/scratch.py",
                "tmp.log"
            ]
        );

        // Plain `git clean -fdx` from habit takes the private files with the rest; the
        // staged ones come back as last staged, and an edit never staged is gone.
        write(&dir, ".env.local", b"private staged environment\n");
        succeeds(dupe(s, &dir, &["add", ".env.local"]));
        write(&dir, ".env.local", b"private edit never staged\n");
        let tracked = s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout;
        let tracked: Vec<&[u8]> = tracked
            .split(|&byte| byte == 0)
            .filter(|path| !path.is_empty())
            .collect();
        s.git(["clean", "-fdx"]).from(&dir).succeeds();
        for path in &tracked {
            let path = dir.join(OsStr::from_bytes(path));
            assert!(
                !path.exists(),
                "{} survived plain git clean",
                path.display()
            );
        }
        let status = succeeds(dupe(s, &dir, &["status", "--porcelain"]));
        let records: Vec<&[u8]> = status.stdout.split(|&byte| byte == b'\n').collect();
        for path in &tracked {
            assert!(
                records
                    .iter()
                    .any(|record| record.get(1) == Some(&b'D') && record[3..] == **path),
                "{} not shown deleted: {status:?}",
                path.escape_ascii()
            );
        }
        succeeds(dupe(s, &dir, &["restore", "."]));
        for path in &tracked {
            let staged = [b":".as_slice(), path].concat();
            let staged = s
                .private(&dir)
                .git([
                    OsStr::new("cat-file"),
                    OsStr::new("blob"),
                    OsStr::from_bytes(&staged),
                ])
                .succeeds();
            assert_eq!(
                fs::read(dir.join(OsStr::from_bytes(path))).unwrap(),
                staged.stdout,
                "{}",
                path.escape_ascii()
            );
        }
        assert_eq!(
            fs::read(dir.join(".env.local")).unwrap(),
            b"private staged environment\n"
        );
        // Never staged: the hidden paths' untracked files, and `.gitdupe`'s line for
        // `manual`, written by hand.
        assert!(!dir.join("scratch/later.md").exists());
        assert!(!dir.join("manual/later.md").exists());
        assert_eq!(
            fs::read(dir.join(".gitdupe")).unwrap(),
            b".vscode\nscratch\n"
        );
    });
}
