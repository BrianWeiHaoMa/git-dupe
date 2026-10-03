//! A path the project's Git tracks, and the route that makes it private, which the
//! refusals of `add` and `hide` and `add`'s count of skipped paths name (G11, G14, N9):
//! `git rm --cached` first, a deletion the project's other clones receive, then
//! `git dupe add`.

use std::fs;

use crate::harness::{End, Scenario, names, names_the_route_to_private, under_each_release, write};

/// The refusal of a publicly tracked file operand, then the commands it gives, typed
/// where it says: the file ends privately tracked, its deletion staged in the project, and
/// on disk as it was. Where the project's `.gitignore` names the file, the line's
/// `git dupe add` is refused by Git and its `git dupe add -f` takes the file (G15).
fn refused_then_made_private(s: &Scenario, ignored: bool) {
    let dir = s.dir().join("project");
    s.attached_project(&dir);
    if ignored {
        let mut rules = fs::read(dir.join(".gitignore")).unwrap();
        rules.extend_from_slice(b"README.md\n");
        write(&dir, ".gitignore", &rules);
        s.git(["add", ".gitignore"]).from(&dir).succeeds();
        s.commit_public(&dir);
    }
    let content = fs::read(dir.join("README.md")).unwrap();
    let index = s
        .git(["dupe", "ls-files", "-s"])
        .from(&dir)
        .succeeds()
        .stdout;
    let public = s
        .git(["status", "--porcelain"])
        .from(&dir)
        .succeeds()
        .stdout;
    assert!(public.is_empty());

    let output = s.git(["dupe", "add", "README.md"]).from(&dir).run();
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let fatal = output.only_line("fatal");
    names(fatal, b"README.md is tracked by the project's Git");
    names(fatal, b"git dupe add never stages such a path");
    names(fatal, b"run from the root");
    names_the_route_to_private(fatal);
    assert_eq!(
        s.git(["dupe", "ls-files", "-s"])
            .from(&dir)
            .succeeds()
            .stdout,
        index
    );
    assert_eq!(
        s.git(["status", "--porcelain"])
            .from(&dir)
            .succeeds()
            .stdout,
        public
    );

    // The quoted commands, typed from the root as the refusal says.
    let text = std::str::from_utf8(fatal)
        .unwrap()
        .split_once("run from the root, ")
        .unwrap()
        .1;
    let commands: Vec<Vec<&str>> = text
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(|command| command.split(' ').collect())
        .collect();
    assert_eq!(
        commands,
        [
            vec!["git", "rm", "--cached", "--", "README.md"],
            vec!["git", "dupe", "add", "--", "README.md"],
            vec!["git", "dupe", "add", "-f", "--", "README.md"],
        ],
        "{output:?}"
    );
    let typed = |command: &[&str]| s.git(&command[1..]).from(&dir).run();
    assert_eq!(typed(&commands[0]).end, End::Code(0));
    let added = typed(&commands[1]);
    if ignored {
        assert_ne!(added.end, End::Code(0), "{added:?}");
        assert!(
            s.git(["dupe", "ls-files"])
                .from(&dir)
                .succeeds()
                .stdout
                .is_empty()
        );
        assert_eq!(typed(&commands[2]).end, End::Code(0));
    } else {
        assert_eq!(added.end, End::Code(0), "{added:?}");
    }
    assert_eq!(
        s.git(["status", "--porcelain"])
            .from(&dir)
            .succeeds()
            .stdout,
        b"D  README.md\n"
    );
    assert_eq!(
        s.git(["dupe", "ls-files", "-z"])
            .from(&dir)
            .succeeds()
            .stdout,
        b"README.md\0"
    );
    assert_eq!(fs::read(dir.join("README.md")).unwrap(), content);
}

#[test]
fn the_file_refusal_names_commands_that_stage_public_deletion_then_track_privately() {
    under_each_release(|s| refused_then_made_private(s, false));
}

#[test]
fn a_file_the_project_ignores_is_made_private_by_the_forced_add_the_refusal_names() {
    under_each_release(|s| refused_then_made_private(s, true));
}
