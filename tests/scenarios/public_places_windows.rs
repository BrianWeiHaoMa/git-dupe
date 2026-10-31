//! The spellings of a public place that Git for Windows opens and no other Git does (G18,
//! `Holds/G18`, S13): a root with `\` and no drive, `/<letter>` and its neighbors, what MinGit's
//! shell reads under `/tmp` and `/proc`, and a public remote spelled through them. Off Windows
//! these words name other paths, so each check reports itself unavailable there.
//!
//! The limits of `Product` are left out, because the guard does not see through them: a
//! `/<path>` read below Git's own installation, `/proc/<number>/cwd/<path>`, and a UNC path to
//! a local drive.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;
use std::{env, fs};

use crate::harness::{
    Scenario, Transfer, Tree, as_git_spells, canonical, refusal, report, second_filesystems,
    under_each_release, word,
};

fn unavailable_here() -> bool {
    let unavailable = word() != "windows";
    if unavailable {
        report("public_places_windows unavailable: it checks Git for Windows's spellings");
    }
    unavailable
}

/// Each of `words`, given to `fetch`, is refused, and nothing changes.
fn each_refused(s: &Scenario, t: &Transfer, words: &[String]) {
    let before = t.everything();
    for word in words {
        let output = s.git(["dupe", "fetch", word]).from(&t.root).run();
        refusal(&output, &[b"git dupe git"]);
        before.unchanged();
    }
}

/// The drive of the path `spelled` as Git spells it, and the rest.
fn drive_and_path(spelled: &str) -> (String, &str) {
    let (drive, path) = spelled.split_once(':').expect("a path with a drive");
    (drive.to_owned(), path)
}

#[test]
fn the_project_under_the_spellings_git_for_windows_opens_is_refused() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let t = s.transfer_workspace("workspace");
        let spelled = as_git_spells(&t.root);
        let (drive, path) = drive_and_path(&spelled);
        let letter = drive.to_ascii_lowercase();
        let backslashed = path.replace('/', "\\");
        let temporary = as_git_spells(&canonical(&env::temp_dir()).unwrap());
        let below_temporary = spelled
            .strip_prefix(temporary.trim_end_matches('/'))
            .expect("a scenario's directory lies below the temporary directory");
        each_refused(
            s,
            &t,
            &[
                backslashed.clone(),
                format!("file://{drive}:{backslashed}"),
                format!("/{letter}{path}"),
                format!("/{letter}{backslashed}"),
                format!("/{drive}:{path}"),
                format!("/./{letter}{path}"),
                format!("/tmp{below_temporary}"),
                format!("{drive}:/"),
                format!("/{letter}"),
                format!("/{drive}:."),
                format!("file://{drive}:"),
                format!("{letter}:."),
                format!("/proc/cygdrive/{letter}{path}"),
                "/proc/self/cwd/".to_owned(),
            ],
        );
    });
}

/// `/g` is the top of the drive `G:`, and a project standing there is the project.
#[test]
fn a_project_at_the_top_of_a_second_volume_is_refused_under_its_letter() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let second = second_filesystems()
            .into_iter()
            .next()
            .expect("the letter way");
        let second = match second.found {
            Ok(second) => second,
            Err(reason) => {
                report(&format!(
                    "the second filesystem {} unavailable: {reason}; the top of a volume left out",
                    second.reached
                ));
                return;
            }
        };
        let top = second.path().parent().expect("a volume's top");
        if [".git", ".gitdupe"]
            .iter()
            .any(|path| top.join(path).exists())
        {
            report(
                "the second filesystem under its own drive letter unavailable: a repository \
                 stands at its top; the top of a volume left out",
            );
            return;
        }
        // Removed whatever the check finds: nothing stood at either path before it.
        let checked = catch_unwind(AssertUnwindSafe(|| at_the_top_of_a_volume(s, top)));
        for path in [".git", ".gitdupe"] {
            let entry = top.join(path);
            if entry.is_dir() {
                fs::remove_dir_all(&entry).unwrap();
            } else if entry.exists() {
                fs::remove_file(&entry).unwrap();
            }
        }
        if let Err(failure) = checked {
            resume_unwind(failure);
        }
    });
}

fn at_the_top_of_a_volume(s: &Scenario, top: &Path) {
    let spelled = as_git_spells(top);
    // A volume's top is the system's, which Git trusts only when told to.
    s.git(["config", "--global", "--add", "safe.directory", &spelled])
        .succeeds();
    s.git(["init", "-q", "-b", "main"]).from(top).succeeds();
    s.init(top);
    s.git(["dupe", "status"]).from(top).succeeds();
    let (drive, _) = drive_and_path(&spelled);
    let letter = drive.to_ascii_lowercase();
    let before = Tree::of(&top.join(".git"));
    for word in [format!("/{letter}"), format!("/{letter}:")] {
        let output = s.git(["dupe", "fetch", &word]).from(top).run();
        refusal(&output, &[b"git dupe git"]);
        let changed = before.changed_in(&Tree::of(&top.join(".git")));
        assert!(changed.is_empty(), "{word}: changed {changed:?}");
    }
}

/// A public remote spelled through the shell's `/proc`, or by its drive, is the same place.
#[test]
fn a_public_remote_under_either_spelling_is_refused_under_the_other() {
    under_each_release(|s| {
        if unavailable_here() {
            return;
        }
        let t = s.transfer_workspace("workspace");
        let spelled = as_git_spells(&t.public_remote);
        let (drive, path) = drive_and_path(&spelled);
        let letter = drive.to_ascii_lowercase();
        let through_proc = format!("/proc/cygdrive/{letter}{path}");
        for (configured, word) in [(&through_proc, &spelled), (&spelled, &through_proc)] {
            s.git(["remote", "set-url", "local", configured])
                .from(&t.root)
                .succeeds();
            each_refused(s, &t, std::slice::from_ref(word));
        }
    });
}
