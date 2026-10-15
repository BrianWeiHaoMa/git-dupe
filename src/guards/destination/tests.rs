use super::*;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

const ROOT: &str = "/nonexistent-git-dupe/w/repo";

fn from_root() -> ReadFrom<'static> {
    ReadFrom {
        root: Path::new(ROOT),
        outside: None,
        home: None,
    }
}

fn url_form(host: &str, path: &str) -> Form {
    Form::Url {
        host: host.into(),
        path: path.into(),
    }
}

fn local_forms(path: &str) -> Vec<Form> {
    [
        path.to_owned(),
        format!("{path}.git"),
        format!("{path}/.git"),
        format!("{path}.git/.git"),
    ]
    .map(|path| Form::Local(path.into_bytes()))
    .to_vec()
}

fn of(word: &str) -> Vec<Form> {
    forms(word.as_bytes(), from_root())
}

#[test]
fn a_url_with_a_scheme_is_its_lowered_host_and_its_path_without_git_and_slashes() {
    let project = url_form("example.com", "/team/project");
    for word in [
        "https://Example.com/team/project.git",
        "http://example.com/team/project.git",
        "ssh://someone@EXAMPLE.com:2222/team/project",
        "https://example.com/team/project.git/",
        "https://example.com/team/project//",
        "git+ssh://user:secret@example.com/team/project.git",
    ] {
        assert_eq!(of(word), std::slice::from_ref(&project), "{word}");
    }
    // The path keeps its case; another host or path is another form.
    assert_eq!(
        of("https://example.com/team/Project.git"),
        [url_form("example.com", "/team/Project")]
    );
    assert_eq!(
        of("https://example.com/team/project-private.git"),
        [url_form("example.com", "/team/project-private")]
    );
    assert_eq!(of("https://example.com"), [url_form("example.com", "")]);
    assert_eq!(of("ssh://[::1]:22/p.git"), [url_form("::1", "/p")]);
    // A scheme may begin with a digit, as Git's `is_url` reads one: Git hands such a
    // URL to its remote helper, never opens it as a path.
    assert_eq!(of("1x://Example.com/team/project.git"), [project]);
}

#[test]
fn each_percent_escape_after_a_scheme_is_decoded_in_the_authority_and_the_path() {
    assert_eq!(
        of("https://example.com/team/pro%6Aect.git"),
        [url_form("example.com", "/team/project")]
    );
    assert_eq!(
        of("ssh://ex%41mple.com/a%2fb%zz%4"),
        [url_form("example.com", "/a/b%zz%4")]
    );
}

#[test]
fn the_authority_ends_at_the_first_slash_before_anything_is_decoded() {
    let public = url_form("localhost", "/x/public");
    // An encoded `/`, in either case, or `?` or `#` in a credential is credential bytes:
    // the host is still the one after the last `@`, the path the one after the host.
    for word in [
        "https://reader:pass@localhost/x/public.git",
        "https://reader:p%2Fss@localhost/x/public.git",
        "https://reader:p%2fss@localhost/x/public.git",
        "ssh://reader:p%2Fss@localhost/x/public.git",
        "https://reader%2F:pass@localhost/x/public.git",
        "https://reader:p%3Fs%23s@localhost/x/public.git",
        "https://reader:p%2Fs%2Fs@LocalHost:8443/x/public.git/",
    ] {
        assert_eq!(of(word), std::slice::from_ref(&public), "{word}");
    }
    // An encoded `@` in a credential is decoded before the last `@` is found.
    assert_eq!(
        of("https://reader:p%40ss@localhost/x"),
        [url_form("localhost", "/x")]
    );
    // With no `/` after the authority, the path is empty.
    assert_eq!(
        of("https://reader:p%2Fss@localhost"),
        [url_form("localhost", "")]
    );
}

#[test]
fn a_file_url_also_has_the_local_forms_of_the_path_git_opens_once_it_decodes_the_url_whole() {
    // Git decodes a `file://` URL before it looks for the path and opens the path from
    // the first `/` it then finds, whatever the host: `/ss@localhost/srv/p`, where the
    // URL's own host and path are `localhost` and `/srv/p`.
    let mut expected = vec![url_form("localhost", "/srv/p")];
    expected.extend(local_forms("/srv/p"));
    expected.extend(local_forms("/ss@localhost/srv/p"));
    assert_eq!(of("file://reader:p%2Fss@localhost/srv/p"), expected);
    let mut expected = vec![url_form("x/srv", "/p")];
    expected.extend(local_forms("/p"));
    expected.extend(local_forms("/srv/p"));
    assert_eq!(of("file://x%2fsrv/p"), expected);
    // With no path of its own, the URL has the local forms of the one Git opens alone.
    let mut expected = vec![url_form("x/srv", "")];
    expected.extend(local_forms("/srv"));
    assert_eq!(of("file://x%2Fsrv"), expected);
}

