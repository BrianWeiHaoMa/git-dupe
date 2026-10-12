//! The product's `Done when`, its bullet on macOS's default volume, which equates spellings
//! that differ in case: `notes` hidden before anything stands at it, the directory then
//! made as `Notes` (`Product`, terms and the sixth accepted limit; F3, G7, G11, G12, G13,
//! G16, G18; S18). Git compares the spellings as its ignore rules read them, git-dupe byte
//! for byte. The check names that volume, so it runs on macOS alone, where the scenario's
//! own directory equates spellings, and reports itself unavailable everywhere else.

use std::ffi::OsStr;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    End, holds, names, privately_tracked, records, region_rules, report, under_each_release,
    watched_dupe_succeeds, word, write,
};

/// Whether `dir` equates spellings that differ in case: an `a` made there is found as `A`.
fn equates_spellings(dir: &Path) -> bool {
    let made = dir.join("a");
    fs::write(&made, b"").unwrap_or_else(|cause| panic!("{}: {cause}", made.display()));
    let found = match fs::symlink_metadata(dir.join("A")) {
        Ok(_) => true,
        Err(cause) if cause.kind() == ErrorKind::NotFound => false,
        Err(cause) => panic!("{}: {cause}", dir.join("A").display()),
    };
    fs::remove_file(&made).unwrap_or_else(|cause| panic!("{}: {cause}", made.display()));
    found
}

/// Whether `text` names `notes` in any spelling of its case.
fn names_notes(text: &[u8]) -> bool {
    holds(&text.to_ascii_lowercase(), b"notes")
}

#[test]
fn notes_hidden_before_it_stands_hides_the_directory_made_as_capital_notes() {
    under_each_release(|s| {
        if word() != "macos" {
            report("macos_volume unavailable: it checks macOS's default volume, on macOS alone");
            return;
        }
        if !equates_spellings(s.dir()) {
            report(
                "macos_volume unavailable: the scenarios' directory distinguishes case, \
                 where macOS's default volume does not",
            );
            return;
        }
        let root = s.dir().join("project");
        s.attached_project(&root);
        // Git found the filesystem equating spellings when it made the repository (S18).
        let ignore_case = s.git(["config", "core.ignoreCase"]).from(&root).succeeds();
        assert_eq!(ignore_case.stdout, b"true\n", "{ignore_case:?}");

        watched_dupe_succeeds(s, &root, &["hide", "notes/"]);
        write(&root, "Notes/draft.md", b"draft\n");
        // The rule for `notes` and none for `Notes`.
        assert_eq!(region_rules(&root), [&b"/.gitdupe"[..], b"/notes"]);

        // Neither public Git nor `git dupe status` names anything under it.
        let status = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(&root)
            .succeeds();
        assert!(!names_notes(&status.stdout), "{status:?}");
        let add = s.git(["add", "-A", "--dry-run"]).from(&root).succeeds();
        assert!(!names_notes(&add.stdout), "{add:?}");
        let private = watched_dupe_succeeds(s, &root, &["status", "--porcelain"]);
        assert!(!names_notes(&private.stdout), "{private:?}");

        // `clean -fdx` from the root deletes an untracked file and leaves `Notes/` with its
        // file, spelled as it was made.
        write(&root, "scratch.txt", b"scratch\n");
        watched_dupe_succeeds(s, &root, &["clean", "-fdx"]);
        assert!(!root.join("scratch.txt").exists());
        assert_eq!(fs::read(root.join("Notes/draft.md")).unwrap(), b"draft\n");
        let entries: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(entries.iter().any(|name| name == "Notes"), "{entries:?}");
        assert_eq!(region_rules(&root), [&b"/.gitdupe"[..], b"/notes"]);

        // `add Notes/draft.md` makes the file privately tracked, and the next status shows
        // it staged.
        watched_dupe_succeeds(s, &root, &["add", "Notes/draft.md"]);
        assert!(
            privately_tracked(s, &root).contains(&b"Notes/draft.md".to_vec()),
            "{:?}",
            privately_tracked(s, &root)
        );
        let staged = watched_dupe_succeeds(s, &root, &["status", "--porcelain"]);
        assert!(
            records(&staged.stdout, b'\n').contains(&&b"A  Notes/draft.md"[..]),
            "{staged:?}"
        );

        // `hide Notes/` lists `Notes` beside `notes`: another path to git-dupe.
        let hide = watched_dupe_succeeds(s, &root, &["hide", "Notes/"]);
        let hint = hide.only_line("hint");
        names(hint, b"Notes");
        names(hint, b"git dupe unhide");
        assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"notes\nNotes\n");
        assert_eq!(
            region_rules(&root),
            [&b"/.gitdupe"[..], b"/Notes", b"/notes"]
        );

        // A destination naming the root in another case names the project's working tree.
        let other_case = s.dir().join("PROJECT");
        let words = [
            OsStr::new("dupe"),
            OsStr::new("push"),
            other_case.as_os_str(),
            OsStr::new("main:leak"),
        ];
        let push = s.git(words).from(&root).run();
        assert_eq!(push.end, End::Code(128), "{push:?}");
        let refusal = push.only_line("fatal");
        names(refusal, root.as_os_str().as_bytes());
        names(refusal, b"git dupe git");
    });
}
