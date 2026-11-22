//! G8's warning for a path both repositories track names the path in each command it
//! offers, says which repository each stops tracking it, and that a public `git add`
//! stages whatever stands there (G8, G25). Each command, run as the line shows it from the
//! root, stops the repository it names from tracking that path alone and leaves the other
//! tracking it, whether the path came to be tracked by both through a plain `git add -f`
//! or through a public commit of the project's own file there: for a path holding a space
//! and a `$`, which a shell would split and expand, and for one holding `*`, which Git
//! would read as a pattern that also names its neighbor, itself tracked by both.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::{fs, str};

use crate::harness::{
    End, Output, Scenario, name_accepted, names, offered, private_add, private_commit,
    privately_tracked, publicly_tracked, under_each_release, write,
};

/// A path a shell splits and expands unless the command quotes it.
const SPACED: &str = "two $words.md";

/// A path Git reads as a pattern unless the command makes it literal, and the neighbor
/// that pattern also names.
const STARRED: &str = "star*.md";
const NEIGHBOR: &str = "starX.md";

/// How the paths came to be tracked by both.
#[derive(Clone, Copy, Debug)]
enum Arrival {
    /// A plain `git add -f` staged the private files in the project.
    AddForce,
    /// The project committed its own files at the private paths, as a pull brings them.
    ProjectsCommit,
}

/// An attached project at `name` whose `paths` are privately tracked and committed, then
/// tracked by both as `arrival` says.
fn tracked_by_both(s: &Scenario, name: &str, arrival: Arrival, paths: &[&str]) -> PathBuf {
    let root = s.dir().join(name);
    s.attached_project(&root);
    for path in paths {
        write(&root, path, b"private\n");
        private_add(s, &root, &format!(":(literal){path}"));
    }
    private_commit(s, &root);
    for path in paths {
        if let Arrival::ProjectsCommit = arrival {
            write(&root, path, b"the project's\n");
        }
        s.git(["add", "-f", "--", &format!(":(literal){path}")])
            .from(&root)
            .succeeds();
    }
    if let Arrival::ProjectsCommit = arrival {
        s.commit_public(&root);
    }
    root
}

/// G8's warning for `path` on the next command, which every command G6 covers prints.
fn warning(s: &Scenario, root: &Path, path: &str) -> Vec<u8> {
    let status = s.git(["dupe", "status"]).from(&root.join("docs")).run();
    assert_eq!(status.end, End::Code(0), "{status:?}");
    let start = format!("{path} is tracked by both");
    let lines: Vec<&[u8]> = status
        .lines("warning")
        .into_iter()
        .filter(|line| line.starts_with(start.as_bytes()))
        .collect();
    assert_eq!(lines.len(), 1, "{status:?}");
    lines[0].to_vec()
}

/// Runs `command`, as the line shows it, through the scenario's shell from `root`.
fn run_as_shown(s: &Scenario, root: &Path, command: &[u8]) -> Output {
    let command = str::from_utf8(command).expect("a UTF-8 command");
    let output = s.program(s.shell(), ["-c", command]).from(root).run();
    assert_eq!(output.end, End::Code(0), "{command}: {output:?}");
    output
}

/// `paths` less `released`, as the sets of paths an index then holds.
fn without(paths: &[&str], released: &str) -> BTreeSet<Vec<u8>> {
    paths
        .iter()
        .filter(|path| **path != released)
        .map(|path| path.as_bytes().to_vec())
        .collect()
}

/// `tracked` as a set, the project's own files left out.
fn among(tracked: Vec<Vec<u8>>, paths: &[&str]) -> BTreeSet<Vec<u8>> {
    tracked
        .into_iter()
        .filter(|path| paths.iter().any(|ours| ours.as_bytes() == &path[..]))
        .collect()
}

#[test]
fn each_command_the_both_tracked_warning_names_releases_the_path_from_the_repository_it_names() {
    under_each_release(|s| {
        let starred = name_accepted(s.dir(), STARRED.as_bytes(), "a path holding '*'");
        let mut paths = vec![SPACED];
        if starred {
            paths.extend([STARRED, NEIGHBOR]);
        }
        for arrival in [Arrival::AddForce, Arrival::ProjectsCommit] {
            for released in paths.iter().copied().filter(|path| *path != NEIGHBOR) {
                // The private repository's command, in one project.
                let name = format!("private-{arrival:?}-{}", released.len());
                let root = tracked_by_both(s, &name, arrival, &paths);
                let line = warning(s, &root, released);
                names(&line, b"a public 'git add' stages whatever stands there");
                let private = offered(
                    &line,
                    b"from the root, '",
                    b"' stops the private repository",
                );
                run_as_shown(s, &root, private);
                let both: BTreeSet<Vec<u8>> =
                    paths.iter().map(|path| path.as_bytes().to_vec()).collect();
                assert_eq!(
                    among(privately_tracked(s, &root), &paths),
                    without(&paths, released),
                    "{arrival:?} {released}"
                );
                assert_eq!(among(publicly_tracked(s, &root), &paths), both);
                assert!(root.join(released).is_file());

                // The project's command, in another.
                let name = format!("public-{arrival:?}-{}", released.len());
                let root = tracked_by_both(s, &name, arrival, &paths);
                let line = warning(s, &root, released);
                let public = offered(&line, b"tracking it and '", b"' stops the public one");
                let before = fs::read(root.join(released)).unwrap();
                run_as_shown(s, &root, public);
                assert_eq!(
                    among(publicly_tracked(s, &root), &paths),
                    without(&paths, released),
                    "{arrival:?} {released}"
                );
                assert_eq!(among(privately_tracked(s, &root), &paths), both);
                assert_eq!(fs::read(root.join(released)).unwrap(), before);
                // Released from one side, the path is no longer named as tracked by both.
                let status = s.git(["dupe", "status"]).from(&root).succeeds();
                let start = format!("{released} is tracked");
                assert!(
                    status
                        .lines("warning")
                        .iter()
                        .all(|line| !line.starts_with(start.as_bytes())),
                    "{status:?}"
                );
            }
        }
        // The spaced path's commands, as a reader sees them.
        let root = tracked_by_both(s, "spelled", Arrival::AddForce, &[SPACED]);
        let line = warning(s, &root, SPACED);
        names(&line, br#"'git dupe rm --cached -- "two \$words.md"'"#);
        names(&line, br#"'git rm --cached -- "two \$words.md"'"#);
    });
}
