//! The product's `Done when`, the Windows half of its `core.autocrlf` bullet, and `In use` 12
//! through it, as one story on one project: a path spelled with `\` or a drive (G29), a
//! destination spelled so (G18), a `.gitdupe` Windows PowerShell 5.1's `>` wrote (F5, S23), and
//! a `clean` pathspec read with `\` as a separator (G16, G29, S20). And `git dupe --help`,
//! which is Git's own failure there (G24). Off Windows these spellings mean something else, so
//! each check reports itself unavailable there.

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Scenario, Tree, as_git_spells, holds, names, refusal, region_rules, report,
    under_each_release, word, write,
};

fn unavailable_here() -> bool {
    let unavailable = word() != "windows";
    if unavailable {
        report("done_when_windows unavailable: it checks Windows's own spellings, on Windows");
    }
    unavailable
}

/// `path` as a developer types it at a Windows prompt: `\` throughout, the drive upper case.
fn typed_at_a_prompt(path: &Path) -> String {
    let mut typed = as_git_spells(path).replace('/', "\\");
    typed[..1].make_ascii_uppercase();
    typed
}

/// `path` with `/` throughout and the drive in lower case.
fn in_lower_case_drive(path: &Path) -> String {
    let mut spelled = as_git_spells(path);
    spelled[..1].make_ascii_lowercase();
    spelled
}

/// A destination naming the project, refused with nothing sent, fetched, or changed.
fn destination_refused(s: &Scenario, root: &Path, words: &[&str]) {
    let before = Tree::of(root);
    let output = s.git(words).from(root).run();
    refusal(&output, &[b"git dupe git"]);
    let changed = before.changed_in(&Tree::of(root));
    assert!(changed.is_empty(), "{words:?}: changed {changed:?}");
}

#[test]
fn the_spellings_of_windows_mean_what_they_name() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        write(&root, "notes/today.md", b"today\n");
        fs::create_dir(root.join("scratch")).unwrap();

        // `.\notes\`, as PowerShell's tab completion writes it, is `notes/`.
        let added = s.git(["dupe", "add", r".\notes\"]).from(&root).succeeds();
        names(added.only_line("hint"), b"notes");
        assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"notes\n");

        // An absolute path with `\`, from any directory of the project.
        let scratch = format!(r"{}\scratch", typed_at_a_prompt(&root));
        let hidden = s
            .git(["dupe", "hide", &scratch])
            .from(&root.join("docs"))
            .succeeds();
        names(hidden.only_line("hint"), b"scratch");
        assert_eq!(
            fs::read(root.join(".gitdupe")).unwrap(),
            b"notes\nscratch\n"
        );

        // A drive and no root is a usage error.
        let drive = &typed_at_a_prompt(&root)[..2];
        let no_root = format!("{drive}notes");
        let refused = s.git(["dupe", "hide", &no_root]).from(&root).run();
        assert_eq!(refused.end, End::Code(129), "{refused:?}");
        let error = refused.lines("error");
        assert_eq!(error.len(), 1, "{refused:?}");
        names(error[0], no_root.as_bytes());
        assert!(
            holds(&refused.stderr, b"usage: git dupe hide"),
            "{refused:?}"
        );
        assert_eq!(
            fs::read(root.join(".gitdupe")).unwrap(),
            b"notes\nscratch\n"
        );

        // The project under either spelling is a public place.
        let typed = typed_at_a_prompt(&root);
        destination_refused(s, &root, &["dupe", "fetch", &typed]);
        let lower = in_lower_case_drive(&root);
        destination_refused(s, &root, &["dupe", "push", &lower, "main:leak"]);

        // What PowerShell 5.1's `>` writes begins with a UTF-16 mark and cannot be read:
        // the region holds what it held, and `clean` is refused naming the file.
        let utf_16: Vec<u8> = "scratch\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        write(&root, ".gitdupe", &[b"\xff\xfe", &utf_16[..]].concat());
        let status = s.git(["dupe", "status"]).from(&root).succeeds();
        let warned = status.lines("warning");
        let warned: Vec<_> = warned
            .iter()
            .filter(|line| holds(line, b".gitdupe"))
            .collect();
        assert_eq!(warned.len(), 1, "{status:?}");
        assert!(region_rules(&root).contains(&b"/scratch".to_vec()));
        write(&root, r"scratch\x", b"private\n");
        let visible = s.git(["status", "--porcelain"]).from(&root).succeeds();
        assert!(!holds(&visible.stdout, b"scratch/"), "{visible:?}");
        let refused = s.git(["dupe", "clean", "-n"]).from(&root).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        let fatal = refused.lines("fatal");
        assert_eq!(fatal.len(), 1, "{refused:?}");
        names(fatal[0], b".gitdupe");

        // A hidden path below a directory that is not hidden: the developer rewrites the
        // file, as `hide` refuses to while it is unreadable, and one command settles it, so
        // that what the two `clean`s print differs by nothing that command leaves behind.
        write(&root, ".gitdupe", b"notes\ntmp/notes\n");
        write(&root, r"tmp\notes\x", b"hidden\n");
        write(&root, r"tmp\other", b"other\n");
        s.git(["dupe", "status"]).from(&root).succeeds();
        assert!(region_rules(&root).contains(&b"/tmp/notes".to_vec()));
        let typed_clean = s.git(["dupe", "clean", "-n", r".\tmp\"]).from(&root).run();
        let plain_clean = s.git(["dupe", "clean", "-n", "tmp/"]).from(&root).run();
        assert_eq!(typed_clean, plain_clean);
        assert_eq!(plain_clean.end, End::Code(0), "{plain_clean:?}");
        assert!(holds(&plain_clean.stdout, b"tmp/other"), "{plain_clean:?}");

        // `*` reaches git-dupe as typed, no shell expanding it (S20).
        let before = Tree::of(&root);
        let star = s.git(["dupe", "clean", "-n", "tmp/*"]).from(&root).run();
        assert_eq!(star.end, End::Code(128), "{star:?}");
        let fatal = star.only_line("fatal");
        names(fatal, b"tmp/*");
        names(fatal, b"tmp/notes");
        names(fatal, b"tmp");
        let changed = before.changed_in(&Tree::of(&root));
        assert!(changed.is_empty(), "changed {changed:?}");
    });
}

/// No `man`, and no page: `--help` is Git's own failure, and its text is not read here.
#[test]
fn help_as_an_option_is_gits_own_failure() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        let help = s.git(["dupe", "--help"]).from(&root).run();
        assert_eq!(help.end, End::Code(128), "{help:?}");
        assert!(help.stdout.is_empty(), "{help:?}");
    });
}
