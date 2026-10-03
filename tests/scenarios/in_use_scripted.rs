//! The product's `In use` steps 6 to 8, in order, on one workspace: an agent and a
//! script driving git-dupe without a terminal, and the unguarded route.
//! Step 6 starts from a workspace in daily use.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::harness::{
    End, Scenario, Tree, changed_since, daily_state, holds, leaving_public_git, names,
    region_rules, run_traced, staged_gitdupe, unchanged, under_each_release, usage_line, write,
};

fn usage_error(s: &Scenario, dir: &Path, command: &str, options: &[&str], word: &[u8]) {
    let help = leaving_public_git(dir, s.git(["dupe", command, "-h"]).from(dir));
    assert_eq!(help.end, End::Code(0), "{help:?}");
    let before = Tree::of(dir);
    let (output, runs) = run_traced(
        s.git(["dupe", command].into_iter().chain(options.iter().copied()))
            .from(dir),
        &s.dir().join("usage-trace"),
    );
    assert_eq!(output.end, End::Code(129), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let error = output.line_then("error", usage_line(&help.stdout));
    names(error, word);
    names(error, b"git dupe git");
    assert_eq!(runs.commands(), [b"dupe".as_slice()], "{runs:?}");
    assert_eq!(runs.count(), 1, "{runs:?}");
    unchanged(&before, dir);
}

#[test]
fn agent_then_script_then_unguarded_git_walk() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let dir = s.dir().join("workspace");
        daily_state(s, &dir);
        let settled = leaving_public_git(&dir, s.git(["dupe", "status"]).from(&dir));
        assert_eq!(settled.end, End::Code(0), "{settled:?}");

        // 6: ignored ancestor, staged deletion still on disk, and daily private/public edits.
        write(&dir, "build/cache/item", b"cache\n");
        write(&dir, "build/public-product", b"build\n");
        let hidden = leaving_public_git(&dir, s.git(["dupe", "hide", "build/cache"]).from(&dir));
        assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
        let removed = leaving_public_git(
            &dir,
            s.git(["dupe", "rm", "--cached", "docs/notes.md"])
                .from(&dir),
        );
        assert_eq!(removed.end, End::Code(0), "{removed:?}");
        assert_eq!(fs::read(dir.join("docs/notes.md")).unwrap(), b"private\n");
        write(&dir, "notes/a.md", b"agent change\n");
        write(&dir, "notes/today.md", b"plan\n");
        write(&dir, ".vscode/launch.json", b"ignored private\n");
        write(&dir, "public-untracked", b"public\n");
        write(&dir, "src/scratch.py", b"public scratch\n");
        let declarations = fs::read(dir.join(".gitdupe")).unwrap();
        assert_eq!(declarations, b"notes\n.vscode\nbuild/cache\n");
        assert_eq!(staged_gitdupe(s, &dir), Some(declarations.clone()));
        let tracked = s.private(&dir).git(["ls-files", "-z"]).succeeds();
        let hidden_paths: Vec<&[u8]> = declarations
            .split(|&byte| byte == b'\n')
            .chain(tracked.stdout.split(|&byte| byte == 0))
            .chain([b".gitdupe".as_slice()])
            .filter(|path| !path.is_empty())
            .collect();
        let status = leaving_public_git(&dir, s.git(["dupe", "status", "--porcelain"]).from(&dir));
        assert_eq!(status.end, End::Code(0), "{status:?}");
        assert!(status.stderr.is_empty(), "{status:?}");
        let records: Vec<&[u8]> = status
            .stdout
            .strip_suffix(b"\n")
            .expect("porcelain records end in a newline")
            .split(|&byte| byte == b'\n')
            .collect();
        for record in &records {
            assert!(record.len() > 3 && record[2] == b' ', "{status:?}");
            let code = &record[..2];
            assert!(
                code == b"??"
                    || code == b"!!"
                    || code.iter().all(|byte| b" MADRCUT".contains(byte)),
                "{status:?}"
            );
            assert_ne!(code, b"  ", "{status:?}");
            let path = &record[3..];
            assert!(
                hidden_paths.iter().any(|hidden| {
                    path == *hidden
                        || path
                            .strip_prefix(*hidden)
                            .is_some_and(|rest| rest.starts_with(b"/"))
                }) || path == b"docs/notes.md"
                    || *record == b"!! build/",
                "outside the hidden paths: {status:?}"
            );
        }
        for record in [
            b"M  .gitdupe".as_slice(),
            b" M notes/a.md",
            b"?? notes/today.md",
            b"!! .vscode/launch.json",
            b"D  docs/notes.md",
            b"?? docs/notes.md",
            b"!! build/",
        ] {
            assert!(
                records.contains(&record),
                "missing {}: {status:?}",
                record.escape_ascii()
            );
        }
        assert_eq!(
            records
                .iter()
                .filter(|record| **record == b"!! build/")
                .count(),
            1
        );
        let add = leaving_public_git(&dir, s.git(["dupe", "add", "."]).from(&dir));
        assert_eq!(add.end, End::Code(0), "{add:?}");
        let staged = s
            .private(&dir)
            .git(["diff", "--cached", "--name-only", "-z"])
            .succeeds();
        assert_eq!(
            staged.stdout,
            b".gitdupe\0docs/notes.md\0notes/a.md\0notes/today.md\0"
        );
        let head = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds();
        let commit = leaving_public_git(&dir, s.git(["dupe", "commit", "-m", "Plan"]).from(&dir));
        assert_eq!(commit.end, End::Code(0), "{commit:?}");
        assert_ne!(
            s.private(&dir).git(["rev-parse", "HEAD"]).succeeds().stdout,
            head.stdout
        );
        assert_eq!(
            s.private(&dir)
                .git(["log", "-1", "--format=%s"])
                .succeeds()
                .stdout,
            b"Plan\n"
        );
        assert_eq!(
            s.private(&dir)
                .git(["show", "HEAD:notes/today.md"])
                .succeeds()
                .stdout,
            b"plan\n"
        );
        // Between runs, with no terminal: what the run built and every other untracked
        // file goes, the released `docs/notes.md` among them; every hidden path stays.
        write(&dir, "build/run/output.log", b"run\n");
        let before = Tree::working(&dir);
        let cleaned = leaving_public_git(&dir, s.git(["dupe", "clean", "-fdx"]).from(&dir));
        assert_eq!(cleaned.end, End::Code(0), "{cleaned:?}");
        assert!(cleaned.stderr.is_empty(), "{cleaned:?}");
        assert_eq!(
            changed_since(&before, &dir),
            [
                "build/public-product",
                "build/run",
                "build/run/output.log",
                "docs/notes.md",
                "public-untracked",
                "src",
                "src/scratch.py"
            ]
        );
        let before = Tree::of(&dir);
        let refusal = leaving_public_git(&dir, s.git(["dupe", "stash", "-u"]).from(&dir));
        assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
        assert!(refusal.stdout.is_empty(), "{refusal:?}");
        let fatal = refusal.only_line("fatal");
        names(fatal, b"git dupe add");
        names(fatal, b"git dupe stash");
        let text = String::from_utf8_lossy(fatal);
        assert!(text.find("git dupe add").unwrap() < text.find("git dupe stash").unwrap());
        unchanged(&before, &dir);
        usage_error(s, &dir, "status", &["--porcelian"], b"--porcelian");
        usage_error(
            s,
            &dir,
            "add",
            &["--pathspec-from-file=list"],
            b"--pathspec-from-file=list",
        );
        let quiet = leaving_public_git(&dir, s.git(["dupe", "status", "--porcelain"]).from(&dir));
        let ignore = fs::read(dir.join(".gitignore")).unwrap();
        fs::write(
            dir.join(".gitignore"),
            [ignore.as_slice(), b"!.env.local\n"].concat(),
        )
        .unwrap();
        let warned = leaving_public_git(&dir, s.git(["dupe", "status", "--porcelain"]).from(&dir));
        assert_eq!(warned.end, quiet.end);
        assert_eq!(warned.stdout, quiet.stdout);
        names(warned.only_line("warning"), b".env.local");
        fs::write(dir.join(".gitignore"), ignore).unwrap();

        write(&dir, "notes/a.md", b"editor change\n");
        let add = leaving_public_git(&dir, s.git(["dupe", "add", "."]).from(&dir));
        assert_eq!(add.end, End::Code(0), "{add:?}");
        let editor = s.dir().join("editor");
        let record = s.dir().join("editor-record");
        write(s.dir(), "editor", b"#!/bin/sh\nprintf '%s\\n' \"$GIT_TERMINAL_PROMPT\" \"$GIT_EDITOR\" \"$0\" \"$#\" \"$1\" > \"$EDITOR_RECORD\"\nprintf 'From editor\\n' > \"$1\"\n");
        fs::set_permissions(&editor, fs::Permissions::from_mode(0o755)).unwrap();
        let commit = leaving_public_git(
            &dir,
            s.git(["dupe", "commit"])
                .from(&dir)
                .variable("GIT_EDITOR", &editor)
                .variable("GIT_TERMINAL_PROMPT", "0")
                .variable("EDITOR_RECORD", &record),
        );
        assert_eq!(commit.end, End::Code(0), "{commit:?}");
        assert_eq!(
            fs::read(record).unwrap(),
            format!(
                "0\n{}\n{}\n1\n{}\n",
                editor.display(),
                editor.display(),
                dir.join(".git/dupe/COMMIT_EDITMSG").display()
            )
            .as_bytes()
        );
        assert_eq!(
            s.private(&dir)
                .git(["log", "-1", "--format=%s"])
                .succeeds()
                .stdout,
            b"From editor\n"
        );

        // 7: filesystem attachment discovery, machine-readable paths, Git's exits and history.
        assert!(dir.join(".git/dupe").is_dir());
        let clone = s.dir().join("public-clone");
        s.git([
            OsStr::new("clone"),
            OsStr::new("-q"),
            dir.as_os_str(),
            clone.as_os_str(),
        ])
        .succeeds();
        assert!(!clone.join(".git/dupe").exists());
        let declarations = fs::read(dir.join(".gitdupe")).unwrap();
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "ls-files", "-z"])
            .run();
        let listed = leaving_public_git(
            &dir,
            s.git([
                OsStr::new("-C"),
                dir.as_os_str(),
                OsStr::new("dupe"),
                OsStr::new("ls-files"),
                OsStr::new("-z"),
            ])
            .from(s.dir()),
        );
        assert_eq!(listed, expected);
        assert_eq!(listed.end, End::Code(0));
        assert!(holds(&listed.stdout, b"notes/today.md\0"));
        let allowed: Vec<Vec<u8>> = declarations
            .split(|&byte| byte == b'\n')
            .chain(listed.stdout.split(|&byte| byte == 0))
            .chain([b".gitdupe".as_slice()])
            .filter(|path| !path.is_empty())
            .map(|path| [b"/", path].concat())
            .collect();
        let rules = region_rules(&dir);
        assert!(!rules.is_empty());
        assert!(rules.iter().all(|rule| allowed.contains(rule)), "{rules:?}");
        write(&dir, "notes/a.md", b"script change\n");
        for (words, end) in [
            (&["diff", "--quiet"][..], End::Code(1)),
            (
                &["rev-parse", "--verify", "missing-private-name"][..],
                End::Code(128),
            ),
        ] {
            let expected = s
                .private(&dir)
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .run();
            let output = leaving_public_git(
                &dir,
                s.git(["dupe"].into_iter().chain(words.iter().copied()))
                    .from(&dir),
            );
            assert_eq!(output, expected);
            assert_eq!(output.end, end);
        }
        let log = leaving_public_git(
            &dir,
            s.git([
                OsStr::new("-C"),
                dir.as_os_str(),
                OsStr::new("dupe"),
                OsStr::new("log"),
            ])
            .from(s.dir()),
        );
        let git_dir = format!("--git-dir={}", dir.join(".git/dupe").display());
        let direct = s.git([git_dir.as_str(), "log"]).from(s.dir()).succeeds();
        assert_eq!(log, direct);

        // 8: unknown options and interactive add reach unguarded Git verbatim, at EOF.
        let words = ["status", "--porcelain=v2", "--newoption"];
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0"].into_iter().chain(words))
            .run();
        // A line added by hand: the unguarded route settles too, though Git refuses.
        let gitdupe = fs::read(dir.join(".gitdupe")).unwrap();
        write(
            &dir,
            ".gitdupe",
            &[gitdupe.as_slice(), b"by-hand\n"].concat(),
        );
        assert!(!region_rules(&dir).contains(&b"/by-hand".to_vec()));
        let output = leaving_public_git(
            &dir,
            s.git(["dupe", "git"].into_iter().chain(words)).from(&dir),
        );
        assert_eq!(output, expected);
        assert!(region_rules(&dir).contains(&b"/by-hand".to_vec()));
        usage_error(
            s,
            &dir,
            "status",
            &["--porcelain=v2", "--newoption"],
            b"--newoption",
        );
        // At end of input `add -i` stages nothing, so both runs start from one index.
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "add", "-i"])
            .run();
        let expected_index = s
            .private(&dir)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let output = leaving_public_git(&dir, s.git(["dupe", "git", "add", "-i"]).from(&dir));
        assert_eq!(output, expected);
        assert_eq!(
            s.private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            expected_index
        );
        assert_eq!(
            fs::read(dir.join("notes/a.md")).unwrap(),
            b"script change\n"
        );
    });
}
