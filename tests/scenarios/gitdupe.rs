//! `.gitdupe` as the privately tracked file it is: staged by `hide`, `unhide`, and `add`
//! as Git's `add` stages any file, merged as Git merges any file, and the hidden paths
//! following what stands on disk, whatever line endings or byte order mark a checkout or
//! an editor gave it (F5).

use std::fs;

use crate::harness::{
    End, daily_state, gitdupe_written_and_staged, names, private_commit, public_status,
    region_rules, staged_gitdupe, under_each_release, write,
};

/// While Git assumes `.gitdupe`'s index entry unchanged, `hide`, `add`, and `unhide`
/// write the file, the region follows it, and the staged version stays as Git's `add`
/// leaves such an entry; once Git looks at the entry again, `add` stages the file.
#[test]
fn an_entry_git_assumes_unchanged_stays_staged_as_it_was_while_the_file_hides() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        let staged = b"notes\n.vscode\n";
        s.git(["dupe", "update-index", "--assume-unchanged", ".gitdupe"])
            .from(&dir)
            .succeeds();

        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        write(&dir, "tools/run.sh", b"private\n");
        s.git(["dupe", "add", "tools"]).from(&dir).succeeds();
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        let written = b".vscode\nscratch\ntools\n";
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), written);
        assert_eq!(staged_gitdupe(s, &dir).as_deref(), Some(&staged[..]));
        // `notes` stays hidden through its privately tracked file alone.
        assert_eq!(
            region_rules(&dir),
            [
                b"/.env.local".as_slice(),
                b"/.gitdupe",
                b"/.vscode",
                b"/docs/notes.md",
                b"/notes/a.md",
                b"/scratch",
                b"/tools",
            ]
        );

        s.git(["dupe", "update-index", "--no-assume-unchanged", ".gitdupe"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "add", ".gitdupe"]).from(&dir).succeeds();
        gitdupe_written_and_staged(s, &dir, written);
    });
}

/// Two branches that each hide a path merge as Git merges any privately tracked file, and
/// the merged `.gitdupe` decides the hidden paths.
#[test]
fn a_merge_of_two_branches_hiding_paths_is_gits_and_hides_what_the_merged_file_lists() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        s.git(["dupe", "switch", "-q", "-c", "unhiding"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        private_commit(s, &dir);
        s.git(["dupe", "switch", "-q", "main"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        private_commit(s, &dir);

        s.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "dupe",
            "merge",
            "-q",
            "--no-edit",
            "unhiding",
        ])
        .from(&dir)
        .succeeds();
        gitdupe_written_and_staged(s, &dir, b".vscode\nscratch\n");
        assert_eq!(
            region_rules(&dir),
            [
                b"/.env.local".as_slice(),
                b"/.gitdupe",
                b"/.vscode",
                b"/docs/notes.md",
                b"/notes/a.md",
                b"/scratch",
            ]
        );
    });
}

/// With `core.autocrlf=true` in the global configuration, as Git for Windows's installer
/// ships it, the `.gitdupe` a second machine's `clone` checks out has CRLF line endings
/// and hides what it listed (`Done when`, F5, `In use` 12).
#[test]
fn a_gitdupe_checked_out_with_crlf_line_endings_hides_what_it_listed() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[core]\n\tautocrlf = true\n",
        )
        .unwrap();

        let second = &m.root;
        s.git([
            "dupe".as_ref(),
            "clone".as_ref(),
            m.first.private_remote.as_os_str(),
        ])
        .from(second)
        .succeeds();
        assert_eq!(
            fs::read(second.join(".gitdupe")).unwrap(),
            b"notes\r\n.vscode\r\n"
        );
        // The rule of the path listed, and none with a carriage return in brackets.
        let rules = region_rules(second);
        assert!(rules.contains(&b"/notes".to_vec()), "{rules:?}");
        assert!(!rules.iter().any(|rule| rule.contains(&b'[')), "{rules:?}");

        write(second, "notes/x.md", b"new\n");
        assert!(
            !public_status(s, second)
                .windows(5)
                .any(|name| name == b"notes")
        );
        let status = s
            .git(["dupe", "status", "--porcelain"])
            .from(second)
            .succeeds();
        assert!(
            status
                .stdout
                .split(|&byte| byte == b'\n')
                .any(|line| line == b"?? notes/x.md"),
            "{status:?}"
        );
    });
}

