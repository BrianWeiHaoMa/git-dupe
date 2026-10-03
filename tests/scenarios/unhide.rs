//! `git dupe unhide`: the lines of `.gitdupe` naming a path removed, every other line
//! kept byte for byte, the file staged privately, one hint per path given saying whether
//! a line named it and what still hides it, and the path named as visible when something
//! stands at it.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, holds, locate_words, private_add, region, region_rules,
    staged_gitdupe, under_each_release, usage_line, write,
};

/// A workspace attached by `git dupe init`, with a directory `sub` below its root.
fn init_attached(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    s.repository(&dir);
    s.init(&dir);
    fs::create_dir(dir.join("sub")).unwrap();
    dir
}

fn unhide(s: &Scenario, from: &Path, words: &[&str]) -> Output {
    s.git(["dupe", "unhide"].iter().chain(words))
        .from(from)
        .run()
}

fn gitdupe(dir: &Path) -> Vec<u8> {
    fs::read(dir.join(".gitdupe")).unwrap()
}

/// What a hint says of a path whose line was removed.
const REMOVED: &[u8] = b" is no longer listed in .gitdupe";
/// What a hint says of a path no line named.
const UNLISTED: &[u8] = b" is not listed in .gitdupe";

/// Exit 0, nothing on standard output, and one `hint:` per path given, in the order first
/// given, each beginning with its path and what became of its line, and none saying that
/// a path is visible or no longer hidden, which is settle's `warning:` to say (G9).
fn unhidden(output: &Output, hints: &[(&[u8], &[u8])]) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let found = output.lines("hint");
    assert_eq!(found.len(), hints.len(), "{output:?}");
    for (hint, (path, said)) in found.iter().zip(hints) {
        assert!(hint.starts_with(&[path, *said].concat()), "{output:?}");
        assert!(!holds(hint, b"visible"), "{output:?}");
        assert!(!holds(hint, b"no longer hidden"), "{output:?}");
    }
    assert!(output.lines("fatal").is_empty(), "{output:?}");
}

#[test]
fn a_listed_directory_is_released_and_its_privately_tracked_files_stay_hidden() {
    under_each_release(|s| {
        for below in [false, true] {
            let dir = init_attached(s, &format!("workspace-{below}"));
            // Hidden by a command, so that the region holds `notes` when `unhide` begins.
            let hidden = s.git(["dupe", "hide", "notes"]).from(&dir).run();
            assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
            write(&dir, "notes/today.md", b"today\n");
            private_add(s, &dir, "notes/today.md");
            write(&dir, "notes/scratch.md", b"scratch\n");
            let (from, operand) = if below {
                (dir.join("sub"), "../notes/")
            } else {
                (dir.clone(), "notes/")
            };

            let output = unhide(s, &from, &[operand]);
            unhidden(&output, &[(b"notes", REMOVED)]);
            // The directory is released; the file below it the private repository tracks
            // is not.
            assert!(
                holds(
                    output.lines("hint")[0],
                    b"the files below it that the private repository tracks stay hidden"
                ),
                "{output:?}"
            );
            let warnings = output.lines("warning");
            assert_eq!(warnings.len(), 1, "{output:?}");
            assert!(holds(warnings[0], b"notes"), "{output:?}");
            assert!(holds(warnings[0], b"visible to public Git"), "{output:?}");
            assert_eq!(gitdupe(&dir), b"");
            assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"");
            assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/notes/today.md"]);
            let public = s.git(["status", "--porcelain"]).from(&dir).run();
            assert_eq!(public.stdout, b"?? notes/\n", "{public:?}");
        }
    });
}

