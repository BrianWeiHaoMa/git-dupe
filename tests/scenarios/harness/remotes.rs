//! The workspace of the transfer guards (G18) and of `detach`, and the check that a
//! refusal of one changed nothing anywhere: an attached project with private history whose
//! public repository has remotes and a linked worktree, beside local repositories a remote
//! can name, the private one pushed to where `detach` needs it. Beside it, the second
//! machine `clone` attaches, and the same check over both machines.

use std::ffi::OsStr;
use std::iter;
use std::path::{Path, PathBuf};

use super::attached::daily_state;
use super::output::End;
use super::scenario::Scenario;
use super::tree::Tree;

/// The URL of the public remote `origin`, as configured: a host in mixed case and a path
/// ending in `.git`, never contacted.
pub const PROJECT_URL: &str = "https://Example.com/team/project.git";

/// The three URLs of the public remote `many`: two `url` records and a `pushurl`, never
/// contacted.
pub const MANY_URLS: [&str; 3] = [
    "https://example.net/team/first.git",
    "git@example.net:team/second.git",
    "ssh://example.net/team/third.git",
];

/// A transfer workspace's paths, all canonical.
pub struct Transfer {
    /// The root: `daily_state`, settled once by `git dupe status`. Its public repository
    /// has the remotes `origin` (`PROJECT_URL`), `local` (`public_remote`), and `many`
    /// (`MANY_URLS`), and the linked worktree `linked`.
    pub root: PathBuf,
    /// The public repository's linked worktree, beside the root.
    pub linked: PathBuf,
    /// The bare repository the public remote `local` names, by its absolute path.
    pub public_remote: PathBuf,
    /// A bare repository for private history, beside the root, configured nowhere.
    pub private_remote: PathBuf,
}

impl Scenario {
    /// Makes the transfer workspace `name` below the scenario's directory: the root
    /// `<name>`, the linked worktree `<name>-linked`, and the bare repositories
    /// `<name>-public.git` and `<name>-private.git`.
    pub fn transfer_workspace(&self, name: &str) -> Transfer {
        let at = |suffix: &str| self.dir().join(format!("{name}{suffix}"));
        let transfer = Transfer {
            root: at(""),
            linked: at("-linked"),
            public_remote: at("-public.git"),
            private_remote: at("-private.git"),
        };
        daily_state(self, &transfer.root);
        self.bare_repository(&transfer.public_remote);
        self.bare_repository(&transfer.private_remote);
        self.linked_worktree(&transfer.root, &transfer.linked);
        let public_remote = transfer.public_remote.to_str().expect("a UTF-8 path");
        for words in [
            ["remote", "add", "origin", PROJECT_URL].as_slice(),
            &["remote", "add", "local", public_remote],
            &["remote", "add", "many", MANY_URLS[0]],
            &["config", "--add", "remote.many.url", MANY_URLS[1]],
            &["config", "remote.many.pushurl", MANY_URLS[2]],
        ] {
            self.git(words).from(&transfer.root).succeeds();
        }
        let settled = self.git(["dupe", "status"]).from(&transfer.root).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        transfer
    }

    /// The transfer workspace `name` whose bare `private_remote` is the private remote
    /// `origin`, added by `git dupe remote add` and holding `main` by
    /// `git dupe push -u origin main`: every private commit is on a remote, nothing is
    /// uncommitted, and `git dupe detach` succeeds in it without `--force`.
    pub fn pushed_workspace(&self, name: &str) -> Transfer {
        let transfer = self.transfer_workspace(name);
        let private_remote = transfer.private_remote.to_str().expect("a UTF-8 path");
        for words in [
            ["dupe", "remote", "add", "origin", private_remote].as_slice(),
            &["dupe", "push", "-u", "origin", "main"],
        ] {
            self.git(words).from(&transfer.root).succeeds();
        }
        transfer
    }
}

/// A second machine of the project: beside a pushed workspace, the first machine, a second,
/// unattached clone of the same public project, made by plain `git clone` of the first
/// machine's root, holding the public files and none of the private ones.
pub struct SecondMachine {
    /// The first machine, `pushed_workspace`, whose private remote's `HEAD` names `main`,
    /// as a hosting service's does once its first branch is pushed.
    pub first: Transfer,
    /// The second clone's root. Its public remote `origin` is the first machine's root.
    pub root: PathBuf,
}

impl Scenario {
    /// Makes the second machine `name` below the scenario's directory: the pushed
    /// workspace `name`, then the clone `<name>-second`.
    pub fn second_machine(&self, name: &str) -> SecondMachine {
        let first = self.pushed_workspace(name);
        self.git(["symbolic-ref", "HEAD", "refs/heads/main"])
            .from(&first.private_remote)
            .succeeds();
        let root = self.dir().join(format!("{name}-second"));
        self.public_clone(&first.root, &root);
        SecondMachine { first, root }
    }

    /// Makes `to` a plain `git clone` of the project at `from`: its public history alone.
    pub fn public_clone(&self, from: &Path, to: &Path) {
        let words = [
            OsStr::new("clone"),
            OsStr::new("-q"),
            from.as_os_str(),
            to.as_os_str(),
        ];
        self.git(words).succeeds();
    }
}

impl Transfer {
    /// Every byte below the root, the linked worktree, and each bare repository, the
    /// public `.git`, the private repository, and all of `.git/info/exclude` included:
    /// what a refusal must leave as it was, read in a workspace already settled.
    pub fn everything(&self) -> Everything {
        let places = [
            &self.root,
            &self.linked,
            &self.public_remote,
            &self.private_remote,
        ];
        Everything(
            places
                .into_iter()
                .map(|place| (place.clone(), Tree::of(place)))
                .collect(),
        )
    }
}

impl SecondMachine {
    /// Every byte of both machines: what `Transfer::everything` reads of the first, and
    /// everything below the second clone's root and each of `beside`, the places of its
    /// public repository beside the root, such as a linked worktree.
    pub fn everything(&self, beside: &[&Path]) -> Everything {
        let Everything(mut trees) = self.first.everything();
        let places = iter::once(self.root.as_path()).chain(beside.iter().copied());
        trees.extend(places.map(|place| (place.to_path_buf(), Tree::of(place))));
        Everything(trees)
    }
}

/// Every tree a refusal must leave as it was.
pub struct Everything(Vec<(PathBuf, Tree)>);

impl Everything {
    /// Asserts that nothing below any of the trees changed since they were read.
    pub fn unchanged(&self) {
        for (place, before) in &self.0 {
            let changed = before.changed_in(&Tree::of(place));
            assert!(
                changed.is_empty(),
                "{}: changed {changed:?}",
                place.display()
            );
        }
    }
}
