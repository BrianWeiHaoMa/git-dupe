//! The forms by which a word names a public place (G18, `Holds/G18`, S13): every
//! spelling of the project's remote URL, every path of its working trees and Git
//! directories, and the neighbors that name none, asked through `git dupe remote`.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::harness::{
    Everything, MANY_URLS, PROJECT_URL, Scenario, Transfer, Tree, refusal, text, unchanged,
    under_each_release,
};

fn refused(s: &Scenario, t: &Transfer, from: &Path, word: &str, place: &str) {
    let before: Everything = t.everything();
    let output = s
        .git(["dupe", "remote", "add", "leak", word])
        .from(from)
        .run();
    refusal(&output, &[b"git dupe git", place.as_bytes()]);
    before.unchanged();
}

fn configured(s: &Scenario, t: &Transfer, name: &str, word: &str) {
    let key = format!("remote.{name}.url");
    let output = s.private(&t.root).git(["config", "--get", &key]).succeeds();
    assert_eq!(output.stdout, format!("{word}\n").as_bytes(), "{output:?}");
}

fn runs(s: &Scenario, t: &Transfer, from: &Path, name: &str, word: &str) {
    s.git(["dupe", "remote", "add", name, word])
        .from(from)
        .succeeds();
    configured(s, t, name, word);
}

/// Schemes, host case, user, port, suffix and encoded URL bytes name the same remote.
#[test]
fn url_forms_and_equal_values_name_origin() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        for word in [
            PROJECT_URL,
            "http://example.com/team/project.git",
            "git@example.com:team/project.git",
            "git@example.com:/team/project.git",
            "ssh://someone@EXAMPLE.com:2222/team/project",
            "https://example.com/team/project.git/",
            "https://example.com/team/pro%6Aect.git",
            "1x://example.com/team/project.git",
        ] {
            for spelling in [
                word.to_owned(),
                format!("--mirror={word}"),
                format!("x={word}"),
            ] {
                refused(s, &t, &t.root, &spelling, "origin");
            }
        }
    });
}

/// Path case, another host and a longer path remain distinct from the public URL.
#[test]
fn url_neighbors_keep_their_spelling() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        refused(
            s,
            &t,
            &t.root,
            "https://example.com/team/project.git",
            "origin",
        );
        for (index, word) in [
            "https://example.com/team/Project.git",
            "https://example.org/team/project.git",
            "https://example.com/team/project-private.git",
        ]
        .into_iter()
        .enumerate()
        {
            runs(s, &t, &t.root, &format!("near{index}"), word);
        }
    });
}

/// Percent escapes are decoded after a scheme delimiter and left literal without one.
#[test]
fn percent_escapes_in_scp_words_stay_literal() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        refused(
            s,
            &t,
            &t.root,
            "https://example.com/team/pro%6Aect.git",
            "origin",
        );
        runs(s, &t, &t.root, "near", "git@example.com:team/pro%6Aect.git");
    });
}

/// A URL's authority ends at its first `/` before anything is decoded, so an encoded `/`
/// in a credential stays in the credential: such a URL names the public place its host
/// and path name, and a public URL so spelled is named by every other spelling of its
/// host and path, the configured URL of a private remote included.
#[test]
fn an_encoded_slash_in_a_credential_hides_no_public_place() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let hosted = s.dir().join("hosted/public.git");
        s.bare_repository(&hosted);
        let path = text(&hosted);
        let origin = |url: &str| {
            s.git(["remote", "set-url", "origin", url])
                .from(&t.root)
                .succeeds();
        };
        for credential in ["pass", "p%2Fss", "p%2fss"] {
            let credentialed = format!("https://reader:{credential}@localhost{path}");
            origin(&format!("https://localhost{path}"));
            refused(s, &t, &t.root, &credentialed, "remote 'origin'");
            origin(&credentialed);
            for word in [
                format!("ssh://git@localhost{path}"),
                format!("file://localhost{path}"),
            ] {
                refused(s, &t, &t.root, &word, "remote 'origin'");
            }
        }
        origin(&format!("https://reader:p%2Fss@localhost{path}"));
        s.git([
            "dupe",
            "git",
            "remote",
            "add",
            "origin",
            &format!("file://localhost{path}"),
        ])
        .from(&t.root)
        .succeeds();
        let before = t.everything();
        let hosted_before = Tree::of(&hosted);
        let output = s
            .git(["dupe", "push", "-u", "origin", "main"])
            .from(&t.root)
            .run();
        refusal(&output, &[b"remote 'origin'", b"git dupe remote remove"]);
        before.unchanged();
        unchanged(&hosted_before, &hosted);
    });
}

/// Git decodes a `file://` URL whole before it looks for the path and opens the path from
/// the first `/` it then finds, whatever the host: a word whose authority holds an
/// encoded `/` names the root when that path is the root, and also when its own path
/// after the host is.
#[test]
fn a_file_url_names_its_own_path_and_the_path_git_opens_once_it_decodes_the_url_whole() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let root = text(&t.root);
        let rest = root.strip_prefix('/').unwrap();
        let place = format!("working tree '{root}'");
        for word in [
            format!("file://x%2F{rest}"),
            format!("file://reader:p%2f{rest}"),
            format!("file://reader:p%2Fss@localhost{root}"),
        ] {
            refused(s, &t, &t.root, &word, &place);
        }
    });
}