/// A `.gitdupe` an editor saved with a UTF-8 byte order mark hides what it lists, and
/// `hide` and `unhide` keep the mark and every other line.
#[test]
fn a_utf_8_byte_order_mark_is_read_past_and_kept() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"\xef\xbb\xbfnotes\n");
        let status = s.git(["dupe", "status"]).from(&dir).succeeds();
        assert!(status.lines("warning").is_empty(), "{status:?}");
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/notes"]);

        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        assert_eq!(
            fs::read(dir.join(".gitdupe")).unwrap(),
            b"\xef\xbb\xbfnotes\nscratch\n"
        );
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        gitdupe_written_and_staged(s, &dir, b"\xef\xbb\xbfscratch\n");
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..], b"/scratch"]);
    });
}

/// `hide` and `unhide` leave a file saved with CRLF line endings as it was but for the line
/// they add or remove, which `hide` ends with a newline alone.
#[test]
fn an_edit_keeps_every_other_line_of_a_crlf_file_byte_for_byte() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"notes\r\nscratch\r\n");
        s.git(["dupe", "hide", "tools"]).from(&dir).succeeds();
        assert_eq!(
            fs::read(dir.join(".gitdupe")).unwrap(),
            b"notes\r\nscratch\r\ntools\n"
        );
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        assert_eq!(
            fs::read(dir.join(".gitdupe")).unwrap(),
            b"scratch\r\ntools\n"
        );
        assert_eq!(
            region_rules(&dir),
            [&b"/.gitdupe"[..], b"/scratch", b"/tools"]
        );
    });
}

/// A `.gitdupe` beginning with a UTF-16 byte order mark, as Windows PowerShell 5.1's `>`
/// writes one, lists nothing: every command warns of it, public Git sees what it meant to
/// hide, and `hide`, `unhide`, and a hiding `add` refuse naming the cause, changing nothing.
#[test]
fn a_utf_16_byte_order_mark_lists_nothing_is_warned_of_and_refuses_an_edit() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let bytes = b"\xff\xfes\0c\0r\0a\0t\0c\0h\0\r\0\n\0";
        write(&dir, ".gitdupe", bytes);

        let status = s.git(["dupe", "status"]).from(&dir).succeeds();
        let warned = |output: &crate::harness::Output| {
            let warnings = output.lines("warning");
            assert!(
                warnings.iter().any(|line| line.starts_with(b".gitdupe ")
                    && line.windows(6).any(|part| part == b"UTF-16")),
                "{output:?}"
            );
        };
        warned(&status);
        assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..]]);
        write(&dir, "scratch/a.md", b"seen\n");
        assert!(
            public_status(s, &dir)
                .windows(10)
                .any(|name| name == b"scratch/a."),
            "public Git does not see what the file meant to hide"
        );

        write(&dir, "tools/run.sh", b"private\n");
        for words in [
            &["dupe", "hide", "x"][..],
            &["dupe", "unhide", "x"],
            &["dupe", "add", "tools"],
        ] {
            let refused = s.git(words).from(&dir).run();
            assert_eq!(refused.end, End::Code(128), "{words:?}: {refused:?}");
            let fatal = refused.lines("fatal");
            assert_eq!(fatal.len(), 1, "{refused:?}");
            names(fatal[0], b"UTF-16");
            warned(&refused);
            assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), bytes);
            assert_eq!(region_rules(&dir), [&b"/.gitdupe"[..]]);
        }
        assert_eq!(staged_gitdupe(s, &dir), None);
    });
}

/// A path whose bytes begin as a byte order mark does stays that path, in the first line
/// `hide` writes and in the line `unhide` leaves first.
#[test]
fn a_path_beginning_as_a_mark_stays_that_path_as_the_first_line() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let marked = "\u{feff}scratch";
        let rule = [b"/", marked.as_bytes()].concat();
        s.git(["dupe", "hide", marked]).from(&dir).succeeds();
        assert_eq!(region_rules(&dir), [b"/.gitdupe".to_vec(), rule.clone()]);

        s.git(["dupe", "hide", "notes"]).from(&dir).succeeds();
        s.git(["dupe", "unhide", marked]).from(&dir).succeeds();
        s.git(["dupe", "hide", marked]).from(&dir).succeeds();
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        assert_eq!(region_rules(&dir), [b"/.gitdupe".to_vec(), rule]);
    });
}
