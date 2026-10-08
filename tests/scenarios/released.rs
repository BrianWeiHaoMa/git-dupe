//! The warning of a path no longer hidden that public Git can see (G9), and the command it
//! names where there is one (G25): `git dupe hide` where `hide` would hide the path again
//! (G11, F5), a path beginning with `:` written `./<path>` so that `hide` reads it as that
//! path, and nothing where `hide` would refuse it, `.gitdupe` unreadable among the causes,
//! or read it as a usage error.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, holds, lines_in_order, private_add, private_commit, region_rules,
    under_each_release, write,
};

fn workspace(s: &Scenario) -> PathBuf {
    let dir = s.dir().join("workspace");
    s.repository(&dir);
    s.init(&dir);
    dir
}

fn committed_file(s: &Scenario, dir: &Path, path: &str) {
    write(dir, path, b"private\n");
    private_add(s, dir, path);
    private_commit(s, dir);
    s.git(["dupe", "status"]).from(dir).succeeds();
    assert!(
        region_rules(dir)
            .iter()
            .any(|rule| rule == format!("/{path}").as_bytes())
    );
}

/// Match the whole warning, so a refusal cannot accidentally acquire a recovery command.
fn released_warning<'a>(output: &'a Output, path: &str, hideable: bool) -> &'a [u8] {
    let beginning = format!("{path} is no longer hidden and is visible to public Git");
    let warnings = output.lines("warning");
    let found: Vec<_> = warnings
        .into_iter()
        .filter(|line| line.starts_with(beginning.as_bytes()))
        .collect();
    assert_eq!(found.len(), 1, "{path}: {output:?}");
    let expected = if hideable {
        // A leading `:` would be read as magic: the operand that resolves to the path.
        let operand = match path.strip_prefix(':') {
            Some(_) => format!("./{path}"),
            None => path.to_owned(),
        };
        format!("{beginning}; run from the root, 'git dupe hide -- {operand}' hides it again")
    } else {
        beginning
    };
    assert_eq!(found[0], expected.as_bytes(), "{output:?}");
    assert_eq!(holds(found[0], b"git dupe hide"), hideable, "{output:?}");
    found[0]
}

/// Type the warning's command as given, from the root it names, and check its effects.
fn follow_warning(s: &Scenario, dir: &Path, line: &[u8], path: &str) {
    let text = std::str::from_utf8(line).unwrap();
    let command = text.split('\'').nth(1).expect("a quoted recovery command");
    let mut words = command.split(' ');
    assert_eq!(words.next(), Some("git"), "{text}");
    assert!(command.starts_with("git dupe hide -- "), "{text}");
    s.git(words).from(dir).succeeds();
    assert_eq!(
        fs::read(dir.join(".gitdupe")).unwrap(),
        format!("{path}\n").as_bytes()
    );
    let public = s.git(["status", "--porcelain"]).from(dir).succeeds();
    assert!(public.stdout.is_empty(), "{public:?}");
}

#[test]
fn cached_removal_names_a_working_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        committed_file(s, &dir, "scratch.py");
        let output = s
            .git(["dupe", "rm", "--cached", "scratch.py"])
            .from(&dir)
            .succeeds();
        assert_eq!(output.lines("warning").len(), 1, "{output:?}");
        let line = released_warning(&output, "scratch.py", true);
        follow_warning(s, &dir, line, "scratch.py");
    });
}

#[test]
fn cached_removal_from_below_names_a_root_relative_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        committed_file(s, &dir, "sub/x.txt");
        let output = s
            .git(["dupe", "rm", "--cached", "x.txt"])
            .from(&dir.join("sub"))
            .succeeds();
        assert_eq!(output.lines("warning").len(), 1, "{output:?}");
        let line = released_warning(&output, "sub/x.txt", true);
        follow_warning(s, &dir, line, "sub/x.txt");
    });
}

#[test]
fn cached_removal_of_a_dash_path_names_a_working_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        committed_file(s, &dir, "-dash.txt");
        let output = s
            .git(["dupe", "rm", "--cached", "--", "-dash.txt"])
            .from(&dir)
            .succeeds();
        let line = released_warning(&output, "-dash.txt", true);
        follow_warning(s, &dir, line, "-dash.txt");
    });
}

#[test]
fn a_publicly_tracked_released_file_names_no_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        committed_file(s, &dir, "both.txt");
        s.git(["add", "-f", "both.txt"]).from(&dir).succeeds();
        s.commit_public(&dir);
        let output = s
            .git(["dupe", "rm", "--cached", "both.txt"])
            .from(&dir)
            .succeeds();
        released_warning(&output, "both.txt", false);
        assert!(
            output
                .lines("warning")
                .iter()
                .all(|line| !holds(line, b"git dupe hide")),
            "{output:?}"
        );
    });
}