#[test]
fn a_word_with_a_colon_before_any_slash_and_no_scheme_is_host_and_path() {
    let project = url_form("example.com", "/team/project");
    for word in [
        "git@example.com:team/project.git",
        "git@example.com:/team/project.git",
        "Example.COM:team/project/",
    ] {
        assert_eq!(of(word), std::slice::from_ref(&project), "{word}");
    }
    // Nothing is decoded without a scheme, and a refspec is this form too.
    assert_eq!(
        of("git@example.com:team/pro%6Aect.git"),
        [url_form("example.com", "/team/pro%6Aect")]
    );
    assert_eq!(of("main:leak"), [url_form("main", "/leak")]);
    assert_eq!(of("user@[::1]:repo.git"), [url_form("::1", "/repo")]);
    assert_eq!(of("[fe80::1]:/r"), [url_form("fe80::1", "/r")]);
}

#[test]
fn a_file_url_has_its_url_form_and_the_local_forms_of_the_path_after_its_host() {
    let mut expected = vec![url_form("localhost", "/srv/p")];
    expected.extend(local_forms("/srv/p.git"));
    assert_eq!(of("file://localhost/srv/p.git"), expected);
    let mut expected = vec![url_form("", "/srv/a b")];
    expected.extend(local_forms("/srv/a b"));
    assert_eq!(of("file:///srv/a%20b/"), expected);
    // Any other spelling of the scheme is a URL Git hands to a remote helper: no local
    // form.
    assert_eq!(of("FILE:///srv/p"), [url_form("", "/srv/p")]);
    assert_eq!(
        of("File://localhost/srv/p"),
        [url_form("localhost", "/srv/p")]
    );
}

#[test]
fn anything_else_is_a_local_path_read_from_the_root_and_the_four_paths_git_opens() {
    assert_eq!(of("/srv/p.git"), local_forms("/srv/p.git"));
    // Trailing slashes are dropped before `.git` is appended.
    assert_eq!(of("/srv/p/"), local_forms("/srv/p"));
    assert_eq!(of("/srv/p//"), local_forms("/srv/p"));
    // Relative to the root, whatever the user's directory below it; `..` cleaned as
    // text where nothing exists. `.git` is appended to the path as written, as Git
    // appends it.
    let dot: Vec<Form> = [
        ROOT.to_owned(),
        format!("{ROOT}/..git"),
        format!("{ROOT}/.git"),
        format!("{ROOT}/..git/.git"),
    ]
    .map(|path| Form::Local(path.into_bytes()))
    .to_vec();
    assert_eq!(of("."), dot);
    assert_eq!(of("../repo"), local_forms(ROOT));
    assert_eq!(of("sub/../x"), local_forms(&format!("{ROOT}/x")));
    assert_eq!(of("-v"), local_forms(&format!("{ROOT}/-v")));
    // Not a scheme Git reads, though it holds `://`: a local path.
    assert_eq!(of("./a://b"), local_forms(&format!("{ROOT}/a:/b")));
    // The empty word, part, or URL is the root.
    assert_eq!(of(""), local_forms(ROOT));
}

#[test]
fn a_relative_path_is_also_read_from_the_users_directory_outside_the_root() {
    let outside = Path::new("/nonexistent-git-dupe/w");
    let from = ReadFrom {
        root: Path::new(ROOT),
        outside: Some(outside),
        home: None,
    };
    let mut expected = local_forms(&format!("{ROOT}/repo"));
    expected.extend(local_forms(ROOT));
    assert_eq!(forms(b"repo", from), expected);
    assert_eq!(forms(b"/srv/x", from), local_forms("/srv/x"));
}

