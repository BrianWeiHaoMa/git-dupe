//! `.gitdupe`'s line in the private repository's own `info/attributes` (F2, G1): `init`
//! and `clone` leave `/.gitdupe -text merge=union` there, after a template's lines, in
//! every worktree's private repository; `init` run again adds it where the file lacks it
//! and writes nothing where it holds it; and Git then converts no line ending of the list,
//! so that `add` and `hide` succeed under `core.autocrlf=true` with `core.safecrlf=true`.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::harness::{
    End, Scenario, completed_hint, gitdupe_written_and_staged, linked_on_branch, names, symlink,
    under_each_release, worktree_clone, write,
};

/// The line F2 names, on its line.
const LINE: &[u8] = b"/.gitdupe -text merge=union\n";

/// The private repository's `info/attributes` below the private Git directory `private`.
fn attributes(private: &Path) -> Option<Vec<u8>> {
    fs::read(private.join("info/attributes")).ok()
}

/// What Git reads for `.gitdupe`'s `text` and `merge` in the workspace at `dir`, through
/// `git dupe check-attr -z`: `unset` and `union` where the line decides.
fn read_by_git(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git([
        "dupe",
        "check-attr",
        "-z",
        "text",
        "merge",
        "--",
        ".gitdupe",
    ])
    .from(dir)
    .succeeds()
    .stdout
}

const UNSET_AND_UNION: &[u8] = b".gitdupe\0text\0unset\0.gitdupe\0merge\0union\0";

#[test]
fn init_and_clone_leave_the_line_in_every_worktrees_private_repository() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.unattached_project(&dir);
        // The project's own attributes, which the private repository reads in the same
        // working tree, are outranked by the line.
        write(
            &dir,
            ".gitattributes",
            b"* text=auto\n/.gitdupe text eol=crlf\n",
        );
        s.git(["add", ".gitattributes"]).from(&dir).succeeds();
        s.commit_public(&dir);
        s.git(["dupe", "init"]).from(&dir).succeeds();
        assert_eq!(attributes(&dir.join(".git/dupe")).as_deref(), Some(LINE));
        assert_eq!(read_by_git(s, &dir), UNSET_AND_UNION);

        let m = s.second_machine("machine");
        let url = m.first.private_remote.to_str().unwrap();
        s.git(["dupe", "clone", url]).from(&m.root).succeeds();
        assert_eq!(attributes(&m.root.join(".git/dupe")).as_deref(), Some(LINE));
        assert_eq!(read_by_git(s, &m.root), UNSET_AND_UNION);

        let linked = linked_on_branch(s, &m.first.root, &s.dir().join("agent"), "agent");
        worktree_clone(s, &linked, &m.first.private_remote);
        assert_eq!(
            attributes(&linked.private_directory()).as_deref(),
            Some(LINE)
        );
        assert_eq!(read_by_git(s, &linked.root), UNSET_AND_UNION);
    });
}

/// A template's `info/attributes` keeps its bytes before the line, with and without a final
/// newline, and with a later line of its own for `.gitdupe`, which the line outranks; one
/// that holds the line already gets none added.
#[test]
fn a_templates_attributes_keep_their_lines_before_the_line() {
    under_each_release(|s| {
        for (index, (held, left)) in [
            (
                &b"*.txt text\n"[..],
                &b"*.txt text\n/.gitdupe -text merge=union\n"[..],
            ),
            (b"*.txt text", b"*.txt text\n/.gitdupe -text merge=union\n"),
            (
                b"* text eol=crlf merge=binary\n/.gitdupe text\n",
                b"* text eol=crlf merge=binary\n/.gitdupe text\n/.gitdupe -text merge=union\n",
            ),
            (
                b"/.gitdupe -text merge=union\n*.txt text\n",
                b"/.gitdupe -text merge=union\n*.txt text\n",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let template = s.dir().join(format!("template-{index}"));
            write(&template, "info/attributes", held);
            let mut template_dir = OsString::from("init.templateDir=");
            template_dir.push(&template);
            let dir = s.dir().join(format!("project-{index}"));
            s.unattached_project(&dir);
            let words = [
                OsStr::new("-c"),
                &template_dir,
                OsStr::new("dupe"),
                OsStr::new("init"),
            ];
            let init = s.git(words).from(&dir).run();
            assert_eq!(init.end, End::Code(0), "{init:?}");
            assert_eq!(
                attributes(&dir.join(".git/dupe")).as_deref(),
                Some(left),
                "{index}"
            );
            assert_eq!(read_by_git(s, &dir), UNSET_AND_UNION, "{index}");
        }
    });
}

/// Run again where the file holds the line, `init` writes nothing to it and says nothing
/// of it; where the file lacks the line, as a release that wrote none left it, `init` adds
/// it after the lines there and its hint names it among what it completed.
#[test]
fn init_run_again_adds_the_line_only_where_the_file_lacks_it() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let private = dir.join(".git/dupe");
        let file = private.join("info/attributes");
        // A fixed time, as the filesystem stores it, which a write of the file would change.
        File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000))
            .unwrap();
        let stored = fs::metadata(&file).unwrap().modified().unwrap();
        let again = s.git(["dupe", "init"]).from(&dir).run();
        completed_hint(&again, false);
        assert_eq!(attributes(&private).as_deref(), Some(LINE));
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), stored);

        for (index, (held, left)) in [
            (None, LINE),
            (Some(&b""[..]), LINE),
            (
                Some(b"*.png binary"),
                b"*.png binary\n/.gitdupe -text merge=union\n",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            match held {
                None => fs::remove_file(&file).unwrap(),
                Some(held) => fs::write(&file, held).unwrap(),
            }
            let again = s.git(["dupe", "init"]).from(&dir).run();
            assert_eq!(again.end, End::Code(0), "{index}: {again:?}");
            names(again.only_line("hint"), b"already attached");
            names(
                again.only_line("hint"),
                b"completed its info/attributes line for .gitdupe",
            );
            assert_eq!(attributes(&private).as_deref(), Some(left), "{index}");
            assert_eq!(read_by_git(s, &dir), UNSET_AND_UNION, "{index}");
        }

        // With `info/` gone too, as an empty template directory leaves the repository.
        fs::remove_dir_all(private.join("info")).unwrap();
        let again = s.git(["dupe", "init"]).from(&dir).run();
        assert_eq!(again.end, End::Code(0), "{again:?}");
        assert_eq!(attributes(&private).as_deref(), Some(LINE));
    });
}

