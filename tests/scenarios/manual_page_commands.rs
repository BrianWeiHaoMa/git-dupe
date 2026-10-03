//! What the page's own texts (`src/front/page/`) tell a developer to type, run as written:
//! every line of a text that begins, after its indentation, with `$ ` is taken from the
//! text as the repository holds it and run, in order, in the state the texts describe,
//! each exiting 0; then what the texts say those commands did is observed through
//! git-dupe's own statuses, the two indexes, the refs, and the files. The quick start
//! runs in a project whose `.gitignore` ignores `.env.local`, on `main` and on a branch of
//! another name, and the examples in the state it left.
//!
//! A command line is indented, or the page fills it into a paragraph and nothing runs it.
//! It holds `git dupe` and plain words, letters, digits, and `-_./:=@,+%`, a double-quoted
//! run of them being one word, so that it runs here as `s.git(["dupe", …])`, with no
//! shell. An option word holds no `/`, no other word nor a part of one after an `=` begins
//! with `/`, and no word holds `..` among its parts, so that a line run from a root inside
//! the scenario's directory names no place outside it. The one placeholder is
//! `<private-url>`, the URL of the developer's private repository: it stands for the empty
//! bare repository the quick start tells the developer to make, made here inside the
//! scenario's directory with the project's branch as its default. Each text's lines run
//! from the root of the project, except that a `git dupe clone` line begins the second
//! machine, a fresh `git clone` of the project, from whose root it and every later line of
//! its text run. A line that breaks one of these rules fails before any line runs. A
//! command that needs a terminal, or a state these scenarios do not build, belongs in a
//! text's prose.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, command_lines, held_publicly, leaving_public_git, page_texts_present,
    privately_tracked, publicly_tracked, records, text_present, under_each_release, write,
};

/// The placeholder for the URL of the developer's private repository.
const PRIVATE_URL: &str = "<private-url>";

/// The texts followed here, by file name. A text holding a command line is one of them.
const QUICK_START: &str = "10-quick-start.txt";
const EXAMPLES: &str = "20-examples.txt";

/// The bytes besides letters and digits a plain word may hold.
const PLAIN: &[u8] = b"-_./:=@,+%<>";

/// The words of a command line as a shell splits it when it holds only plain words: at
/// single spaces, a double-quoted run of plain words and spaces being one word without its
/// quotes. Fails on anything else a shell reads, on a placeholder other than
/// `<private-url>`, and on a word that could name a place outside the root it runs from:
/// an option with a path attached, as `-f/tmp/x`, an absolute path, or a way up.
fn words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    for byte in line.bytes() {
        match byte {
            b'"' => (quoted, started) = (!quoted, true),
            b' ' if !quoted => {
                assert!(started, "not one space between two words: {line}");
                words.push(std::mem::take(&mut word));
                started = false;
            }
            _ if byte.is_ascii_alphanumeric()
                || PLAIN.contains(&byte)
                || (quoted && byte == b' ') =>
            {
                word.push(char::from(byte));
                started = true;
            }
            _ => panic!("{:?} is more than a plain word: {line}", char::from(byte)),
        }
    }
    assert!(
        !quoted && started,
        "an open quote or a space at the end: {line}"
    );
    words.push(word);

    for word in &words {
        assert!(
            word == PRIVATE_URL || !word.contains(['<', '>']),
            "a placeholder these scenarios do not know, in {word:?}: {line}"
        );
        let outside = (word.starts_with('-') && word.contains('/'))
            || word.split('=').any(|part| part.starts_with('/'))
            || word.split(['/', '=']).any(|part| part == "..");
        assert!(
            !outside,
            "a word that can name a place outside the root, {word:?}: {line}"
        );
    }
    assert!(
        words.starts_with(&["git".to_owned(), "dupe".to_owned()]),
        "not a git dupe command: {line}"
    );
    words
}

/// The developer's machines: the first, where the project was cloned, the second, which a
/// `git dupe clone` line begins, and the private repository the texts push to.
struct Machines {
    first: PathBuf,
    second: PathBuf,
    private_remote: PathBuf,
}

/// Every command line of a text, with what it printed, in the order run.
struct Ran(Vec<(String, Output)>);

impl Ran {
    /// What the line `line` of the text printed: the observation of what the text says
    /// about it follows the text, and fails here when the text no longer runs it.
    fn of(&self, line: &str) -> &Output {
        let mut found = self.0.iter().filter(|(ran, _)| ran == line);
        let (Some((_, output)), None) = (found.next(), found.next()) else {
            panic!("the text does not run `{line}` once; what it says of it is observed here");
        };
        output
    }
}