#[test]
fn only_lines_whose_cleaned_path_equals_a_given_path_are_removed() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        let lines = b"notes/\n\n/docs/x/\nnotes/a.md\n./scratch//\n-x\na\nb";
        write(&dir, ".gitdupe", lines);
        // The words, the paths their hints name with what became of each one's line, and
        // the file left.
        type Step<'a> = (&'a [&'a str], &'a [(&'a [u8], &'a [u8])], &'a [u8]);
        let steps: [Step; 6] = [
            (
                &["docs/x"],
                &[(b"docs/x", REMOVED)],
                b"notes/\n\nnotes/a.md\n./scratch//\n-x\na\nb",
            ),
            // Equality only: the line below `notes` stays.
            (
                &["notes"],
                &[(b"notes", REMOVED)],
                b"\nnotes/a.md\n./scratch//\n-x\na\nb",
            ),
            (
                &["scratch"],
                &[(b"scratch", REMOVED)],
                b"\nnotes/a.md\n-x\na\nb",
            ),
            (&["--", "-x"], &[(b"-x", REMOVED)], b"\nnotes/a.md\na\nb"),
            // One hint per path given, listed or not, in the order first given, each once.
            (
                &["b", "typo", "a", "b"],
                &[(b"b", REMOVED), (b"typo", UNLISTED), (b"a", REMOVED)],
                b"\nnotes/a.md\n",
            ),
            // A path that no line names changes nothing, and its hint says so.
            (
                &["typo", "notes/a.md/x", "notes"],
                &[
                    (b"typo", UNLISTED),
                    (b"notes/a.md/x", UNLISTED),
                    (b"notes", UNLISTED),
                ],
                b"\nnotes/a.md\n",
            ),
        ];
        for (words, paths, left) in steps {
            let output = unhide(s, &dir, words);
            unhidden(&output, paths);
            assert!(output.lines("warning").is_empty(), "{words:?}: {output:?}");
            assert_eq!(gitdupe(&dir), left, "{words:?}");
            assert_eq!(staged_gitdupe(s, &dir).unwrap(), left, "{words:?}");
        }
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/notes/a.md"]);
    });
}

/// Each path given is named with what still hides it, where something does: the private
/// repository tracking it, a listed path above it, or files below it that the private
/// repository tracks. A path with none of these is said only to be no longer listed, and
/// settle's warning names it where public Git can now see it. A path no line names gets
/// its hint too, with exit 0 and `.gitdupe` as it was, on disk and staged, whatever bytes
/// the path holds.
#[test]
fn each_path_given_is_named_with_what_still_hides_it() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".env", b"secret\n");
        private_add(s, &dir, ".env");
        write(&dir, "scratch/todo.txt", b"todo\n");
        write(
            &dir,
            ".gitdupe",
            b".env\nscratch\nnotes\nnotes/sub\ncaf\xe9\n",
        );
        private_add(s, &dir, ".gitdupe");
        // Settled, the region holds every listed path when `unhide` begins.
        s.git(["dupe", "status"]).from(&dir).succeeds();
        let stays: &[u8] = b"; it stays hidden, because the private repository tracks it";

        let output = unhide(s, &dir, &[".env"]);
        unhidden(&output, &[(b".env", REMOVED)]);
        assert_eq!(
            output.lines("hint")[0],
            [&b".env"[..], REMOVED, stays].concat(),
            "{output:?}"
        );
        assert!(output.lines("warning").is_empty(), "{output:?}");
        let left = b"scratch\nnotes\nnotes/sub\ncaf\xe9\n";
        assert_eq!(gitdupe(&dir), left);

        // The same path again, now listed nowhere: privately tracked, it stays hidden.
        let output = unhide(s, &dir, &[".env"]);
        unhidden(&output, &[(b".env", UNLISTED)]);
        assert!(holds(output.lines("hint")[0], stays), "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert_eq!(gitdupe(&dir), left);
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), left);

        let output = unhide(s, &dir, &["notes/sub"]);
        unhidden(&output, &[(b"notes/sub", REMOVED)]);
        let below = b"; it stays hidden below notes, which .gitdupe lists";
        assert!(holds(output.lines("hint")[0], below), "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");

        // Nothing still hides it: the hint says only that, and settle names it visible.
        let output = unhide(s, &dir, &["scratch"]);
        unhidden(&output, &[(b"scratch", REMOVED)]);
        assert_eq!(output.lines("hint")[0], [&b"scratch"[..], REMOVED].concat());
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        assert!(holds(warnings[0], b"scratch"), "{output:?}");
        assert!(holds(warnings[0], b"visible to public Git"), "{output:?}");

        let left = b"notes\ncaf\xe9\n";
        assert_eq!(gitdupe(&dir), left);
        let output = unhide(s, &dir, &["typo"]);
        unhidden(&output, &[(b"typo", UNLISTED)]);
        assert_eq!(
            output.lines("hint")[0],
            b"typo is not listed in .gitdupe; there is no line to remove"
        );
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert_eq!(gitdupe(&dir), left);
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), left);

        // A path that is not UTF-8 is named as its bytes, listed and then not.
        let word = OsStr::from_bytes(b"caf\xe9");
        for said in [REMOVED, UNLISTED] {
            let dupe = [OsStr::new("dupe"), OsStr::new("unhide"), word];
            let output = s.git(dupe).from(&dir).run();
            unhidden(&output, &[(b"caf\xe9", said)]);
            assert_eq!(gitdupe(&dir), b"notes\n");
            assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"notes\n");
        }
    });
}

