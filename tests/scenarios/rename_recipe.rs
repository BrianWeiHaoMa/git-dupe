//! The order the manual page's HIDDEN AND PRIVATE gives for renaming a hidden directory,
//! run as it gives it, from the root: `git dupe hide journal/`, then `git dupe mv notes
//! journal`, then `git dupe unhide notes`. From a hidden `notes/` holding a committed file,
//! an unstaged edit of it, and a new file, no step leaves any of them visible to public
//! `git status`, and the private repository ends holding the move. Where `git dupe hide`
//! refuses the new name, which the project's Git tracks a file under, the recipe stops
//! there and nothing has moved; where a rule of the project's re-includes the new name, a
//! warning names it and its rule from the step at which it applies. A public `git mv` of a directory that holds a private file, its new path
//! hidden first, shows the project nothing either, and `git dupe add -A`, with
//! `git dupe add -f` for a moved file the project ignores, records the move. The page holds
//! the recipe as prose, which `manual_page_commands` does not run: this scenario is its
//! run.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, dupe_succeeds, holds, private_commit, under_each_release, write,
};

/// The attached workspace `name` whose `.gitdupe` lists `notes`, holding the committed
/// `notes/saved.md`, edited since and not staged, and the new `notes/draft.md`.
fn fixture(s: &Scenario, name: &str) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let root = s.dir().join(name);
    s.repository(&root);
    s.init(&root);
    write(&root, "notes/saved.md", b"saved\n");
    dupe_succeeds(s, &root, &["add", "notes/"]);
    private_commit(s, &root);
    write(&root, "notes/saved.md", b"edited\n");
    write(&root, "notes/draft.md", b"draft\n");
    root
}

/// What public `git status` lists, every untracked file named.
fn public_status(s: &Scenario, root: &Path) -> String {
    let status = s
        .git(["status", "--porcelain", "-uall"])
        .from(root)
        .succeeds();
    String::from_utf8(status.stdout).unwrap()
}

#[test]
fn renaming_a_hidden_directory_in_the_pages_order_shows_the_project_nothing() {
    under_each_release(|s| {
        let root = fixture(s, "workspace");
        assert_eq!(public_status(s, &root), "");
        for step in [
            &["hide", "journal/"][..],
            &["mv", "notes", "journal"],
            &["unhide", "notes"],
        ] {
            let ran = dupe_succeeds(s, &root, step);
            assert_eq!(public_status(s, &root), "", "after {step:?}: {ran:?}");
            // Nothing is left for the move's hint to name.
            assert!(
                !holds(&ran.stderr, b"move the listing"),
                "{step:?}: {ran:?}"
            );
        }
        assert_eq!(
            fs::read(root.join("journal/saved.md")).unwrap(),
            b"edited\n"
        );
        assert_eq!(fs::read(root.join("journal/draft.md")).unwrap(), b"draft\n");
        assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"journal\n");
        let private = dupe_succeeds(s, &root, &["status", "--porcelain"]);
        let private = String::from_utf8(private.stdout).unwrap();
        for entry in ["notes/saved.md -> journal/saved.md", "?? journal/draft.md"] {
            assert!(private.contains(entry), "{entry}: {private}");
        }
        // A file written there afterwards is hidden too.
        write(&root, "journal/later.md", b"later\n");
        assert_eq!(public_status(s, &root), "");
    });
}

#[test]
fn where_hide_refuses_the_new_name_nothing_has_moved() {
    under_each_release(|s| {
        let root = fixture(s, "workspace");
        write(&root, "journal/shared.md", b"the project's\n");
        s.git(["add", "journal/shared.md"]).from(&root).succeeds();
        s.commit_public(&root);
        let refused = s.git(["dupe", "hide", "journal/"]).from(&root).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"notes\n");
        assert_eq!(fs::read(root.join("notes/saved.md")).unwrap(), b"edited\n");
        assert_eq!(fs::read(root.join("notes/draft.md")).unwrap(), b"draft\n");
        assert_eq!(public_status(s, &root), "");
    });
}

#[test]
fn where_a_rule_of_the_project_re_includes_the_new_name_a_warning_names_it() {
    under_each_release(|s| {
        let root = fixture(s, "workspace");
        write(&root, ".gitignore", b"!/journal/\n");
        s.git(["add", ".gitignore"]).from(&root).succeeds();
        s.commit_public(&root);
        dupe_succeeds(s, &root, &["hide", "journal/"]);
        // Nothing stands at `journal` yet, so the rule for a directory applies at the move.
        let moved = dupe_succeeds(s, &root, &["mv", "notes", "journal"]);
        assert!(holds(&moved.stderr, b"warning: journal"), "{moved:?}");
        assert!(holds(&moved.stderr, b".gitignore:1"), "{moved:?}");
    });
}

#[test]
fn a_public_mv_with_the_new_paths_hidden_first_shows_the_project_nothing() {
    under_each_release(|s| {
        for ignored in [false, true] {
            let root = s.dir().join(format!("public-{ignored}"));
            s.repository(&root);
            write(&root, "mixed/shared.md", b"the project's\n");
            s.git(["add", "mixed/shared.md"]).from(&root).succeeds();
            if ignored {
                write(&root, ".gitignore", b"/renamed/private.md\n");
                s.git(["add", ".gitignore"]).from(&root).succeeds();
            }
            s.commit_public(&root);
            s.init(&root);
            write(&root, "mixed/private.md", b"mine\n");
            dupe_succeeds(s, &root, &["add", "-f", "mixed/private.md"]);
            private_commit(s, &root);
            dupe_succeeds(s, &root, &["hide", "renamed/private.md"]);
            s.git(["mv", "mixed", "renamed"]).from(&root).succeeds();
            let status = public_status(s, &root);
            assert!(!status.contains("private.md"), "{ignored}: {status}");
            dupe_succeeds(s, &root, &["add", "-A"]);
            if ignored {
                dupe_succeeds(s, &root, &["add", "-f", "renamed/private.md"]);
            }
            let tracked = s.private(&root).git(["ls-files"]).succeeds();
            let tracked = String::from_utf8(tracked.stdout).unwrap();
            assert!(
                tracked.contains("renamed/private.md"),
                "{ignored}: {tracked}"
            );
            assert!(
                !tracked.contains("mixed/private.md"),
                "{ignored}: {tracked}"
            );
        }
    });
}
