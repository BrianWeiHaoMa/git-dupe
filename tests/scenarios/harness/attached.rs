//! Attached workspaces: a private repository made as `init` makes one, `init` itself for
//! scenarios about something else, private runs for building fixtures and reading the
//! private index, `.gitdupe` as staged there, and the managed region read back.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::output::End;
use super::scenario::{Git, Scenario};

impl Scenario {
    /// Makes `directory` an ordinary repository with one commit on `main`, attached to a
    /// private repository at `.git/dupe` as `init` makes one: Git's `init` with the
    /// private Git directory and the root in the environment, then `core.worktree` set
    /// to `../..`. Nothing else: no key of F2's, no region, no `.gitdupe`.
    pub fn attached_repository(&self, directory: &Path) {
        self.repository(directory);
        let private = directory.join(".git/dupe");
        for words in [&["init", "-q"][..], &["config", "core.worktree", "../.."]] {
            let made = self
                .git(words)
                .from(directory)
                .variable("GIT_DIR", &private)
                .variable("GIT_WORK_TREE", directory)
                .run();
            assert_eq!(made.end, End::Code(0), "git {words:?}: {made:?}");
        }
    }

    /// Runs `git dupe init` from `directory`, which must succeed: for a scenario whose
    /// subject is not `init` itself.
    pub fn init(&self, directory: &Path) {
        let init = self.git(["dupe", "init"]).from(directory).run();
        assert_eq!(init.end, End::Code(0), "git dupe init: {init:?}");
    }

    /// Git against the private repository of the workspace at `directory`, run from
    /// there: for building fixtures (`add -f`, `commit`) and reading the private index in
    /// assertions. git-dupe itself is reached only as `git dupe …`.
    pub fn private(&self, directory: &Path) -> Private<'_> {
        Private {
            scenario: self,
            directory: directory.to_path_buf(),
        }
    }
}

pub struct Private<'s> {
    scenario: &'s Scenario,
    directory: PathBuf,
}

impl<'s> Private<'s> {
    /// `git --git-dir=<directory>/.git/dupe --work-tree=<directory> <words>`.
    pub fn git<W: AsRef<OsStr>>(&self, words: impl IntoIterator<Item = W>) -> Git<'s> {
        let mut git_dir = OsString::from("--git-dir=");
        git_dir.push(self.directory.join(".git/dupe"));
        let mut work_tree = OsString::from("--work-tree=");
        work_tree.push(&self.directory);
        let words = words.into_iter().map(|word| word.as_ref().to_owned());
        self.scenario
            .git([git_dir, work_tree].into_iter().chain(words))
            .from(&self.directory)
    }
}

/// Stages `path` in the private index of the workspace at `dir`, with `add -f`.
pub fn private_add(s: &Scenario, dir: &Path, path: &str) {
    let output = s.private(dir).git(["add", "-f", "--", path]).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
}

/// Commits what the private index of the workspace at `dir` holds, as private history for
/// a fixture.
pub fn private_commit(s: &Scenario, dir: &Path) {
    let words = [
        "-c",
        "maintenance.auto=false",
        "-c",
        "user.name=Scenario",
        "-c",
        "user.email=scenario@example.invalid",
        "commit",
        "-qm",
        "private files",
    ];
    let output = s.private(dir).git(words).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
}

/// Makes `dir` the project of `Scenario::attached_project` in daily use: `.gitdupe`
/// listing `notes` and `.vscode`, and `.gitdupe`, `notes/a.md`, `.env.local`,
/// `.vscode/settings.json`, and `docs/notes.md` privately tracked and committed, each of
/// the four files holding `private\n`.
pub fn daily_state(s: &Scenario, dir: &Path) {
    s.attached_project(dir);
    fs::write(dir.join(".gitdupe"), b"notes\n.vscode\n").unwrap();
    for path in [
        "notes/a.md",
        ".env.local",
        ".vscode/settings.json",
        "docs/notes.md",
    ] {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, b"private\n").unwrap();
        private_add(s, dir, path);
    }
    private_add(s, dir, ".gitdupe");
    private_commit(s, dir);
}

/// `daily_state` in `name` below the scenario's directory, with the day's two edits: a
/// new `notes/today.md` holding `today\n`, and `.env.local` changed to `changed\n`.
pub fn daily_edited(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    fs::write(dir.join("notes/today.md"), b"today\n").unwrap();
    fs::write(dir.join(".env.local"), b"changed\n").unwrap();
    dir
}

/// The staged `.gitdupe` of the private index, or `None` when none is staged.
pub fn staged_gitdupe(s: &Scenario, dir: &Path) -> Option<Vec<u8>> {
    let blob = s.private(dir).git(["cat-file", "blob", ":.gitdupe"]).run();
    (blob.end == End::Code(0)).then_some(blob.stdout)
}

/// `.gitdupe` is in the private index, and both its staged blob and the file on disk are
/// `content`.
pub fn gitdupe_written_and_staged(s: &Scenario, dir: &Path, content: &[u8]) {
    let listed = s.private(dir).git(["ls-files", "-z"]).run();
    assert_eq!(listed.end, End::Code(0), "{listed:?}");
    assert!(
        listed
            .stdout
            .split(|&byte| byte == 0)
            .any(|path| path == b".gitdupe"),
        "{listed:?}"
    );
    assert_eq!(staged_gitdupe(s, dir).as_deref(), Some(content));
    assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), content);
}

/// The managed region of a workspace's `.git/info/exclude`, as bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct Region {
    /// The bytes before the line `# BEGIN git-dupe`.
    pub before: Vec<u8>,
    /// The lines between the markers, without their newlines.
    pub rules: Vec<Vec<u8>>,
    /// The bytes after the line `# END git-dupe`, empty when there is no end marker.
    pub after: Vec<u8>,
}

/// The region of the workspace at `directory`, or `None` when the exclude file holds no
/// begin marker or does not exist. The first begin marker and the first end marker after
/// it bound the region; without an end marker it runs to the end of the file.
pub fn region(directory: &Path) -> Option<Region> {
    let file = directory.join(".git/info/exclude");
    let bytes = match fs::read(&file) {
        Ok(bytes) => bytes,
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => return None,
        Err(cause) => panic!("{}: {cause}", file.display()),
    };
    let mut offset = 0;
    let mut lines = Vec::new();
    for line in bytes.split_inclusive(|&byte| byte == b'\n') {
        lines.push((offset, line.strip_suffix(b"\n").unwrap_or(line)));
        offset += line.len();
    }
    let begin = lines
        .iter()
        .position(|(_, line)| *line == b"# BEGIN git-dupe")?;
    let mut rules = Vec::new();
    let mut after = Vec::new();
    for (index, (_, line)) in lines.iter().enumerate().skip(begin + 1) {
        if *line == b"# END git-dupe" {
            let next = lines
                .get(index + 1)
                .map_or(bytes.len(), |(start, _)| *start);
            after = bytes[next..].to_vec();
            break;
        }
        rules.push(line.to_vec());
    }
    Some(Region {
        before: bytes[..lines[begin].0].to_vec(),
        rules,
        after,
    })
}

/// The rules of the region of the workspace at `directory`, which must have one.
pub fn region_rules(directory: &Path) -> Vec<Vec<u8>> {
    region(directory)
        .unwrap_or_else(|| panic!("no region in {}", directory.display()))
        .rules
}
