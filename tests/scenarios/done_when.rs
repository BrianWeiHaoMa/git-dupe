//! The product's `Done when`: its first two bullets as one story on one project, and its
//! third as another, each in the order written. Every `git dupe` command of them leaves
//! the public `.git` as it was but for `.git/dupe` and the exclude file, and when the first
//! story ends the public repository holds nothing private.

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    EVERY_OBJECT, End, Output, Scenario, Tree, changed_since, held_publicly, holds,
    leaving_public_git, names, names_number, privately_tracked, public_git, publicly_tracked,
    records, region_rules, under_each_release, warnings_in_any_order, write,
};

/// The developer's own text in `.git/info/exclude`, before `init` and after the region.
const BEFORE: &[u8] = b"# the developer's own rules\n*.orig\n";
const AFTER: &[u8] = b"*.rej\n";

/// The exclude file once `notes/`, `.env.local`, `.vscode/`, and `docs/notes.md` are
/// private: the developer's bytes, the markers, and exactly the five rules in byte order.
fn exclude_with_the_five_rules() -> Vec<u8> {
    [
        BEFORE,
        b"# BEGIN git-dupe\n/.env.local\n/.gitdupe\n/.vscode\n/docs/notes.md\n/notes\n# END git-dupe\n",
        AFTER,
    ]
    .concat()
}

/// The paths only the managed region hides from public Git: the project's own
/// `.gitignore` already ignores `.env.local` and `.vscode/`, so their absence from a
/// public listing shows nothing about git-dupe.
const ONLY_THE_REGION_HIDES: [&[u8]; 3] = [b"notes", b"docs/notes.md", b".gitdupe"];

/// `git dupe <words>` from `from` in the workspace at `dir`, the public `.git` watched.
fn dupe(s: &Scenario, dir: &Path, from: &Path, words: &[&str]) -> Output {
    leaving_public_git(dir, s.git(["dupe"].iter().chain(words)).from(from))
}

/// `dupe`, from the root, which must exit 0.
fn dupe_succeeds(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let output = dupe(s, dir, dir, words);
    assert_eq!(output.end, End::Code(0), "git dupe {words:?}: {output:?}");
    output
}

/// The paths of public `git status --porcelain`, every untracked file listed by itself.
fn public_status(s: &Scenario, dir: &Path) -> Vec<Vec<u8>> {
    let status = s
        .git(["status", "--porcelain", "--untracked-files=all"])
        .from(dir)
        .succeeds();
    paths_of(&status.stdout)
}

