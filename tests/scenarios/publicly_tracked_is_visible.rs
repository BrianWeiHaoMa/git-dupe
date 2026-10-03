//! A path the public index tracks is one public Git does not ignore, whatever an ignore
//! rule says, because Git applies no ignore rule to a path its index tracks (G7): settle,
//! a release, and `detach` name it as visible, and a path tracked by both is named by
//! G8's warning alone (G3, G6, G8, G9).

use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, detached, now_visible, under_each_release, warnings, warnings_in_any_order,
    write,
};

/// The project of the product's `Done when`, attached, with `.env.local`, which its
/// `.gitignore` ignores, and `notes.md`, which no rule names, each added by `git dupe add`
/// and then by a plain `git add -f`: tracked by both, each named on every command by G8's
/// warning alone.
fn tracked_by_both(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    s.attached_project(&dir);
    write(&dir, ".env.local", b"SECRET=1\n");
    write(&dir, "notes.md", b"notes\n");
    let added = s
        .git(["dupe", "add", "-f", ".env.local", "notes.md"])
        .from(&dir)
        .run();
    assert_eq!(added.end, End::Code(0), "{added:?}");
    s.git(["add", "-f", "--", ".env.local", "notes.md"])
        .from(&dir)
        .succeeds();
    both_named_alone(s, &dir);
    dir
}

fn both_named_alone(s: &Scenario, dir: &Path) {
    let status = s.git(["dupe", "status"]).from(dir).run();
    assert_eq!(status.end, End::Code(0), "{status:?}");
    warnings_in_any_order(
        &status,
        &[
            &[b".env.local", b"tracked by both"],
            &[b"notes.md", b"tracked by both"],
        ],
    );
}

#[test]
fn detach_names_a_publicly_tracked_path_a_rule_ignores_as_visible() {
    under_each_release(|s| {
        let dir = tracked_by_both(s, "workspace");
        let output = s.git(["dupe", "detach", "--force"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        warnings(
            &output,
            vec![now_visible(".env.local"), now_visible("notes.md")],
        );
        detached(&dir);
    });
}

#[test]
fn a_release_names_a_publicly_tracked_path_a_rule_ignores_as_visible() {
    under_each_release(|s| {
        let dir = tracked_by_both(s, "workspace");
        let output = s
            .git(["dupe", "rm", "-q", "--cached", ".env.local", "notes.md"])
            .from(&dir)
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        warnings_in_any_order(
            &output,
            &[
                &[b".env.local", b"visible to public Git"],
                &[b"notes.md", b"visible to public Git"],
            ],
        );
        // Released once: the next command starts from a region without them.
        let next = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(next.end, End::Code(0), "{next:?}");
        warnings_in_any_order(&next, &[]);
    });
}

#[test]
fn a_listed_path_the_project_tracks_is_named_on_every_command() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"README.md\n");
        for from in [dir.clone(), dir.join("docs")] {
            let output = s.git(["dupe", "status"]).from(&from).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            warnings_in_any_order(&output, &[&[b"README.md"]]);
        }
    });
}

/// A rule that re-includes a path both repositories track decides nothing for public
/// Git, which tracks it: G8's warning is the one that names it.
#[test]
fn a_path_tracked_by_both_is_named_by_one_warning_whatever_a_rule_says() {
    under_each_release(|s| {
        let dir = tracked_by_both(s, "workspace");
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!notes.md\n",
        );
        both_named_alone(s, &dir);
    });
}