/// Git decodes every other `%XX` of a URL and keeps `%00` as its three bytes, so a
/// `file://` URL whose path holds them opens the repository at that path: the project's
/// remote there, configured as a path or as such a URL, is the place either names, and a
/// push names it, sending nothing.
#[test]
fn a_file_url_keeps_a_percent_escaped_zero_byte_as_git_opens_it() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let public = s.dir().join("public%00.git");
        s.bare_repository(&public);
        let path = text(&public).to_owned();
        let url = format!("file://{path}");
        for (configured, word) in [(&path, &url), (&url, &path)] {
            s.git(["remote", "add", "percent", configured])
                .from(&t.root)
                .succeeds();
            refused(s, &t, &t.root, word, "percent");
            s.git(["remote", "remove", "percent"])
                .from(&t.root)
                .succeeds();
        }
        s.git(["remote", "add", "percent", &path])
            .from(&t.root)
            .succeeds();
        let pushed = s
            .git(["dupe", "push", &url, "HEAD:refs/heads/leak"])
            .from(&t.root)
            .run();
        refusal(&pushed, &[b"git dupe git", b"percent"]);
        let refs = s.git(["for-each-ref"]).from(&public).succeeds();
        assert!(refs.stdout.is_empty(), "{refs:?}");
    });
}

/// Both URL records and the push URL are places of their public remote.
#[test]
fn every_url_of_many_is_public() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        for word in MANY_URLS {
            refused(s, &t, &t.root, word, "many");
        }
    });
}

/// Worktree roots and their Git entries are exact places, with distinct neighbors.
#[test]
fn working_trees_and_git_directories_are_public() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let common = t.root.join(".git");
        let linked_git = common.join("worktrees").join(t.linked.file_name().unwrap());
        for path in [
            &t.root,
            &common,
            &t.linked,
            &linked_git,
            &t.linked.join(".git"),
        ] {
            refused(s, &t, &t.root, text(path), text(path));
        }
        runs(
            s,
            &t,
            &t.root,
            "root-neighbor",
            &format!("{}-other", t.root.display()),
        );
        runs(
            s,
            &t,
            &t.root,
            "linked-neighbor",
            &format!("{}-x", t.linked.display()),
        );
    });
}

/// A local remote may omit .git after trailing slashes are dropped, but not add a prefix.
#[test]
fn local_remote_suffix_and_slashes_name_local() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let public = text(&t.public_remote);
        let without_git = public.strip_suffix(".git").unwrap();
        let neighbor = format!("{without_git}-private.git");
        for word in [
            public.to_owned(),
            without_git.to_owned(),
            format!("{without_git}/"),
            format!("{without_git}//"),
        ] {
            refused(s, &t, &t.root, &word, "local");
        }
        runs(s, &t, &t.root, "near", &neighbor);
    });
}

/// Existing paths follow symlinks before .., while absent paths use lexical cleaning.
#[test]
fn real_paths_precede_lexical_cleaning() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let sub = t.root.join("sub");
        fs::create_dir(&sub).unwrap();
        let alias = s.dir().join("root-link");
        symlink(&t.root, &alias).unwrap();
        let elsewhere = s.dir().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        symlink(&sub, elsewhere.join("link")).unwrap();
        for path in [alias, elsewhere.join("link/.."), t.root.join("absent/..")] {
            refused(s, &t, &t.root, text(&path), text(&t.root));
        }
    });
}

/// File URLs name their local paths regardless of host, including linked Git directories.
#[test]
fn file_urls_also_name_local_places() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let common = t.root.join(".git");
        let linked_git = common.join("worktrees").join(t.linked.file_name().unwrap());
        for (host, path) in [
            ("", &t.root),
            ("localhost", &common),
            ("", &t.linked),
            ("", &linked_git),
        ] {
            refused(
                s,
                &t,
                &t.root,
                &format!("file://{host}{}", path.display()),
                text(path),
            );
        }
        let relative = format!("../{}", t.root.file_name().unwrap().to_str().unwrap());
        refused(s, &t, &t.root, &relative, text(&t.root));
    });
}

/// Git opens a local path only for a URL that begins `file://` as written; any other
/// spelling of the scheme goes to a remote helper of that name, so it names no local
/// place and runs as Git's own.
#[test]
fn a_file_scheme_spelled_otherwise_names_no_local_place() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        for (name, scheme) in [("upper", "FILE"), ("mixed", "File")] {
            runs(
                s,
                &t,
                &t.root,
                name,
                &format!("{scheme}://{}", t.root.display()),
            );
        }
    });
}

/// A word holding a newline is refused on one line, named in Git's quoting.
#[test]
fn a_word_holding_a_newline_is_named_on_one_line() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let word = "https://first\nsecond@example.com/team/project.git";
        refused(s, &t, &t.root, word, "origin");
        refused(s, &t, &t.root, word, r#""https://first\nsecond@"#);
    });
}

/// Inside the workspace, relative destinations use the root rather than the user's subdirectory.
#[test]
fn relative_words_inside_use_the_root() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let sub = t.root.join("sub");
        fs::create_dir(&sub).unwrap();
        refused(s, &t, &sub, ".", text(&t.root));
        runs(s, &t, &sub, "near", "..");
    });
}

/// An explicit worktree adds the outside user's directory only while the user is outside.
#[test]
fn relative_words_outside_also_use_the_users_directory() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let word = t.root.file_name().unwrap().to_str().unwrap();
        let before: Everything = t.everything();
        let output = s
            .git(["dupe", "remote", "add", "leak", word])
            .from(s.dir())
            .variable("GIT_DIR", t.root.join(".git"))
            .variable("GIT_WORK_TREE", &t.root)
            .run();
        refusal(&output, &[b"git dupe git", text(&t.root).as_bytes()]);
        before.unchanged();
        s.git(["dupe", "remote", "add", "leak", word])
            .from(&t.root)
            .variable("GIT_DIR", t.root.join(".git"))
            .variable("GIT_WORK_TREE", &t.root)
            .succeeds();
        configured(s, &t, "leak", word);
    });
}
