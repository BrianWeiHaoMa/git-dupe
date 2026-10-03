//! `.gitdupe` as the privately tracked file it is: staged by `hide`, `unhide`, and `add`
//! as Git's `add` stages any file, merged as Git merges any file, and the hidden paths
//! following what stands on disk (F5).

use std::fs;

use crate::harness::{
    daily_state, gitdupe_written_and_staged, private_commit, region_rules, staged_gitdupe,
    under_each_release, write,
};

/// While Git assumes `.gitdupe`'s index entry unchanged, `hide`, `add`, and `unhide`
/// write the file, the region follows it, and the staged version stays as Git's `add`
/// leaves such an entry; once Git looks at the entry again, `add` stages the file.
#[test]
fn an_entry_git_assumes_unchanged_stays_staged_as_it_was_while_the_file_hides() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        let staged = b"notes\n.vscode\n";
        s.git(["dupe", "update-index", "--assume-unchanged", ".gitdupe"])
            .from(&dir)
            .succeeds();

        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        write(&dir, "tools/run.sh", b"private\n");
        s.git(["dupe", "add", "tools"]).from(&dir).succeeds();
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        let written = b".vscode\nscratch\ntools\n";
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), written);
        assert_eq!(staged_gitdupe(s, &dir).as_deref(), Some(&staged[..]));
        // `notes` stays hidden through its privately tracked file alone.
        assert_eq!(
            region_rules(&dir),
            [
                b"/.env.local".as_slice(),
                b"/.gitdupe",
                b"/.vscode",
                b"/docs/notes.md",
                b"/notes/a.md",
                b"/scratch",
                b"/tools",
            ]
        );

        s.git(["dupe", "update-index", "--no-assume-unchanged", ".gitdupe"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "add", ".gitdupe"]).from(&dir).succeeds();
        gitdupe_written_and_staged(s, &dir, written);
    });
}

/// Two branches that each hide a path merge as Git merges any privately tracked file, and
/// the merged `.gitdupe` decides the hidden paths.
#[test]
fn a_merge_of_two_branches_hiding_paths_is_gits_and_hides_what_the_merged_file_lists() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        s.git(["dupe", "switch", "-q", "-c", "unhiding"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "unhide", "notes"]).from(&dir).succeeds();
        private_commit(s, &dir);
        s.git(["dupe", "switch", "-q", "main"])
            .from(&dir)
            .succeeds();
        s.git(["dupe", "hide", "scratch"]).from(&dir).succeeds();
        private_commit(s, &dir);

        s.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "dupe",
            "merge",
            "-q",
            "--no-edit",
            "unhiding",
        ])
        .from(&dir)
        .succeeds();
        gitdupe_written_and_staged(s, &dir, b".vscode\nscratch\n");
        assert_eq!(
            region_rules(&dir),
            [
                b"/.env.local".as_slice(),
                b"/.gitdupe",
                b"/.vscode",
                b"/docs/notes.md",
                b"/notes/a.md",
                b"/scratch",
            ]
        );
    });
}