#[test]
fn an_operand_is_read_from_the_users_directory_or_as_an_absolute_path() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\nsub/x\nsub/y\n");
        let root = s
            .git(["rev-parse", "--show-toplevel"])
            .from(&dir)
            .run()
            .stdout;
        let root = String::from_utf8(root).unwrap();
        let absolute = format!("{}/sub/./y/", root.trim_end());
        let output = unhide(s, &dir.join("sub"), &["../notes", "./x//", &absolute]);
        unhidden(
            &output,
            &[
                (b"notes", REMOVED),
                (b"sub/x", REMOVED),
                (b"sub/y", REMOVED),
            ],
        );
        assert_eq!(gitdupe(&dir), b"");
    });
}

#[test]
fn a_usage_error_is_reported_after_locating_and_settles_nothing() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "unhide"]).run();
        assert_eq!(help.end, End::Code(0), "{help:?}");
        let misuses: [&[&str]; 11] = [
            &[],
            &["--"],
            &["-f", "x"],
            &["x", "--force"],
            &["a*"],
            &["a?"],
            &["a[1]"],
            &[":/x"],
            &[":x"],
            &["."],
            &["../x"],
        ];
        // The word each error line names, as typed.
        let named = [
            None,
            None,
            Some("-f"),
            Some("--force"),
            Some("a*"),
            Some("a?"),
            Some("a[1]"),
            Some(":/x"),
            Some(":x"),
            Some("."),
            Some("../x"),
        ];

        let dir = init_attached(s, "attached");
        write(&dir, ".gitdupe", b"a*\nx\n");
        // Settle would warn of this `!` rule if it ran.
        write(&dir, ".gitignore", b"!x\n");
        let before = Tree::of(&dir);
        let from_below: [&[&str]; 3] = [&[".."], &["../.."], &["/"]];
        let at_root = misuses
            .iter()
            .zip(named)
            .map(|(words, named)| (*words, named, &dir));
        let sub = dir.join("sub");
        let below = from_below
            .iter()
            .map(|words| (*words, Some(words[0]), &sub));
        for (words, named, from) in at_root.chain(below) {
            let output = unhide(s, from, words);
            assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{words:?}: {output:?}");
            let line = output.line_then("error", usage_line(&help.stdout));
            if let Some(named) = named {
                assert!(holds(line, format!("'{named}'").as_bytes()), "{output:?}");
            }
            let changed = before.changed_in(&Tree::of(&dir));
            assert!(changed.is_empty(), "{words:?}: {changed:?}");
        }

        // Unattached, every one of them is the refusal naming `git dupe init`.
        let plain = s.dir().join("plain");
        s.repository(&plain);
        for words in misuses {
            let output = unhide(s, &plain, words);
            assert_eq!(output.end, End::Code(128), "{words:?}: {output:?}");
            assert!(holds(output.only_line("fatal"), b"git dupe init"));
        }
        assert!(!plain.join(".git/dupe").exists());
        assert!(region(&plain).is_none());

        // Outside any repository, Git's own message and status.
        let gits = s.git(locate_words(false)).run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        for words in misuses {
            let output = unhide(s, s.dir(), words);
            assert_eq!(output.end, gits.end, "{words:?}: {output:?}");
            assert_eq!(output.stderr, gits.stderr, "{words:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{words:?}: {output:?}");
        }
    });
}