#[test]
fn a_path_that_is_tilde_or_begins_with_tilde_slash_is_read_from_home() {
    const HOME: &str = "/nonexistent-git-dupe/h";
    let outside = Path::new("/nonexistent-git-dupe/w");
    let with = |home: Option<&'static str>, word: &str| {
        let from = ReadFrom {
            root: Path::new(ROOT),
            outside: Some(outside),
            home: home.map(str::as_bytes),
        };
        forms(word.as_bytes(), from)
    };
    // `HOME` in place of the `~`, after the trailing slashes are dropped; never joined
    // to the root or the user's directory when `HOME` is absolute.
    for word in ["~", "~/", "~//"] {
        assert_eq!(with(Some(HOME), word), local_forms(HOME), "{word}");
    }
    for word in ["~/proj", "~/proj/", "~//proj"] {
        assert_eq!(
            with(Some(HOME), word),
            local_forms(&format!("{HOME}/proj")),
            "{word}"
        );
    }
    assert_eq!(
        with(Some(HOME), "~/../w/repo"),
        local_forms(ROOT),
        "cleaned as text where nothing exists"
    );
    // `.git` is appended to what `HOME` gives, its own trailing slash kept, as Git
    // appends it.
    let git = format!("{HOME}/.git");
    let slashed: Vec<Form> = [HOME, &git, &git, &format!("{git}/.git")]
        .map(|path| Form::Local(path.as_bytes().to_vec()))
        .to_vec();
    assert_eq!(with(Some("/nonexistent-git-dupe/h/"), "~"), slashed);
    // A relative `HOME` gives a relative path, read where any relative one is.
    let mut expected = local_forms(&format!("{ROOT}/h/proj"));
    expected.extend(local_forms("/nonexistent-git-dupe/w/h/proj"));
    assert_eq!(with(Some("h"), "~/proj"), expected);
    // Where `HOME` is unset Git opens nothing for it.
    assert_eq!(with(None, "~/proj"), []);
    assert_eq!(with(None, "~"), []);
    // `~<user>`, and a `~` anywhere but first, are read as written.
    let mut expected = local_forms(&format!("{ROOT}/~someone/proj"));
    expected.extend(local_forms("/nonexistent-git-dupe/w/~someone/proj"));
    assert_eq!(with(Some(HOME), "~someone/proj"), expected);
    assert_eq!(with(None, "~someone/proj"), expected);
    let mut expected = local_forms(&format!("{ROOT}/~"));
    expected.extend(local_forms("/nonexistent-git-dupe/w/~"));
    assert_eq!(with(Some(HOME), "./~"), expected);
    // A URL's path is never read from `HOME`.
    assert_eq!(
        with(Some(HOME), "ssh://example.com/~/p.git"),
        [url_form("example.com", "/~/p")]
    );
    assert_eq!(with(Some(HOME), "file://~/p")[1..], local_forms("/p")[..]);
}

/// A directory of the test's own below the system's temporary one, removed after.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_path_that_exists_is_its_real_path_asked_as_written() {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("git-dupe-destination-{}", std::process::id())));
    let _ = fs::remove_dir_all(&scratch.0);
    let real = fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(scratch.0.file_name().unwrap());
    fs::create_dir_all(real.join("repo/sub")).unwrap();
    fs::create_dir_all(real.join("elsewhere")).unwrap();
    symlink(real.join("repo"), real.join("to-repo")).unwrap();
    symlink(real.join("repo/sub"), real.join("elsewhere/link")).unwrap();
    symlink(real.join("loop"), real.join("loop")).unwrap();
    let bytes = |path: &Path| path.as_os_str().as_bytes().to_vec();
    let repo = bytes(&real.join("repo"));

    // Through a link to the root, and `link/..` beyond a link, where `..` is taken
    // after the link: the root, not `elsewhere`.
    assert_eq!(canonical(&bytes(&real.join("to-repo"))), repo);
    assert_eq!(canonical(&bytes(&real.join("elsewhere/link/.."))), repo);
    // Through a directory that does not exist: cleaned as text.
    assert_eq!(canonical(&bytes(&real.join("repo/absent/.."))), repo);
    // A loop of links, which Git cannot open either: cleaned as text.
    assert_eq!(
        canonical(&bytes(&real.join("loop/x/.."))),
        bytes(&real.join("loop"))
    );
    let from = ReadFrom {
        root: &real.join("repo"),
        outside: None,
        home: None,
    };
    assert!(forms(b"../to-repo", from).contains(&Form::Local(repo.clone())));
    assert!(!forms(b"../repo-other", from).contains(&Form::Local(repo)));
}
