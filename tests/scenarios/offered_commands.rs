//! A command a line offers acts on the path or name it names, and on nothing else, when run
//! by a shell as the line shows it (G25): `clone`'s lines for a path it kept or a file in
//! the way, `hide`'s hint, the route by which a path the project's Git tracks becomes
//! private, and the alias G19 refuses. Each is offered for a path holding a space and a
//! `$`, which a shell would split or expand if written raw, and, where the command reads
//! a pathspec or an operand, for paths whose leading `:` or `*`, `?`, `[`, or `\` the
//! command would read as magic or a pattern if written as they stand.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, offered, private_commit, privately_tracked, publicly_tracked,
    under_each_release, write,
};

/// A path a shell reads as two words, the second expanded, unless it is quoted.
const SPACED: &str = "two $words";

/// Runs `command`, as a line offers it, through `/bin/sh` from `root`.
fn run_as_offered(s: &Scenario, root: &Path, command: &[u8]) -> Output {
    let command = std::str::from_utf8(command).expect("a UTF-8 command");
    s.program("/bin/sh", ["-c", command]).from(root).run()
}

/// `run_as_offered`, which must exit 0.
fn runs_as_offered(s: &Scenario, root: &Path, command: &[u8]) -> Output {
    let output = run_as_offered(s, root, command);
    assert_eq!(output.end, End::Code(0), "{output:?}");
    output
}

fn tracked(paths: &[Vec<u8>], path: &str) -> bool {
    paths.contains(&path.as_bytes().to_vec())
}

/// Hides `decoy`, which holds a file never staged: an offered `add` whose operand named
/// more than its path, `.` for one, would stage that file too.
fn hidden_decoy(s: &Scenario, root: &Path) {
    write(root, "decoy/d.md", b"not to be staged\n");
    s.git(["dupe", "hide", "decoy"]).from(root).succeeds();
}

/// Runs the offered `add`, which must leave the private index holding what it held and
/// `path`, and nothing else.
fn adds_alone(s: &Scenario, root: &Path, add: &[u8], path: &str) {
    let set = |paths: Vec<Vec<u8>>| paths.into_iter().collect::<BTreeSet<_>>();
    let mut expected = set(privately_tracked(s, root));
    expected.insert(path.as_bytes().to_vec());
    runs_as_offered(s, root, add);
    assert_eq!(set(privately_tracked(s, root)), expected, "{path}");
}

#[test]
fn clones_restore_offers_take_a_kept_path_and_an_obstruction_alone() {
    under_each_release(|s| {
        let machines = s.second_machine("project");
        let first = &machines.first.root;
        let spaced = format!("{SPACED}.txt");
        let obstruction = format!("{SPACED} dir");
        let below = format!("{obstruction}/f.txt");
        // Each kept path, and what a pathspec written as it stands would also name.
        let kept = [spaced.as_str(), ":(exclude)keep", "star*.txt"];
        let near = ["keep", "other", "starX.txt"];
        for path in kept.iter().chain(&near).chain([&below.as_str()]) {
            write(first, path, b"private\n");
            s.private(first)
                .git(["add", "-f", "--", &format!(":(literal){path}")])
                .succeeds();
        }
        private_commit(s, first);
        s.private(first)
            .git(["push", "-q", "origin", "main"])
            .succeeds();

        let second = &machines.root;
        for path in kept.iter().chain(&near) {
            write(second, path, b"mine\n");
        }
        write(second, &obstruction, b"a file\n");
        let remote = machines.first.private_remote.to_str().unwrap();
        let cloned = s.git(["dupe", "clone", remote]).from(second).run();
        assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
        let warnings = cloned.lines("warning");
        let line = |start: &str| {
            *warnings
                .iter()
                .find(|line| line.starts_with(start.as_bytes()))
                .unwrap_or_else(|| panic!("no line for {start}: {cloned:?}"))
        };

        let blocked = line(&format!("{obstruction} was kept: it is a file"));
        let restore = offered(blocked, b"then '", b"' run from the root writes them");
        fs::rename(second.join(&obstruction), second.join("aside")).unwrap();
        runs_as_offered(s, second, restore);
        assert_eq!(fs::read(second.join(&below)).unwrap(), b"private\n");

        for (at, path) in kept.iter().enumerate() {
            let differs = line(&format!("{path} was kept as it was and differs"));
            let restore = offered(differs, b"keeps it, '", b"' run from the root replaces it");
            runs_as_offered(s, second, restore);
            assert_eq!(fs::read(second.join(path)).unwrap(), b"private\n");
            // Restored alone: every other kept file is still the second machine's.
            for other in kept[at + 1..].iter().chain(&near) {
                assert_eq!(fs::read(second.join(other)).unwrap(), b"mine\n", "{other}");
            }
        }
    });
}

