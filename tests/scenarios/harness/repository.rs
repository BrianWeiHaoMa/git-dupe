//! Repositories for a scenario to stand in, made with the release under test, and the
//! words of git-dupe's own locate run, for a scenario that asks Git itself for the answer
//! git-dupe relays.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::output::End;
use super::scenario::Scenario;

impl Scenario {
    /// Makes `directory` an ordinary repository with one commit, on the branch `main`.
    /// Nothing of git-dupe's is in it: it is unattached.
    pub fn repository(&self, directory: &Path) {
        fs::create_dir_all(directory)
            .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()));
        self.git(["init", "-q", "-b", "main"])
            .from(directory)
            .succeeds();
        // Without maintenance: a commit otherwise leaves a detached `git maintenance`
        // behind, still writing under `.git` while the scenario reads the repository.
        self.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=A Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "first",
        ])
        .from(directory)
        .succeeds();
    }

    /// Makes `directory` a bare repository.
    pub fn bare_repository(&self, directory: &Path) {
        fs::create_dir_all(directory)
            .unwrap_or_else(|cause| panic!("{}: {cause}", directory.display()));
        self.git(["init", "-q", "--bare"])
            .from(directory)
            .succeeds();
    }

    /// Adds a linked worktree of `repository` at `directory`.
    pub fn linked_worktree(&self, repository: &Path, directory: &Path) {
        let words = ["worktree", "add", "-q", "--detach"].map(OsStr::new);
        let made = self
            .git(words.into_iter().chain([directory.as_os_str()]))
            .from(repository)
            .run();
        assert_eq!(made.end, End::Code(0), "{made:?}");
    }

    /// Makes `directory` a project with public files, unattached: the repository above,
    /// then the empty files `README.md`, `docs/design.md`, `docs/api.md`, and
    /// `.gitignore`, committed, and an empty directory `sub`.
    pub fn project(&self, directory: &Path) {
        self.repository(directory);
        fs::create_dir(directory.join("docs")).unwrap();
        fs::create_dir(directory.join("sub")).unwrap();
        for path in ["README.md", "docs/design.md", "docs/api.md", ".gitignore"] {
            fs::write(directory.join(path), b"").unwrap();
        }
        self.git(["add", "--", "README.md", "docs", ".gitignore"])
            .from(directory)
            .succeeds();
        self.commit_public(directory);
    }

    /// Makes `directory` the project of the product's `Done when`, attached by
    /// `git dupe init`: `unattached_project`, then `init`. Nothing is hidden but
    /// `.gitdupe`.
    pub fn attached_project(&self, directory: &Path) {
        self.unattached_project(directory);
        self.init(directory);
    }

    /// Makes `directory` the project of the product's `Done when` before git-dupe is
    /// attached: a repository on `main` whose committed `.gitignore` ignores
    /// `.env.local`, `.vscode/`, and `build/`, beside the committed `README.md` and
    /// `docs/design.md`.
    pub fn unattached_project(&self, directory: &Path) {
        self.repository(directory);
        fs::create_dir(directory.join("docs")).unwrap();
        for (path, content) in [
            (".gitignore", &b".env.local\n.vscode/\nbuild/\n"[..]),
            ("README.md", b"readme\n"),
            ("docs/design.md", b"design\n"),
        ] {
            fs::write(directory.join(path), content).unwrap();
        }
        self.git(["add", "--", ".gitignore", "README.md", "docs/design.md"])
            .from(directory)
            .succeeds();
        self.commit_public(directory);
    }

    /// Commits what the public index holds, without maintenance.
    pub fn commit_public(&self, directory: &Path) {
        self.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "commit",
            "-qm",
            "public files",
        ])
        .from(directory)
        .succeeds();
    }
}

/// The words of the `rev-parse` git-dupe locates the workspace with. The run of a command
/// that attaches also asks whether the directory is a submodule checkout.
pub fn locate_words(attaching: bool) -> Vec<&'static str> {
    let mut words = vec![
        "rev-parse",
        "--path-format=absolute",
        "--git-dir",
        "--git-common-dir",
        "--show-toplevel",
    ];
    if attaching {
        words.push("--show-superproject-working-tree");
    }
    words.push("--show-prefix");
    words
}