/// The entries of porcelain status, each its two-letter code, a space, and its path.
fn entries(porcelain: &[u8]) -> Vec<Vec<u8>> {
    records(porcelain, b'\n')
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

/// The paths of porcelain status entries.
fn paths_of(porcelain: &[u8]) -> Vec<Vec<u8>> {
    entries(porcelain)
        .into_iter()
        .map(|entry| entry[3..].to_vec())
        .collect()
}

/// Whether `path` is one of `ONLY_THE_REGION_HIDES` or lies below one.
fn hidden_by_the_region(path: &[u8]) -> bool {
    ONLY_THE_REGION_HIDES.iter().any(|hidden| {
        path == *hidden || (path.starts_with(hidden) && path.get(hidden.len()) == Some(&b'/'))
    })
}

/// The paths staged in the private index against its `HEAD`.
fn privately_staged(s: &Scenario, dir: &Path) -> Vec<Vec<u8>> {
    let words = ["diff", "--cached", "--name-only", "-z", "--no-renames"];
    let staged = s.private(dir).git(words).succeeds();
    records(&staged.stdout, 0)
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

/// The id of every object the public repository holds.
fn public_objects(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git(EVERY_OBJECT).from(dir).succeeds().stdout
}

/// The public refs and what `HEAD` names.
fn public_refs(s: &Scenario, dir: &Path) -> Vec<u8> {
    let refs = s
        .git(["for-each-ref", "--format=%(refname) %(objectname)"])
        .from(dir)
        .succeeds();
    let head = s.git(["symbolic-ref", "HEAD"]).from(dir).succeeds();
    [refs.stdout, head.stdout].concat()
}

/// What a refusal of `add` must leave as it was: the private index, `.gitdupe`, and the
/// exclude file.
fn private_state(s: &Scenario, dir: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let index = s.private(dir).git(["ls-files", "-s", "-z"]).succeeds();
    (
        index.stdout,
        fs::read(dir.join(".gitdupe")).unwrap(),
        fs::read(dir.join(".git/info/exclude")).unwrap(),
    )
}

#[test]
fn a_project_made_private_then_used_daily_never_shows_public_git_a_private_file() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "A Developer"),
            ("user.email", "developer@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let dir = s.dir().join("project");
        s.unattached_project(&dir);
        fs::write(dir.join(".git/info/exclude"), BEFORE).unwrap();
        let objects = public_objects(s, &dir);
        let configuration = fs::read(dir.join(".git/config")).unwrap();
        let mut refs = public_refs(s, &dir);
        let unattached = public_git(&dir);

        // Bullet 1: attach, make three kinds of path private, commit.
        let init = dupe_succeeds(s, &dir, &["init"]);
        names(&init.stdout, dir.join(".git/dupe").as_os_str().as_bytes());
        let head = s.private(&dir).git(["symbolic-ref", "HEAD"]).succeeds();
        assert_eq!(head.stdout, b"refs/heads/main\n");
        let mut exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
        assert!(exclude.starts_with(BEFORE), "{}", exclude.escape_ascii());
        exclude.extend_from_slice(AFTER);
        fs::write(dir.join(".git/info/exclude"), exclude).unwrap();

        // Every private file holds content no public file has, so that an object found
        // in the public store can only have come from the private one.
        write(&dir, "notes/plan.md", b"plan: private\n");
        write(&dir, "notes/ideas.md", b"ideas: private\n");
        dupe_succeeds(s, &dir, &["add", "notes/"]);
        write(&dir, ".env.local", b"SECRET=1\n");
        write(&dir, ".vscode/settings.json", b"{\"private\": true}\n");
        dupe_succeeds(s, &dir, &["add", "-f", ".env.local", ".vscode/"]);
        write(&dir, "docs/notes.md", b"docs notes: private\n");
        dupe_succeeds(s, &dir, &["add", "docs/notes.md"]);
        dupe_succeeds(s, &dir, &["commit", "-m", "Local settings"]);

        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
        let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
        assert_eq!(
            exclude.escape_ascii().to_string(),
            exclude_with_the_five_rules().escape_ascii().to_string()
        );
        let commit = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds().stdout;
        let private_log = dupe_succeeds(s, &dir, &["log", "--format=%H"]);
        assert!(
            records(&private_log.stdout, b'\n').contains(&commit.trim_ascii_end()),
            "{private_log:?}"
        );
        let public_log = s.git(["log", "--all", "--format=%H"]).from(&dir).succeeds();
        assert!(
            !records(&public_log.stdout, b'\n').contains(&commit.trim_ascii_end()),
            "{public_log:?}"
        );
        assert_eq!(held_publicly(s, &dir, &commit), Vec::<Vec<u8>>::new());
        assert_eq!(public_objects(s, &dir), objects);
        // Nothing of the public repository moved but what git-dupe owns.
        let changed = unattached.changed_in(&public_git(&dir));
        assert!(changed.is_empty(), "changed: {changed:?}");

        write(&dir, "CHANGELOG.md", b"a public file\n");
        let status = public_status(s, &dir);
        let dry_run = s.git(["add", "-A", "--dry-run"]).from(&dir).succeeds();
        assert!(status.contains(&b"CHANGELOG.md".to_vec()), "{status:?}");
        assert!(holds(&dry_run.stdout, b"CHANGELOG.md"), "{dry_run:?}");
        for hidden in ONLY_THE_REGION_HIDES {
            assert!(!holds(&dry_run.stdout, hidden), "{dry_run:?}");
        }
        assert!(
            !status.iter().any(|path| hidden_by_the_region(path)),
            "{status:?}"
        );
        fs::remove_file(dir.join("CHANGELOG.md")).unwrap();

        let clone = s.dir().join("clone");
        s.git([
            "clone".as_ref(),
            "-q".as_ref(),
            dir.as_os_str(),
            clone.as_os_str(),
        ])
        .succeeds();
        for absent in [
            ".git/dupe",
            ".gitdupe",
            "notes",
            ".env.local",
            ".vscode",
            "docs/notes.md",
        ] {
            assert!(!clone.join(absent).exists(), "{absent} in the clone");
        }
        let cloned_exclude = fs::read(clone.join(".git/info/exclude")).unwrap_or_default();
        assert!(!holds(&cloned_exclude, b"# BEGIN git-dupe"));

        // Bullet 2, on what bullet 1 left: the day's new files.
        write(&dir, "notes/today.md", b"today: private\n");
        write(&dir, ".vscode/launch.json", b"{\"launch\": 1}\n");
        write(&dir, "docs/api.md", b"api\n");
        write(&dir, "src/scratch.py", b"print('scratch')\n");
        let private_status = dupe_succeeds(s, &dir, &["status", "--porcelain"]);
        let listed = entries(&private_status.stdout);
        assert!(
            listed.contains(&b"?? notes/today.md".to_vec()),
            "{private_status:?}"
        );
        assert!(
            listed.contains(&b"!! .vscode/launch.json".to_vec()),
            "{private_status:?}"
        );
        for public in [&b"docs/api.md"[..], b"src/scratch.py", b"src/", b"docs/"] {
            assert!(
                !paths_of(&private_status.stdout).contains(&public.to_vec()),
                "{private_status:?}"
            );
        }
        let status = public_status(s, &dir);
        for public in [&b"docs/api.md"[..], b"src/scratch.py"] {
            assert!(status.contains(&public.to_vec()), "{status:?}");
        }
        assert!(
            !status.iter().any(|path| hidden_by_the_region(path)),
            "{status:?}"
        );
        assert!(
            !status.contains(&b".vscode/launch.json".to_vec()),
            "{status:?}"
        );

        write(&dir, ".env.local", b"SECRET=2\n");
        let added = dupe(s, &dir, &dir.join("notes"), &["add", "."]);
        assert_eq!(added.end, End::Code(0), "{added:?}");
        assert_eq!(privately_staged(s, &dir), [b"notes/today.md"]);
        dupe_succeeds(s, &dir, &["add", "."]);
        assert_eq!(
            privately_staged(s, &dir),
            [&b".env.local"[..], b"notes/today.md"]
        );

        dupe_succeeds(s, &dir, &["add", "src/scratch.py"]);
        assert!(privately_tracked(s, &dir).contains(&b"src/scratch.py".to_vec()));
        assert!(!public_status(s, &dir).contains(&b"src/scratch.py".to_vec()));
        let released = dupe_succeeds(s, &dir, &["rm", "--cached", "src/scratch.py"]);
        warnings_in_any_order(&released, &[&[b"src/scratch.py"]]);
        assert!(!privately_tracked(s, &dir).contains(&b"src/scratch.py".to_vec()));
        assert!(public_status(s, &dir).contains(&b"src/scratch.py".to_vec()));

        for operand in ["README.md", "docs/"] {
            let before = private_state(s, &dir);
            let refused = dupe(s, &dir, &dir, &["add", operand]);
            assert_eq!(refused.end, End::Code(128), "{operand}: {refused:?}");
            assert_eq!(refused.lines("fatal").len(), 1, "{operand}: {refused:?}");
            assert_eq!(private_state(s, &dir), before, "{operand}");
        }

        // A teammate's public `notes/shared.md` arrives under the hidden `notes`.
        write(&clone, "notes/shared.md", b"shared by a teammate\n");
        s.git(["add", "notes/shared.md"]).from(&clone).succeeds();
        s.commit_public(&clone);
        s.git([
            "pull".as_ref(),
            "-q".as_ref(),
            "--ff-only".as_ref(),
            clone.as_os_str(),
            "main".as_ref(),
        ])
        .from(&dir)
        .succeeds();
        let pulled = public_refs(s, &dir);
        assert_ne!(pulled, refs);
        refs = pulled;
        write(&dir, "notes/plan.md", b"plan: changed\n");
        let added = dupe_succeeds(s, &dir, &["add", "notes/"]);
        let warnings = added.lines("warning");
        assert_eq!(warnings.len(), 1, "{added:?}");
        names_number(warnings[0], 1);
        assert!(privately_staged(s, &dir).contains(&b"notes/plan.md".to_vec()));
        assert!(!privately_tracked(s, &dir).contains(&b"notes/shared.md".to_vec()));
        let private_status = dupe_succeeds(s, &dir, &["status", "--porcelain"]);
        assert!(
            !paths_of(&private_status.stdout).contains(&b"notes/shared.md".to_vec()),
            "{private_status:?}"
        );

        // A `!` rule re-including `.env.local`, on the fourth line of `.gitignore`.
        let quiet = dupe_succeeds(s, &dir, &["status"]);
        assert!(quiet.lines("warning").is_empty(), "{quiet:?}");
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!.env.local\n",
        );
        let warned = dupe(s, &dir, &dir, &["status"]);
        assert_eq!(warned.end, quiet.end, "{warned:?}");
        warnings_in_any_order(&warned, &[&[b".env.local", b".gitignore"]]);
        names_number(warned.lines("warning")[0], 4);

        // When the story ends: nothing private entered the public repository.
        let private_objects = s.private(&dir).git(EVERY_OBJECT).succeeds().stdout;
        assert_eq!(
            held_publicly(s, &dir, &private_objects),
            Vec::<Vec<u8>>::new()
        );
        let private_index = privately_tracked(s, &dir);
        let public_index = publicly_tracked(s, &dir);
        assert!(public_index.contains(&b"notes/shared.md".to_vec()));
        assert!(
            !public_index.iter().any(|path| private_index.contains(path)),
            "public: {public_index:?}; private: {private_index:?}"
        );
        assert_eq!(fs::read(dir.join(".git/config")).unwrap(), configuration);
        assert_eq!(public_refs(s, &dir), refs);
        assert_eq!(
            fs::read(dir.join(".git/info/exclude"))
                .unwrap()
                .escape_ascii()
                .to_string(),
            exclude_with_the_five_rules().escape_ascii().to_string()
        );
    });
}