/// Under `core.autocrlf=true` and `core.safecrlf=true`, where Git refuses to stage a file
/// whose line endings it would convert, `add` and `hide` list each path and stage the
/// list, and the staged list checks out as written, LF and all.
#[test]
fn add_and_hide_stage_the_list_under_autocrlf_and_safecrlf() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[core]\n\tautocrlf = true\n\tsafecrlf = true\n",
        )
        .unwrap();
        // A private file with no line ending, which Git stages under either setting.
        write(&dir, "notes/a.md", b"private");
        let add = s.git(["dupe", "add", "notes/"]).from(&dir).run();
        assert_eq!(add.end, End::Code(0), "{add:?}");
        let hide = s.git(["dupe", "hide", "scratch"]).from(&dir).run();
        assert_eq!(hide.end, End::Code(0), "{hide:?}");
        gitdupe_written_and_staged(s, &dir, b"notes\nscratch\n");

        fs::remove_file(dir.join(".gitdupe")).unwrap();
        s.git(["dupe", "checkout", "--", ".gitdupe"])
            .from(&dir)
            .succeeds();
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\nscratch\n");
    });
}

/// A symbolic link standing at the private repository's `info`, to a directory outside it
/// that others share, is written through by nothing (R8): where the file it reaches lacks
/// the line, `init` refuses naming the link's path and the shared file stays as it was;
/// where that file holds the line, `init` finds nothing missing.
#[test]
fn a_link_at_the_private_info_is_written_through_by_nothing() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let private = dir.join(".git/dupe");
        let shared = s.dir().join("shared");
        write(&shared, "attributes", b"*.png binary\n");
        fs::remove_dir_all(private.join("info")).unwrap();
        symlink(&shared, private.join("info")).unwrap();

        let refused = s.git(["dupe", "init"]).from(&dir).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        let fatal = refused.lines("fatal");
        assert_eq!(fatal.len(), 1, "{refused:?}");
        names(fatal[0], b"info");
        assert_eq!(
            fs::read(shared.join("attributes")).unwrap(),
            b"*.png binary\n"
        );

        fs::write(shared.join("attributes"), LINE).unwrap();
        let again = s.git(["dupe", "init"]).from(&dir).run();
        completed_hint(&again, false);
        assert_eq!(fs::read(shared.join("attributes")).unwrap(), LINE);
    });
}

/// Where the line cannot be written, a template having put a file at `info`, each command
/// refuses naming the path and the way on that fits it: `init` run again completes an
/// `init`, and `clone`, whose steps after the line did not run, starts over after
/// `git dupe detach --force` (G2).
#[test]
fn a_line_that_cannot_be_written_refuses_naming_the_way_on() {
    under_each_release(|s| {
        let template = s.dir().join("template");
        write(&template, "info", b"a file where a directory goes\n");
        let mut template_dir = OsString::from("init.templateDir=");
        template_dir.push(&template);

        let dir = s.dir().join("project");
        s.unattached_project(&dir);
        let words = [
            OsStr::new("-c"),
            &template_dir,
            OsStr::new("dupe"),
            OsStr::new("init"),
        ];
        let init = s.git(words).from(&dir).run();
        assert_eq!(init.end, End::Code(128), "{init:?}");
        let fatal = init.lines("fatal");
        assert_eq!(fatal.len(), 1, "{init:?}");
        names(fatal[0], b"info");
        names(fatal[0], b"'git dupe init'");

        let m = s.second_machine("machine");
        let url = m.first.private_remote.as_os_str();
        let words = [
            OsStr::new("-c"),
            &template_dir,
            OsStr::new("dupe"),
            OsStr::new("clone"),
            url,
        ];
        let clone = s.git(words).from(&m.root).run();
        assert_eq!(clone.end, End::Code(128), "{clone:?}");
        let fatal = clone.lines("fatal");
        assert_eq!(fatal.len(), 1, "{clone:?}");
        names(fatal[0], b"info");
        names(fatal[0], b"'git dupe detach --force'");
        names(fatal[0], b"'git dupe clone'");
        assert!(!m.root.join("notes/a.md").exists(), "{clone:?}");
    });
}