#[test]
fn a_help_request_before_the_end_of_options_prints_the_text_anywhere() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "unhide"]).run();
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert!(help.stdout.starts_with(b"usage: git dupe unhide"));
        let dir = init_attached(s, "attached");
        write(&dir, ".gitdupe", b"x\n");
        let before = Tree::of(&dir);
        for place in [&dir, &dir.join("sub"), s.dir()] {
            for words in [
                &["-h"][..],
                &["--help"],
                &["x", "-h"],
                &["-f", "--help", "--"],
            ] {
                let output = unhide(s, place, words);
                assert_eq!(output, help, "{words:?}");
            }
        }
        assert!(before.changed_in(&Tree::of(&dir)).is_empty());
        // After `--`, `-h` is a path.
        write(&dir, ".gitdupe", b"-h\n");
        let output = unhide(s, &dir, &["--", "-h"]);
        unhidden(&output, &[(b"-h", REMOVED)]);
        assert_eq!(gitdupe(&dir), b"");
    });
}

#[test]
fn a_publicly_tracked_gitdupe_is_refused_unless_the_private_repository_tracks_it() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        let added = s.git(["add", "-f", "--", ".gitdupe"]).from(&dir).run();
        assert_eq!(added.end, End::Code(0), "{added:?}");
        // Settle may bring the region up to date; nothing else changes.
        let exclude = dir.join(".git/info/exclude");
        let before = Tree::of(&dir).without(&[&exclude]);
        for from in [&dir, &dir.join("sub")] {
            let output = unhide(s, from, &["notes"]);
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let refusal = output.lines("fatal");
            assert_eq!(refusal.len(), 1, "{output:?}");
            assert!(holds(refusal[0], b"git rm --cached .gitdupe"), "{output:?}");
            // Public Git tracks `.gitdupe`, so it does not ignore it (G6, G7).
            let warnings = output.lines("warning");
            assert_eq!(warnings.len(), 1, "{output:?}");
            assert!(holds(warnings[0], b".gitdupe"), "{output:?}");
            // The refusal and that warning are all of standard error: no hint of an
            // unhiding.
            let warning = [b"warning: ", warnings[0], b"\n"].concat();
            output.line_then("fatal", &warning);
            let changed = before.changed_in(&Tree::of(&dir).without(&[&exclude]));
            assert!(changed.is_empty(), "{changed:?}");
            assert_eq!(staged_gitdupe(s, &dir), None);
            assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/notes"]);
        }

        // Tracked by both: the command runs, and settle names `.gitdupe`.
        private_add(s, &dir, ".gitdupe");
        let output = unhide(s, &dir, &["notes"]);
        unhidden(&output, &[(b"notes", REMOVED)]);
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        assert!(holds(warnings[0], b".gitdupe"), "{output:?}");
        assert_eq!(gitdupe(&dir), b"");
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"");
    });
}

#[test]
fn a_gitdupe_missing_from_disk_is_rewritten_from_its_staged_lines() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\nscratch");
        private_add(s, &dir, ".gitdupe");
        fs::remove_file(dir.join(".gitdupe")).unwrap();

        // Nothing named: nothing is written or staged.
        let output = unhide(s, &dir, &["typo"]);
        unhidden(&output, &[(b"typo", UNLISTED)]);
        assert!(!dir.join(".gitdupe").exists());
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"notes\nscratch");

        let output = unhide(s, &dir, &["notes"]);
        unhidden(&output, &[(b"notes", REMOVED)]);
        assert_eq!(gitdupe(&dir), b"scratch");
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"scratch");
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/scratch"]);
    });
}

