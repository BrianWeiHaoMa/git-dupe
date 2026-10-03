//! A local destination that is `~` or begins with `~/` is read with `HOME` in place of its
//! `~`, as Git opens it (G18, `Holds/G18`, S13): typed, `clone`'s `URL` among them, as a
//! private remote's URL, as a default remote's value, or as a public remote's URL, it
//! names the place Git would reach, and one that names no public place still receives
//! private history, or for `clone` attaches.

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Everything, Output, Scenario, Transfer, Tree, names, unchanged, under_each_release,
};

/// The transfer workspace with the scenario's `HOME` holding `project`, a link to the
/// root, and `public.git`, a link to the bare repository of the public remote `local`.
fn workspace(s: &Scenario) -> (Transfer, PathBuf) {
    let t = s.transfer_workspace("workspace");
    let home = s.dir().join("home");
    symlink(&t.root, home.join("project")).unwrap();
    symlink(&t.public_remote, home.join("public.git")).unwrap();
    (t, home)
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 scenario path")
}

/// Every tree a refusal must leave as it was: the workspace's and the scenario's `HOME`.
struct Watched {
    workspace: Everything,
    home: Tree,
}

fn watched(t: &Transfer, home: &Path) -> Watched {
    Watched {
        workspace: t.everything(),
        home: Tree::of(home),
    }
}

impl Watched {
    fn unchanged(&self, home: &Path) {
        self.workspace.unchanged();
        unchanged(&self.home, home);
    }
}

/// Refused, exit 128, one `fatal:` line naming each of `parts`, nothing on standard
/// output, and nothing changed.
fn refused(s: &Scenario, t: &Transfer, home: &Path, words: &[&str], parts: &[&str]) {
    let before = watched(t, home);
    let output = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(&t.root)
        .run();
    refusal(&output, parts);
    before.unchanged(home);
}

fn refusal(output: &Output, parts: &[&str]) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    for part in parts {
        names(line, part.as_bytes());
    }
}

/// A word beginning with `~/` names the root and the public remote through `HOME`, `~`
/// alone names `HOME` itself, and a remote's word is compared the same way.
#[test]
fn tilde_words_are_read_from_home() {
    under_each_release(|s| {
        let (t, home) = workspace(s);
        let root = text(&t.root);
        for words in [
            &["push", "~/project", "main:leak"][..],
            &["push", "~/project/", "main:leak"],
            &["fetch", "~/project"],
            &["pull", "~/project/.git"],
            &["push", "--repo=~/project", "main:leak"],
            &["remote", "add", "leak", "~/project"],
        ] {
            refused(s, &t, &home, words, &[root, "git dupe git"]);
        }
        for word in ["~/public.git", "~/public", "~/public/"] {
            let words = ["push", word, "main:leak"];
            refused(s, &t, &home, &words, &["remote 'local'", "git dupe git"]);
        }
        let before = watched(&t, &home);
        let output = s
            .git(["dupe", "push", "~", "main:leak"])
            .from(&t.root)
            .variable("HOME", &t.root)
            .run();
        refusal(&output, &[root, "git dupe git"]);
        before.unchanged(&home);
    });
}

/// A private remote's URL and a default remote's value beginning with `~/` are read from
/// `HOME` too.
#[test]
fn tilde_in_the_private_configuration_is_read_from_home() {
    under_each_release(|s| {
        let (t, home) = workspace(s);
        let root = text(&t.root);
        let private = s.private(&t.root);
        private
            .git(["remote", "add", "home", "~/project"])
            .succeeds();
        for words in [&["push", "home", "main:leak"][..], &["fetch", "home"]] {
            let parts = [root, "private remote 'home'", "git dupe remote remove"];
            refused(s, &t, &home, words, &parts);
        }
        private.git(["remote", "remove", "home"]).succeeds();
        private
            .git(["config", "branch.main.remote", "~/project"])
            .succeeds();
        for command in ["fetch", "pull"] {
            let parts = [root, "branch.main.remote", "given no repository"];
            refused(s, &t, &home, &[command], &parts);
        }
        private
            .git(["config", "--unset", "branch.main.remote"])
            .succeeds();
        private
            .git(["config", "remote.pushDefault", "~/public"])
            .succeeds();
        let parts = [
            "remote 'local'",
            "remote.pushdefault",
            "given no repository",
        ];
        refused(s, &t, &home, &["push"], &parts);
    });
}

/// A public remote whose URL begins with `~/` is named by the path Git opens for it, and a
/// destination beginning with `~/` that names no public place receives private history.
#[test]
fn tilde_in_a_public_url_and_a_private_destination_from_home() {
    under_each_release(|s| {
        let (t, home) = workspace(s);
        let elsewhere = home.join("elsewhere.git");
        s.bare_repository(&elsewhere);
        s.git(["remote", "add", "homeward", "~/elsewhere.git"])
            .from(&t.root)
            .succeeds();
        let without_git = text(&elsewhere).strip_suffix(".git").unwrap();
        for word in [text(&elsewhere), without_git] {
            let words = ["push", word, "main:leak"];
            refused(s, &t, &home, &words, &["remote 'homeward'", "git dupe git"]);
        }

        let mine = home.join("mine.git");
        s.bare_repository(&mine);
        s.git(["dupe", "push", "~/mine.git", "main"])
            .from(&t.root)
            .succeeds();
        let head = s.private(&t.root).git(["rev-parse", "HEAD"]).succeeds();
        let arrived = s
            .git(["rev-parse", "refs/heads/main"])
            .from(&mine)
            .succeeds();
        assert_eq!(head.stdout, arrived.stdout);
    });
}

/// Clone compares home-relative public places as their absolute paths (G18, G2),
/// while a home-relative private URL attaches and writes private files (G2).
#[test]
fn clone_urls_from_home_refuse_public_places_and_attach_private_history() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let home = s.dir().join("home");
        for (link, destination) in [
            ("second", &m.root),
            ("origin", &m.first.root),
            ("private.git", &m.first.private_remote),
        ] {
            symlink(destination, home.join(link)).unwrap();
            assert_eq!(
                std::fs::canonicalize(home.join(link)).unwrap(),
                std::fs::canonicalize(destination).unwrap()
            );
        }
        let before = Tree::of(&m.root);
        let first = m.first.everything();
        let home_before = Tree::of(&home);
        for (word, absolute) in [("~/second", &m.root), ("~/origin", &m.first.root)] {
            for url in [text(absolute), word] {
                let output = s.git(["dupe", "clone", url]).from(&m.root).run();
                refusal(&output, &[url, "git dupe git"]);
                unchanged(&before, &m.root);
                first.unchanged();
                unchanged(&home_before, &home);
                assert!(!m.root.join(".git/dupe").exists());
            }
        }

        s.git(["dupe", "clone", "~/private.git"])
            .from(&m.root)
            .succeeds();
        assert!(m.root.join(".git/dupe").is_dir());
        for path in [
            ".gitdupe",
            "notes/a.md",
            ".env.local",
            ".vscode/settings.json",
            "docs/notes.md",
        ] {
            assert_eq!(
                std::fs::read(m.root.join(path)).unwrap(),
                std::fs::read(m.first.root.join(path)).unwrap(),
                "{path}"
            );
        }
    });
}