#[test]
fn clean_spares_the_private_files_and_restore_brings_back_what_plain_clean_deleted() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "A Developer"),
            ("user.email", "developer@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let dir = s.dir().join("project");
        s.attached_project(&dir);

        // The bullet's workspace: a private `.env.local` whose staged content differs from
        // its last commit, saved; a hidden `notes/` holding the privately tracked
        // `notes/today.md` and the untracked `notes/scratch.md`; the privately tracked
        // `build/local.cfg` inside the ignored `build/` beside `build/out.js`; and the
        // untracked `scratch.txt`.
        write(&dir, "notes/today.md", b"today: private\n");
        dupe_succeeds(s, &dir, &["add", "notes/"]);
        write(&dir, ".env.local", b"SECRET=committed\n");
        write(&dir, "build/local.cfg", b"local: private\n");
        dupe_succeeds(s, &dir, &["add", "-f", ".env.local", "build/local.cfg"]);
        dupe_succeeds(s, &dir, &["commit", "-m", "Private files"]);
        write(&dir, ".env.local", b"SECRET=staged\n");
        dupe_succeeds(s, &dir, &["add", ".env.local"]);
        let saved = fs::read(dir.join(".env.local")).unwrap();
        let staged = s.private(&dir).git(["cat-file", "blob", ":.env.local"]);
        assert_eq!(staged.succeeds().stdout, saved);
        let committed = s.private(&dir).git(["cat-file", "blob", "HEAD:.env.local"]);
        assert_ne!(committed.succeeds().stdout, saved);
        write(&dir, "notes/scratch.md", b"scratch: never staged\n");
        write(&dir, "build/out.js", b"built\n");
        write(&dir, "scratch.txt", b"scratch\n");

        let before = Tree::working(&dir);
        dupe_succeeds(s, &dir, &["clean", "-fdx"]);
        assert_eq!(
            changed_since(&before, &dir),
            ["build/out.js", "scratch.txt"]
        );

        // The same files recreated: `scratch.txt` is not ignored, and `-X` keeps it.
        write(&dir, "build/out.js", b"built\n");
        write(&dir, "scratch.txt", b"scratch\n");
        let before = Tree::working(&dir);
        dupe_succeeds(s, &dir, &["clean", "-fdX"]);
        assert_eq!(changed_since(&before, &dir), ["build/out.js"]);

        // Plain `git clean -fdx`, typed from habit, is not guarded.
        s.git(["clean", "-fdx"]).from(&dir).succeeds();
        for gone in [".env.local", "notes", "build"] {
            assert!(
                fs::symlink_metadata(dir.join(gone)).is_err(),
                "{gone} survived plain git clean -fdx"
            );
        }
        let status = dupe_succeeds(s, &dir, &["status", "--porcelain"]);
        let listed = entries(&status.stdout);
        for path in [&b".env.local"[..], b"notes/today.md", b"build/local.cfg"] {
            assert!(
                listed
                    .iter()
                    .any(|entry| entry[1] == b'D' && entry[3..] == *path),
                "{} not shown deleted: {status:?}",
                path.escape_ascii()
            );
        }

        dupe_succeeds(s, &dir, &["restore", "."]);
        assert_eq!(fs::read(dir.join(".env.local")).unwrap(), saved);
        assert_eq!(
            fs::read(dir.join("notes/today.md")).unwrap(),
            b"today: private\n"
        );
        assert_eq!(
            fs::read(dir.join("build/local.cfg")).unwrap(),
            b"local: private\n"
        );
        // Never staged, so gone for good.
        assert!(!dir.join("notes/scratch.md").exists());
        assert_eq!(
            region_rules(&dir),
            [
                &b"/.env.local"[..],
                b"/.gitdupe",
                b"/build/local.cfg",
                b"/notes"
            ]
        );
    });
}