#[test]
fn a_rewrite_keeps_the_files_permissions_and_replaces_a_link_by_a_regular_file() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\nx\n");
        fs::set_permissions(dir.join(".gitdupe"), fs::Permissions::from_mode(0o600)).unwrap();
        let output = unhide(s, &dir, &["notes"]);
        unhidden(&output, &[(b"notes", REMOVED)]);
        assert!(output.lines("warning").is_empty(), "{output:?}");
        let found = fs::symlink_metadata(dir.join(".gitdupe")).unwrap();
        assert_eq!(found.permissions().mode() & 0o7777, 0o600);
        assert_eq!(gitdupe(&dir), b"x\n");

        // A link read through: nothing named leaves it as it is.
        fs::remove_file(dir.join(".gitdupe")).unwrap();
        write(&dir, "elsewhere/list", b"notes\nx\n");
        symlink("elsewhere/list", dir.join(".gitdupe")).unwrap();
        let output = unhide(s, &dir, &["typo"]);
        unhidden(&output, &[(b"typo", UNLISTED)]);
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert!(
            fs::symlink_metadata(dir.join(".gitdupe"))
                .unwrap()
                .is_symlink()
        );

        // Rewritten, it becomes a regular file; its target is untouched.
        let output = unhide(s, &dir, &["notes"]);
        unhidden(&output, &[(b"notes", REMOVED)]);
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        assert!(holds(warnings[0], b".gitdupe"), "{output:?}");
        assert!(
            fs::symlink_metadata(dir.join(".gitdupe"))
                .unwrap()
                .is_file()
        );
        assert_eq!(gitdupe(&dir), b"x\n");
        assert_eq!(fs::read(dir.join("elsewhere/list")).unwrap(), b"notes\nx\n");
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"x\n");
    });
}

#[test]
fn a_directory_at_gitdupe_is_refused_before_anything_is_written() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        fs::create_dir(dir.join(".gitdupe")).unwrap();
        write(&dir, ".gitdupe/inside", b"kept\n");
        let before = Tree::of(&dir);
        let output = unhide(s, &dir, &["x"]);
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let refusal = output.lines("fatal");
        assert_eq!(refusal.len(), 1, "{output:?}");
        assert!(holds(refusal[0], b".gitdupe"), "{output:?}");
        // Settle still runs, and names the entry that hides nothing.
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        assert!(holds(warnings[0], b".gitdupe"), "{output:?}");
        assert!(before.changed_in(&Tree::of(&dir)).is_empty());
    });
}

#[test]
fn a_failed_staging_run_ends_with_gits_status_and_a_rerun_stages_the_file() {
    under_each_release(|s| {
        let dir = init_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\nx\n");
        private_add(s, &dir, ".gitdupe");
        let lock = dir.join(".git/dupe/index.lock");
        fs::write(&lock, b"").unwrap();
        let gits = s.private(&dir).git(["add", "-f", "--", ".gitdupe"]).run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");

        let output = unhide(s, &dir, &["notes"]);
        assert_eq!(output.end, gits.end, "{output:?}");
        let hints = output.lines("hint");
        assert_eq!(hints.len(), 1, "{output:?}");
        assert!(holds(hints[0], b"notes"), "{output:?}");
        // The file is written, and the region follows it; the index is as it was.
        assert_eq!(gitdupe(&dir), b"x\n");
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/x"]);
        fs::remove_file(&lock).unwrap();
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"notes\nx\n");

        // The rerun changes no line, so its hint says no line named the path, and it
        // stages what was written.
        let output = unhide(s, &dir, &["notes"]);
        unhidden(&output, &[(b"notes", UNLISTED)]);
        assert_eq!(staged_gitdupe(s, &dir).unwrap(), b"x\n");
    });
}