impl Machines {
    /// Runs the command lines of the text at `path`, each from the root of its machine, and
    /// returns what each printed, once each has exited 0 and left the public `.git` of its
    /// machine as it was but for the private repository and the exclude file.
    fn follow(&self, s: &Scenario, path: &Path, text: &str) -> Ran {
        let lines = command_lines(text);
        let lines: Vec<(&str, Vec<String>)> =
            lines.iter().map(|&line| (line, words(line))).collect();
        let mut root = &self.first;
        let mut ran = Vec::new();
        for (line, words) in lines {
            if words.get(2).map(String::as_str) == Some("clone") {
                assert!(
                    root == &self.first,
                    "{}: a second `git dupe clone`: {line}",
                    path.display()
                );
                s.public_clone(&self.first, &self.second);
                root = &self.second;
            }
            let words = words.iter().skip(1).map(|word| {
                if word == PRIVATE_URL {
                    self.private_remote.as_os_str().to_owned()
                } else {
                    word.into()
                }
            });
            let output = leaving_public_git(root, s.git(words).from(root));
            assert_eq!(
                output.end,
                End::Code(0),
                "{}: `{line}`, run from {}: {output:?}",
                path.display(),
                root.display()
            );
            ran.push((line.to_owned(), output));
        }
        Ran(ran)
    }
}

/// `git status --porcelain` of the project at `root`: what public Git lists there.
fn public_status(s: &Scenario, root: &Path) -> Vec<u8> {
    s.git(["status", "--porcelain"])
        .from(root)
        .succeeds()
        .stdout
}

/// `git dupe status --porcelain` at `root`, which must exit 0.
fn private_status(s: &Scenario, root: &Path) -> Vec<u8> {
    s.git(["dupe", "status", "--porcelain"])
        .from(root)
        .succeeds()
        .stdout
}

/// Every object of the private history at `root`, none of which the public repository
/// there may hold.
fn held_publicly_of_private_history(s: &Scenario, root: &Path) -> Vec<Vec<u8>> {
    let private = s
        .private(root)
        .git(["rev-list", "--objects", "--all"])
        .succeeds();
    let ids: Vec<u8> = records(&private.stdout, b'\n')
        .into_iter()
        .flat_map(|record| record.split(|&byte| byte == b' ').next())
        .flat_map(|id| [id, &b"\n"[..]].concat())
        .collect();
    assert!(!ids.is_empty(), "no private history at {}", root.display());
    held_publicly(s, root, &ids)
}

/// The commit `name` names in the repository at `directory`.
fn commit(s: &Scenario, directory: &Path, name: &str) -> Vec<u8> {
    s.git(["rev-parse", name]).from(directory).succeeds().stdout
}

/// The commit the private `HEAD` at `root` names.
fn private_head(s: &Scenario, root: &Path) -> Vec<u8> {
    s.private(root).git(["rev-parse", "HEAD"]).succeeds().stdout
}