#[test]
fn a_released_directory_with_a_publicly_tracked_child_names_no_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        s.git(["dupe", "hide", "notes"]).from(&dir).succeeds();
        private_commit(s, &dir);
        write(&dir, "notes/shared.md", b"public\n");
        s.git(["add", "-f", "notes/shared.md"])
            .from(&dir)
            .succeeds();
        s.commit_public(&dir);
        let output = s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        lines_in_order(&output, "hint", &[b"notes is no longer listed in .gitdupe"]);
        released_warning(&output, "notes", false);
    });
}

#[test]
fn a_released_star_path_names_no_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "star*.txt", b"private\n");
        s.git(["dupe", "git", "add", "--", ":(literal)star*.txt"])
            .from(&dir)
            .succeeds();
        private_commit(s, &dir);
        assert!(
            region_rules(&dir)
                .iter()
                .any(|rule| rule == b"/star\\*.txt")
        );
        let output = s
            .git(["dupe", "git", "rm", "--cached", "--", ":(literal)star*.txt"])
            .from(&dir)
            .succeeds();
        released_warning(&output, "star*.txt", false);
    });
}

#[test]
fn a_released_colon_path_names_hide_with_its_dot_slash_operand() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, ":colon.txt", b"private\n");
        s.git(["dupe", "git", "add", "--", ":(literal):colon.txt"])
            .from(&dir)
            .succeeds();
        private_commit(s, &dir);
        assert!(region_rules(&dir).iter().any(|rule| rule == b"/:colon.txt"));
        let output = s
            .git([
                "dupe",
                "git",
                "rm",
                "--cached",
                "--",
                ":(literal):colon.txt",
            ])
            .from(&dir)
            .succeeds();
        let line = released_warning(&output, ":colon.txt", true);
        follow_warning(s, &dir, line, ":colon.txt");
    });
}

#[test]
fn a_hand_edited_release_warns_once_with_a_working_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "scratch/todo.txt", b"private\n");
        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        assert!(region_rules(&dir).iter().any(|rule| rule == b"/scratch"));
        write(&dir, ".gitdupe", b"");
        let output = s.git(["dupe", "status"]).from(&dir).succeeds();
        let line = released_warning(&output, "scratch", true);
        let second = s.git(["dupe", "status"]).from(&dir).succeeds();
        assert!(!holds(&second.stderr, b"scratch"), "{second:?}");
        follow_warning(s, &dir, line, "scratch");
    });
}

#[test]
fn an_unreadable_gitdupe_blocks_the_released_directorys_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "scratch/todo.txt", b"private\n");
        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        private_commit(s, &dir);
        assert!(region_rules(&dir).iter().any(|rule| rule == b"/scratch"));
        // A symbolic link whose target is gone stands where the file stood: it lists
        // nothing, and `hide` cannot rewrite it.
        fs::remove_file(dir.join(".gitdupe")).unwrap();
        symlink("gone", dir.join(".gitdupe")).unwrap();
        let output = s.git(["dupe", "status"]).from(&dir).succeeds();
        released_warning(&output, "scratch", false);
        let refused = s.git(["dupe", "hide", "--", "scratch"]).from(&dir).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        let fatal = refused.lines("fatal");
        assert_eq!(fatal.len(), 1, "{refused:?}");
        assert!(
            holds(fatal[0], b".gitdupe cannot be read as a file"),
            "{refused:?}"
        );
    });
}

#[test]
fn a_public_only_gitdupe_blocks_the_released_directorys_hide_command() {
    under_each_release(|s| {
        let dir = workspace(s);
        write(&dir, "scratch/todo.txt", b"private\n");
        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        private_commit(s, &dir);
        s.git(["add", "-f", ".gitdupe"]).from(&dir).succeeds();
        s.commit_public(&dir);
        s.git(["dupe", "rm", "--cached", ".gitdupe"])
            .from(&dir)
            .succeeds();
        assert!(region_rules(&dir).iter().any(|rule| rule == b"/scratch"));
        assert!(
            s.private(&dir)
                .git(["ls-files", "--", ".gitdupe"])
                .succeeds()
                .stdout
                .is_empty()
        );
        assert_eq!(
            s.git(["ls-files", "--", ".gitdupe"])
                .from(&dir)
                .succeeds()
                .stdout,
            b".gitdupe\n"
        );
        write(&dir, ".gitdupe", b"");
        let output = s.git(["dupe", "status"]).from(&dir).succeeds();
        released_warning(&output, "scratch", false);
        let refused = s.git(["dupe", "hide", "--", "scratch"]).from(&dir).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        let fatal = refused.lines("fatal");
        assert_eq!(fatal.len(), 1, "{refused:?}");
        assert!(
            holds(fatal[0], b"'git rm --cached .gitdupe'"),
            "{refused:?}"
        );
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"");
    });
}
