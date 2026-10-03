//! git-dupe's own lines whatever Git's color and locale settings: the same English bytes
//! on standard error, never colored, while Git's own output follows those settings and
//! every Git process git-dupe runs receives the caller's locale; and one line each
//! whatever the names they hold (F8, G25).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::symlink;

use crate::harness::{End, daily_state, holds, under_each_release, write};

/// A locale other than English, in every variable that selects one.
const GERMAN: [(&str, &str); 3] = [
    ("LANG", "de_DE.UTF-8"),
    ("LC_ALL", "de_DE.UTF-8"),
    ("LANGUAGE", "de"),
];

/// Color asked for everywhere Git reads it.
const COLORED: [&str; 6] = [
    "-c",
    "color.ui=always",
    "-c",
    "color.status=always",
    "-c",
    "color.advice=always",
];

#[test]
fn own_lines_are_the_same_uncolored_english_bytes_whatever_color_and_locale_say() {
    under_each_release(|s| {
        let plain = s.dir().join("plain");
        let set = s.dir().join("set");
        for dir in [&plain, &set] {
            daily_state(s, dir);
            write(dir, "notes/shared.md", b"public\n");
            s.git(["add", "-f", "notes/shared.md"]).from(dir).succeeds();
            s.commit_public(dir);
            // A `!` rule that settle names on every command.
            write(dir, ".gitignore", b"!notes\n");
        }
        // Each prints git-dupe's own lines alone on standard error: a refusal, a usage
        // error, a hint, and settle's warnings after each but the usage error.
        let commands: [&[&str]; 7] = [
            &["hide", "README.md"],
            &["add", "README.md"],
            &["add", "notes/"],
            &["status", "--porc"],
            &["hide", "scratch"],
            &["stash", "-u"],
            &["status"],
        ];
        for words in commands {
            let ours = s.git(["dupe"].iter().chain(words)).from(&plain).run();
            let mut colored = s
                .git(COLORED.iter().chain(&["dupe"]).chain(words))
                .from(&set);
            for (name, value) in GERMAN {
                colored = colored.variable(name, value);
            }
            let colored = colored.run();
            assert_eq!(colored.end, ours.end, "{words:?}: {colored:?}; {ours:?}");
            assert_eq!(
                colored.stderr, ours.stderr,
                "{words:?}: {colored:?}; {ours:?}"
            );
            assert!(!colored.stderr.contains(&0x1b), "{words:?}: {colored:?}");
            let levels = ["fatal", "error", "warning", "hint"];
            assert!(
                levels.iter().any(|level| !colored.lines(level).is_empty()),
                "{words:?}: {colored:?}"
            );
            // The wording the lines carry, in English under the German locale.
            let english: &[&[u8]] = match words {
                ["hide", "README.md"] => &[b"one file at a time", b"public Git does not ignore"],
                ["add", "README.md"] => &[
                    b"README.md is tracked by the project's Git",
                    b"git dupe add never stages such a path",
                    b"a deletion from the project",
                    b"every other clone",
                    b"public Git does not ignore",
                ],
                ["add", "notes/"] => &[
                    b"1 path the project's Git tracks was left unstaged",
                    b"one file at a time",
                    b"a deletion from the project",
                    b"every other clone",
                    b"public Git does not ignore",
                ],
                ["status", "--porc"] => &[b"git dupe git"],
                ["hide", "scratch"] => &[b"git dupe unhide", b"public Git does not ignore"],
                _ => &[b"public Git does not ignore"],
            };
            for phrase in english {
                assert!(holds(&colored.stderr, phrase), "{words:?}: {colored:?}");
            }
            if words == ["status"] {
                // Git's own status follows the color it was asked for.
                assert_eq!(ours.end, End::Code(0), "{ours:?}");
                assert!(colored.stdout.contains(&0x1b), "{colored:?}");
                assert!(!ours.stdout.contains(&0x1b), "{ours:?}");
            }
        }

        // The locale the caller set reaches the Git process git-dupe runs, unchanged.
        s.private(&set)
            .git([
                "config",
                "alias.locale",
                r#"!printf '%s|%s|%s' "$LANG" "$LC_ALL" "$LANGUAGE""#,
            ])
            .succeeds();
        let mut ours = s.git(["dupe", "locale"]).from(&set);
        for (name, value) in GERMAN {
            ours = ours.variable(name, value);
        }
        let ours = ours.run();
        assert_eq!(ours.end, End::Code(0), "{ours:?}");
        assert_eq!(ours.stdout, b"de_DE.UTF-8|de_DE.UTF-8|de", "{ours:?}");
    });
}

/// The file of a deciding rule lies where E4 does not reach and its name may hold a
/// newline: the warning naming it stays one line, the name quoted as Git quotes a path
/// that needs it (F8, G6). A region that cannot be maintained (G6) leaves the rule of a
/// global excludes file deciding for a hidden path.
#[test]
fn a_rules_file_whose_name_holds_a_newline_is_named_on_one_line() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        let rules = s.dir().join("global\nignore");
        fs::write(&rules, b"!secret\n").unwrap();
        let key = [OsStr::new("config"), OsStr::new("core.excludesFile")];
        s.git(key.into_iter().chain([rules.as_os_str()]))
            .from(&dir)
            .succeeds();
        let moved = s.dir().join("moved-info");
        fs::rename(dir.join(".git/info"), &moved).unwrap();
        symlink(&moved, dir.join(".git/info")).unwrap();
        write(&dir, "secret", b"private\n");
        write(&dir, ".gitdupe", b"secret\n");

        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        for line in output.stderr.split_inclusive(|&byte| byte == b'\n') {
            assert!(line.starts_with(b"warning: "), "{output:?}");
        }
        let exposed: Vec<&[u8]> = output
            .lines("warning")
            .into_iter()
            .filter(|line| line.starts_with(b"secret "))
            .collect();
        assert_eq!(exposed.len(), 1, "{output:?}");
        assert!(holds(exposed[0], b"global\\nignore\":1"), "{output:?}");
    });
}