/// The lines of `.gitdupe` at `root`, sorted.
fn listed(root: &Path) -> Vec<String> {
    let mut lines: Vec<String> = fs::read_to_string(root.join(".gitdupe"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    lines.sort();
    lines
}

/// The quick start, then the examples, as the page's own texts give them, in a project on
/// `branch` whose committed `.gitignore` ignores `.env.local`, `.vscode/`, and `build/`.
fn follow_the_page(s: &Scenario, branch: &str) {
    // What a developer's machines have: an identity for a commit, here in the home both
    // machines share, and no maintenance left running after a command.
    for (key, value) in [
        ("user.name", "A Developer"),
        ("user.email", "developer@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let machines = Machines {
        first: s.dir().join("project"),
        second: s.dir().join("second"),
        private_remote: s.dir().join("private.git"),
    };
    let first = &machines.first;
    s.unattached_project(first);
    if branch != "main" {
        s.git(["branch", "-m", branch]).from(first).succeeds();
    }
    // The private repository as the quick start says to make one on a disk: empty, with
    // the project's branch as its default.
    let words = ["init", "-q", "--bare", "-b", branch].map(OsStr::new);
    s.git(
        words
            .into_iter()
            .chain([machines.private_remote.as_os_str()]),
    )
    .succeeds();
    let public_files = publicly_tracked(s, first);
    let public_head = commit(s, first, "HEAD");

    let mut followed = Vec::new();
    for (path, _, text) in page_texts_present() {
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        match name.as_str() {
            QUICK_START => {
                // The text's own words: notes/ holds notes of yours, and .env.local your
                // settings, a file the project's .gitignore ignores.
                write(first, "notes/todo.md", b"todo\n");
                write(first, ".env.local", b"settings\n");
                machines.follow(s, &path, &text);
                quick_start_holds(s, &machines, branch);
            }
            EXAMPLES => {
                // The text's own words: since the quick start, .env.local edited,
                // notes/today.md written, three new files, and a build product.
                write(first, ".env.local", b"settings of the day\n");
                write(first, "notes/today.md", b"today\n");
                write(first, ".vscode/settings.json", b"{}\n");
                write(first, "docs/notes.md", b"notes on the docs\n");
                write(first, "src/new.py", b"new\n");
                write(first, "build/out.js", b"built\n");
                let ran = machines.follow(s, &path, &text);
                examples_hold(s, &machines, &ran);
            }
            _ => assert!(
                command_lines(&text).is_empty(),
                "{}: no scenario runs its command lines; follow it here",
                path.display()
            ),
        }
        followed.push(name);
    }
    assert!(
        followed.starts_with(&[QUICK_START.to_owned(), EXAMPLES.to_owned()]),
        "{followed:?}"
    );

    // Neither machine's public repository holds anything private, and the project's
    // history and tracked files are as they were.
    for root in [first, &machines.second] {
        assert_eq!(
            held_publicly_of_private_history(s, root),
            Vec::<Vec<u8>>::new()
        );
    }
    assert_eq!(commit(s, first, "HEAD"), public_head);
    assert_eq!(publicly_tracked(s, first), public_files);
}

/// What the quick start says: the project's branch is the private one, the files it made
/// private are hidden and committed, pushed, and brought to the second machine, where the
/// same paths are hidden; neither machine's Git lists a private file.
fn quick_start_holds(s: &Scenario, machines: &Machines, branch: &str) {
    let (first, second) = (&machines.first, &machines.second);
    let tracked: Vec<Vec<u8>> = [".env.local", ".gitdupe", "notes/todo.md"]
        .map(|path| path.as_bytes().to_vec())
        .to_vec();
    assert_eq!(privately_tracked(s, first), tracked);
    assert_eq!(listed(first), ["notes"]);
    let head = private_head(s, first);
    let pushed = commit(s, &machines.private_remote, &format!("refs/heads/{branch}"));
    assert_eq!(pushed, head);
    for root in [first, second] {
        let on = s.private(root).git(["symbolic-ref", "HEAD"]).succeeds();
        assert_eq!(on.stdout, format!("refs/heads/{branch}\n").as_bytes());
        assert_eq!(private_head(s, root), head, "{}", root.display());
        assert_eq!(privately_tracked(s, root), tracked, "{}", root.display());
        assert_eq!(public_status(s, root), b"", "{}", root.display());
        assert_eq!(private_status(s, root), b"", "{}", root.display());
    }
    for path in [".env.local", ".gitdupe", "notes/todo.md"] {
        assert_eq!(
            fs::read(second.join(path)).unwrap(),
            fs::read(first.join(path)).unwrap(),
            "{path}"
        );
    }
}

/// What the examples say: the three paths made private, the day's changes under the
/// hidden paths committed and nothing else, the commit pushed, the project's untracked
/// files deleted and every hidden path kept, nothing for a script to see, and the help of
/// `add` printed.
fn examples_hold(s: &Scenario, machines: &Machines, ran: &Ran) {
    let first = &machines.first;
    assert_eq!(listed(first), [".vscode", "notes", "scratch"]);
    let private_files = [
        ".env.local",
        ".gitdupe",
        ".vscode/settings.json",
        "docs/notes.md",
        "notes/today.md",
        "notes/todo.md",
    ];
    let committed = s
        .private(first)
        .git(["ls-tree", "-r", "-z", "--name-only", "HEAD"])
        .succeeds();
    assert_eq!(
        records(&committed.stdout, 0),
        private_files.map(str::as_bytes),
        "src/new.py, which nothing hides, is not taken"
    );
    let env = s
        .private(first)
        .git(["cat-file", "blob", "HEAD:.env.local"])
        .succeeds();
    assert_eq!(env.stdout, b"settings of the day\n");
    let branch = s
        .private(first)
        .git(["symbolic-ref", "HEAD"])
        .succeeds()
        .stdout;
    let branch = String::from_utf8(branch).unwrap();
    let pushed = commit(s, &machines.private_remote, branch.trim_end());
    assert_eq!(pushed, private_head(s, first));

    // `git dupe clean -fdx`: the project's untracked files gone, ignored or not; every
    // hidden path and the project's own files kept.
    for gone in ["build/out.js", "src/new.py"] {
        assert!(fs::symlink_metadata(first.join(gone)).is_err(), "{gone}");
    }
    for kept in private_files.iter().chain(&["README.md", "docs/design.md"]) {
        assert!(first.join(kept).is_file(), "{kept}");
    }
    assert_eq!(public_status(s, first), b"");

    let porcelain = ran.of("git dupe status --porcelain");
    assert_eq!(porcelain.stdout, b"", "{porcelain:?}");
    let help = ran.of("git dupe help add");
    assert_eq!(help.stdout, text_present("add").as_bytes());

    // Whatever is written in scratch/ later is hidden from the project's Git, and private
    // Git sees it.
    write(first, "scratch/plan.md", b"plan\n");
    assert_eq!(public_status(s, first), b"");
    assert_eq!(private_status(s, first), b"?? scratch/\n");
}

#[test]
fn the_quick_start_and_the_examples_run_as_written_on_main() {
    under_each_release(|s| follow_the_page(s, "main"));
}

#[test]
fn the_quick_start_and_the_examples_run_as_written_on_another_branch() {
    under_each_release(|s| follow_the_page(s, "trunk"));
}
