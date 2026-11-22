//! What `git dupe clone` says and writes where a private file meets a path the project
//! tracks (G2, G5, G25, N9, `In use` 5): no line of `clone`'s offers a `restore` that would
//! name a path at or below which the project tracks one, the project's file stays as it
//! was, and a privately tracked file the project tracks that is missing from disk stays
//! unwritten and is named; where the project tracks nothing there, a kept file is offered
//! a commit of that path alone and a restore, and an obstruction a restore through it.
//! Each command a line names is run as the line shows it, from the root.

use std::path::Path;
use std::{fs, str};

use crate::harness::{
    End, Output, Scenario, committing, dupe_clone, holds, names, offered, private_add,
    private_commit, under_each_release, write,
};

/// The project's own text at a path it tracks.
const PROJECTS: &[u8] = b"the project's own\n";

/// The private text at the same path.
const PRIVATE: &[u8] = b"the private version\n";

/// An editor that writes a commit message, as a developer's would.
const EDITOR: &str = "printf 'Keep the kept file\\n' >";

/// The `warning:` of `output` that begins with `start`.
fn line<'o>(output: &'o Output, start: &str) -> &'o [u8] {
    output
        .lines("warning")
        .into_iter()
        .find(|line| line.starts_with(start.as_bytes()))
        .unwrap_or_else(|| panic!("no line for {start}: {output:?}"))
}

/// Runs `command`, as a line shows it, through the scenario's shell from `root`, with an
/// editor that writes a message, and expects it to succeed.
fn run_as_shown(s: &Scenario, root: &Path, command: &[u8]) -> Output {
    let command = str::from_utf8(command).expect("a UTF-8 command");
    let output = s
        .program(s.shell(), ["-c", command])
        .from(root)
        .variable("GIT_EDITOR", EDITOR)
        .run();
    assert_eq!(output.end, End::Code(0), "{command}: {output:?}");
    output
}

/// Every `'git dupe …'` command `line` offers, in order: what follows each `'git dupe `
/// up to the next `'`, the paths here holding none.
fn commands(line: &[u8]) -> Vec<&[u8]> {
    let opening = b"'git dupe ";
    (0..line.len())
        .filter(|&at| line[at..].starts_with(opening))
        .map(|at| {
            let rest = &line[at + 1..];
            let end = rest[opening.len() - 1..]
                .iter()
                .position(|&byte| byte == b'\'')
                .expect("a closed offer");
            &rest[..opening.len() - 1 + end]
        })
        .collect()
}

/// The entry `git dupe status --porcelain` gives `path` in `root`, its two status letters.
fn private_status(s: &Scenario, root: &Path, path: &str) -> Option<[u8; 2]> {
    let output = s
        .git(["dupe", "status", "--porcelain", "-z"])
        .from(root)
        .succeeds();
    output
        .stdout
        .split(|&byte| byte == 0)
        .find(|entry| entry.len() > 3 && &entry[3..] == path.as_bytes())
        .map(|entry| [entry[0], entry[1]])
}

/// Public `git status --porcelain` in `root`.
fn public_status(s: &Scenario, root: &Path) -> Vec<u8> {
    s.git(["status", "--porcelain"])
        .from(root)
        .succeeds()
        .stdout
}

