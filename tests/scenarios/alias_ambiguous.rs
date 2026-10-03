//! A word the supported Git releases read differently is refused where one of its
//! chains reaches a command git-dupe adds, changes, or guards, and passes through where
//! none does.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, Tree, daily_state, general_text, names, run_traced, unchanged,
    under_each_release, usage_line, write,
};

fn fixture(s: &Scenario) -> PathBuf {
    let dir = s.dir().join("workspace");
    daily_state(s, &dir);
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.private(&dir).git(["config", key, value]).succeeds();
    }
    s.git(["dupe", "status"]).from(&dir).succeeds();
    write(&dir, "README.md", b"public change\n");
    write(&dir, ".env.local", b"private change\n");
    write(&dir, "notes/new.md", b"new private file\n");
    write(&dir, "untracked", b"keep\n");
    dir
}

fn global_aliases(s: &Scenario, config: &[u8]) {
    write(s.dir(), "home/.gitconfig", config);
}

fn ambiguous(s: &Scenario, dir: &Path, typed: &str, alias: &str) {
    let help = general_text(s);
    let before = Tree::of(dir);
    let (output, runs) = run_traced(s.git(["dupe", typed]).from(dir), &s.dir().join("trace"));
    assert_eq!(output.end, End::Code(129), "{typed}: {output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.line_then("error", usage_line(&help));
    names(line, format!("'{alias}'").as_bytes());
    names(line, format!("'{typed}'").as_bytes());
    names(line, b"git dupe git");
    unchanged(&before, dir);
    assert_eq!(fs::read(dir.join("untracked")).unwrap(), b"keep\n");
    for output in [
        s.git(["diff", "--cached", "--name-only"])
            .from(dir)
            .succeeds(),
        s.private(dir)
            .git(["diff", "--cached", "--name-only"])
            .succeeds(),
        s.git(["stash", "list"]).from(dir).succeeds(),
        s.private(dir).git(["stash", "list"]).succeeds(),
    ] {
        assert!(output.stdout.is_empty(), "{output:?}");
    }
    // A release ignoring the alias form could leave the files alone too: the
    // usage lines and the trace prove that neither passthrough nor settle ran.
    let commands = runs.commands();
    assert_eq!(&commands[..2], &[b"dupe".as_slice(), b"rev-parse"]);
    assert!(commands[2..].contains(&b"config".as_slice()), "{runs:?}");
    assert!(
        commands[2..]
            .iter()
            .all(|word| *word == b"config" || word.is_empty()),
        "{runs:?}"
    );
}

#[test]
fn command_subsection_reaching_stash_is_a_usage_error() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias \"wipe\"]\n\tcommand = stash -u\n");
        ambiguous(s, &dir, "wipe", "wipe");
    });
}

#[test]
fn empty_alias_subsection_reaching_stash_is_a_usage_error() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias \"\"]\n\tw = stash -u\n");
        ambiguous(s, &dir, "w", "w");
    });
}

#[test]
fn dotted_alias_reaching_stash_is_a_usage_error() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias \"Foo\"]\n\tx = stash -u\n");
        ambiguous(s, &dir, "foo.x", "foo.x");
    });
}

#[test]
fn every_alias_record_is_checked_for_a_guarded_command() {
    under_each_release(|s| {
        let dir = fixture(s);
        for config in [
            b"[alias]\n\tw2 = log\n[alias \"w2\"]\n\tcommand = stash -u\n".as_slice(),
            b"[alias \"w2\"]\n\tcommand = stash -u\n[alias]\n\tw2 = log\n",
        ] {
            global_aliases(s, config);
            ambiguous(s, &dir, "w2", "w2");
        }
    });
}

#[test]
fn command_subsection_without_a_guarded_chain_keeps_gits_answer() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias \"lg\"]\n\tcommand = log --oneline\n");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "lg", "-1"])
            .run();
        let output = s.git(["dupe", "lg", "-1"]).from(&dir).run();
        assert_eq!(output.end, expected.end, "{output:?}, Git: {expected:?}");
        assert_eq!(output.stdout, expected.stdout);
        assert!(
            output.stderr.starts_with(&expected.stderr),
            "{output:?}, Git: {expected:?}"
        );
    });
}

#[test]
fn builtin_alias_reaching_add_is_a_usage_error_in_global_and_private_config() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias]\n\tlog = add -A\n");
        ambiguous(s, &dir, "log", "log");
        global_aliases(s, b"");
        s.private(&dir)
            .git(["config", "alias.log", "add -A"])
            .succeeds();
        ambiguous(s, &dir, "log", "log");
    });
}

#[test]
fn ambiguous_builtin_requires_an_attached_workspace_before_its_usage_error() {
    under_each_release(|s| {
        let dir = s.dir().join("unattached");
        s.repository(&dir);
        global_aliases(s, b"[alias]\n\tlog = add -A\n");
        let before = Tree::of(&dir);
        let output = s.git(["dupe", "log"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        names(output.only_line("fatal"), b"git dupe init");
        unchanged(&before, &dir);
        let outside = s.git(["dupe", "log"]).run();
        let literal = s.git(["dupe", "add", "-A"]).run();
        assert_eq!(outside, literal);
        assert_eq!(outside.end, End::Code(128), "{outside:?}");
    });
}

#[test]
fn builtin_alias_without_a_guarded_chain_runs_gits_own_log() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias]\n\tlog = show\n");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "log", "-1"])
            .run();
        let output = s.git(["dupe", "log", "-1"]).from(&dir).run();
        assert_eq!(output, expected);
    });
}

#[test]
fn deprecated_builtin_alias_reaching_stash_is_a_usage_error() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias]\n\twhatchanged = stash -u\n");
        ambiguous(s, &dir, "whatchanged", "whatchanged");
    });
}

#[test]
fn chain_to_a_deprecated_builtin_names_the_ambiguous_alias() {
    under_each_release(|s| {
        let dir = fixture(s);
        global_aliases(s, b"[alias]\n\ta = whatchanged\n\twhatchanged = add -A\n");
        ambiguous(s, &dir, "a", "whatchanged");
    });
}