#[test]
fn hides_offers_to_add_and_unhide_take_the_path_alone() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.attached_project(&root);
        hidden_decoy(s, &root);
        for path in [SPACED, ":colon"] {
            let file = format!("{path}/a.md");
            write(&root, &file, b"note\n");
            let word = format!("./{path}");
            let hidden = s.git(["dupe", "hide", "--", &word]).from(&root).succeeds();
            assert_eq!(
                fs::read(root.join(".gitdupe")).unwrap(),
                format!("decoy\n{path}\n").as_bytes()
            );
            let hint = hidden.only_line("hint");

            let add = offered(hint, b"run from the root, '", b"' versions it");
            adds_alone(s, &root, add, &file);

            let unhide = offered(hint, b"versions it, and '", b"' stops hiding it");
            runs_as_offered(s, &root, unhide);
            assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"decoy\n");
        }
    });
}

#[test]
fn the_route_to_private_offered_for_a_publicly_tracked_path_takes_it_alone() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.attached_project(&root);
        let spaced = format!("{SPACED}.md");
        let routed = [spaced.as_str(), ":colon.md", "back\\slash.md"];
        // What `:colon.md` and `back\slash.md`, read as Git reads a pathspec, also name.
        let near = ["colon.md", "backslash.md"];
        for path in routed.iter().chain(&near) {
            write(&root, path, b"shared\n");
        }
        s.git(["add", "-A"]).from(&root).succeeds();
        s.commit_public(&root);
        hidden_decoy(s, &root);

        for path in routed {
            let word = format!("./{path}");
            let refused = s.git(["dupe", "add", "--", &word]).from(&root).run();
            assert_eq!(refused.end, End::Code(128), "{refused:?}");
            let line = refused.only_line("fatal");

            let removal = offered(line, b"run from the root, '", b"' first, ");
            runs_as_offered(s, &root, removal);
            let public = publicly_tracked(s, &root);
            assert!(!tracked(&public, path), "{path}");
            for other in near {
                assert!(tracked(&public, other), "{other} after {path}");
            }

            let addition = offered(line, b", then '", b"' makes it private");
            adds_alone(s, &root, addition, path);
            assert_eq!(fs::read(root.join(path)).unwrap(), b"shared\n");
        }
    });
}

#[test]
fn the_ambiguous_alias_offer_runs_the_alias_name_whole() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.attached_project(&root);
        // A name holding a dot, which the listed releases do not all read alike (G19).
        let name = "two words.x";
        s.git(["config", "--global", &format!("alias.{name}"), "status"])
            .succeeds();
        let refused = s.git(["dupe", name]).from(&root).run();
        assert_eq!(refused.end, End::Code(129), "{refused:?}");
        let errors = refused.lines("error");
        assert_eq!(errors.len(), 1, "{refused:?}");
        let line = errors[0];
        let command = offered(line, b"guards; '", b"' runs it as this Git reads it");

        // Whatever this release reads for the name, the offer runs what `git dupe git`
        // given the name as one word runs.
        let as_offered = run_as_offered(s, &root, command);
        let whole = s.git(["dupe", "git", name]).from(&root).run();
        assert_eq!(as_offered.end, whole.end, "{as_offered:?} {whole:?}");
        assert_eq!(as_offered.stdout, whole.stdout, "{as_offered:?} {whole:?}");
    });
}