#[test]
fn a_path_the_project_tracks_keeps_the_projects_file_and_no_line_offers_to_write_over_it() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        // The first machine's private history: beside `daily_state`'s, the files the
        // project comes to track on the second machine, and `scratch/s.md` and
        // `tools/run.sh`, which only the private repository tracks.
        for path in [
            "AGENTS.md",
            "CLAUDE.md",
            "config",
            "scratch/s.md",
            "tools/run.sh",
        ] {
            write(first, path, PRIVATE);
            private_add(s, first, path);
        }
        private_commit(s, first);
        s.private(first)
            .git(["push", "-q", "origin", "main"])
            .succeeds();

        // The second machine: the project tracks `AGENTS.md`, `CLAUDE.md`, `notes/a.md`,
        // `config/local.example` in a directory where the private repository has a file,
        // and a file `tools` above the private `tools/run.sh`, as after a pull that brought
        // them; `CLAUDE.md` is then deleted from disk and not from the index, and a file
        // stands at `notes`. A file stands at `scratch` too, and the machine has its own
        // `.env.local` and `.vscode/settings.json`, which the project ignores.
        let projects = [
            "AGENTS.md",
            "CLAUDE.md",
            "config/local.example",
            "notes/a.md",
            "tools",
        ];
        for path in projects {
            write(second, path, PROJECTS);
        }
        s.git(["add", "--"].into_iter().chain(projects))
            .from(second)
            .succeeds();
        s.commit_public(second);
        fs::remove_file(second.join("CLAUDE.md")).unwrap();
        fs::remove_dir_all(second.join("notes")).unwrap();
        write(second, "notes", b"a file at notes\n");
        write(second, "scratch", b"a file at scratch\n");
        write(second, ".env.local", b"mine\n");
        write(second, ".vscode/settings.json", b"mine too\n");
        let projects_index = s.git(["ls-files", "-s"]).from(second).succeeds().stdout;

        let cloned = dupe_clone(s, second, &[m.first.private_remote.as_os_str()]);
        assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
        assert!(cloned.lines("hint").is_empty(), "{cloned:?}");

        // The kept file the project tracks: the project's file, no restore, and the
        // command that prints the private version.
        let agents = line(&cloned, "AGENTS.md was kept");
        names(agents, b"the project tracks it");
        names(agents, b"'git add' stages");
        names(agents, b"'git dupe show HEAD:AGENTS.md'");
        assert!(
            !holds(agents, b"restore") && !holds(agents, b"commit"),
            "{cloned:?}"
        );
        // The obstruction above a path the project tracks offers no restore through it.
        let notes = line(&cloned, "notes was kept: it is a file");
        names(notes, b"the project tracks a path at or below it");
        assert!(!holds(notes, b"restore"), "{cloned:?}");
        let listing = offered(notes, b"there: '", b"' run from the root lists them");
        assert_eq!(run_as_shown(s, second, listing).stdout, b"notes/a.md\n");
        // The file below it is named too, as one the project tracks.
        names(
            line(&cloned, "notes/a.md was not written"),
            b"the project tracks it",
        );
        // So does the project's own file above a path only the private repository tracks:
        // moving it aside would take the project's file away.
        let tools = line(&cloned, "tools was kept: it is a file");
        names(tools, b"the project tracks a path at or below it");
        assert!(!holds(tools, b"restore"), "{cloned:?}");
        // And the directory holding a path the project tracks, where the private
        // repository has a file: no restore, and the command that prints the file.
        let config = line(&cloned, "config was kept: it is a directory");
        names(config, b"the project tracks a path at or below it");
        names(config, b"'git dupe show HEAD:config'");
        assert!(!holds(config, b"restore"), "{cloned:?}");
        // The absent file the project tracks is left unwritten and named.
        let claude = line(&cloned, "CLAUDE.md was not written");
        names(claude, b"the project tracks it");
        names(claude, b"'git dupe show HEAD:CLAUDE.md'");
        assert!(!holds(claude, b"restore"), "{cloned:?}");
        // Every line is one line, and none holds either version's text.
        for line in cloned.lines("warning") {
            assert!(!holds(line, b"the project's own") && !holds(line, b"private version\n"));
        }

        // Nothing written over or in place of the project's files: `AGENTS.md` holds the
        // project's bytes, `CLAUDE.md` and `notes/a.md` are still missing, and the public
        // index is as it was; a path only the private repository tracks is written.
        assert_eq!(fs::read(second.join("AGENTS.md")).unwrap(), PROJECTS);
        assert!(!second.join("CLAUDE.md").exists());
        assert!(second.join("notes").is_file());
        assert_eq!(fs::read(second.join("tools")).unwrap(), PROJECTS);
        assert_eq!(
            fs::read(second.join("config/local.example")).unwrap(),
            PROJECTS
        );
        assert_eq!(
            fs::read(second.join("docs/notes.md")).unwrap(),
            b"private\n"
        );
        assert_eq!(
            s.git(["ls-files", "-s"]).from(second).succeeds().stdout,
            projects_index
        );
        // `CLAUDE.md` shows as deleted in the private repository, and a public add names
        // only the project's own deletions.
        assert_eq!(private_status(s, second, "CLAUDE.md"), Some(*b" D"));
        let dry = s
            .git(["add", "-A", "--dry-run"])
            .from(second)
            .succeeds()
            .stdout;
        for private in ["CLAUDE.md", "AGENTS.md", "notes/a.md"] {
            assert!(
                !holds(&dry, format!("add '{private}'").as_bytes()),
                "{dry:?}"
            );
        }

        // Each command the lines for the project's paths name, run as shown from the root,
        // prints the private version and leaves the project's files and index as they were.
        for line in [agents, claude, config] {
            let shown = commands(line);
            assert!(!shown.is_empty(), "{line:?}");
            for command in shown {
                let output = run_as_shown(s, second, command);
                assert_eq!(output.stdout, PRIVATE, "{output:?}");
            }
        }
        assert_eq!(fs::read(second.join("AGENTS.md")).unwrap(), PROJECTS);
        assert!(!second.join("CLAUDE.md").exists());
        assert!(!holds(&public_status(s, second), b"AGENTS.md"));
        assert_eq!(
            s.git(["ls-files", "-s"]).from(second).succeeds().stdout,
            projects_index
        );

        // The obstruction above a path only the private repository tracks keeps its
        // restore, which, followed, writes the file below it.
        let scratch = line(&cloned, "scratch was kept: it is a file");
        let restore = offered(scratch, b"then '", b"' run from the root writes them");
        fs::rename(second.join("scratch"), second.join("scratch-aside")).unwrap();
        run_as_shown(s, second, restore);
        assert_eq!(fs::read(second.join("scratch/s.md")).unwrap(), PRIVATE);

        // A kept file the project ignores is kept by committing that path alone: the other
        // kept file stays modified and uncommitted.
        let env = line(&cloned, ".env.local was kept");
        let keep = offered(env, b"; '", b"' run from the root keeps it");
        assert_eq!(keep, b"git dupe commit -- .env.local");
        names(
            env,
            b"'git dupe restore -- .env.local' run from the root replaces it",
        );
        let vscode = line(&cloned, ".vscode/settings.json was kept");
        names(vscode, b"'git dupe commit -- .vscode/settings.json'");
        committing(s, second);
        run_as_shown(s, second, keep);
        let committed = s
            .private(second)
            .git(["show", "HEAD:.env.local"])
            .succeeds()
            .stdout;
        assert_eq!(committed, b"mine\n");
        assert_eq!(private_status(s, second, ".env.local"), None);
        assert_eq!(
            private_status(s, second, ".vscode/settings.json"),
            Some(*b" M")
        );
        let last = s
            .private(second)
            .git(["show", "--name-only", "--format=", "HEAD"])
            .succeeds()
            .stdout;
        assert_eq!(last, b".env.local\n");
    });
}
