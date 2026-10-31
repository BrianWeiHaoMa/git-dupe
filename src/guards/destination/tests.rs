use super::*;
use crate::links_and_modes::{at_the_top, symlink};
use std::path::PathBuf;

/// `path`, which begins with `/`, as a place this platform calls absolute.
fn top(path: &str) -> String {
    String::from_utf8(at_the_top(path)).unwrap()
}

fn root() -> String {
    top("/nonexistent-git-dupe/w/repo")
}

/// What a word or a URL's path that begins with `/` is read as: itself where the platform
/// calls it absolute, and on Windows, where a root and no drive is not, on the root's drive.
fn read(path: &str) -> String {
    top(path)
}

fn from(root: &Path) -> ReadFrom<'_> {
    ReadFrom {
        root,
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

/// `forms` without the URL form a path on a drive has beside its local ones.
fn locals(forms: Vec<Form>) -> Vec<Form> {
    forms
        .into_iter()
        .filter(|form| matches!(form, Form::Local(_)))
        .collect()
}

fn of(word: &str) -> Vec<Form> {
    forms(word.as_bytes(), from(Path::new(&root())))
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
fn a_percent_escape_of_the_zero_byte_stays_as_written() {
    // Git decodes every other `%XX` of a URL and keeps `%00` as its three bytes, so the
    // path it opens for a `file://` URL holds them, as the other forms do.
    assert_eq!(
        of("ssh://ex%00ample.com/a%00b.git"),
        [url_form("ex%00ample.com", "/a%00b")]
    );
    let mut expected = vec![url_form("", "/srv/p%00q")];
    expected.extend(local_forms(&read("/srv/p%00q.git")));
    assert_eq!(of("file:///srv/p%00q.git"), expected);
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
    expected.extend(local_forms(&read("/srv/p")));
    expected.extend(local_forms(&read("/ss@localhost/srv/p")));
    assert_eq!(of("file://reader:p%2Fss@localhost/srv/p"), expected);
    let mut expected = vec![url_form("x/srv", "/proj")];
    expected.extend(local_forms(&read("/proj")));
    expected.extend(local_forms(&read("/srv/proj")));
    assert_eq!(of("file://x%2fsrv/proj"), expected);
    // With no path of its own, the URL has the local forms of the one Git opens alone.
    let mut expected = vec![url_form("x/srv", "")];
    expected.extend(local_forms(&read("/srv")));
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
    expected.extend(local_forms(&read("/srv/p.git")));
    assert_eq!(of("file://localhost/srv/p.git"), expected);
    let mut expected = vec![url_form("", "/srv/a b")];
    expected.extend(local_forms(&read("/srv/a b")));
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
    let root = root();
    let absolute = top("/srv/p");
    assert_eq!(
        locals(of(&format!("{absolute}.git"))),
        local_forms(&format!("{absolute}.git"))
    );
    // Trailing slashes are dropped before `.git` is appended.
    assert_eq!(locals(of(&format!("{absolute}/"))), local_forms(&absolute));
    assert_eq!(locals(of(&format!("{absolute}//"))), local_forms(&absolute));
    // Relative to the root, whatever the user's directory below it; `..` cleaned as
    // text where nothing exists. `.git` is appended to the path as written, as Git
    // appends it.
    let dot: Vec<Form> = [
        root.clone(),
        format!("{root}/..git"),
        format!("{root}/.git"),
        format!("{root}/..git/.git"),
    ]
    .map(|path| Form::Local(path.into_bytes()))
    .to_vec();
    assert_eq!(of("."), dot);
    assert_eq!(of("../repo"), local_forms(&root));
    assert_eq!(of("sub/../x"), local_forms(&format!("{root}/x")));
    assert_eq!(of("-v"), local_forms(&format!("{root}/-v")));
    // Not a scheme Git reads, though it holds `://`: a local path.
    assert_eq!(of("./a://b"), local_forms(&format!("{root}/a:/b")));
    // The empty word, part, or URL is the root.
    assert_eq!(of(""), local_forms(&root));
}

#[test]
fn a_path_is_absolute_as_the_platform_says_and_only_then_stands_alone() {
    let root = root();
    let outside = top("/nonexistent-git-dupe/w");
    let from = ReadFrom {
        root: Path::new(&root),
        outside: Some(Path::new(&outside)),
        home: None,
    };
    let word = "C:\\proj";
    let expected = if Path::new(word).is_absolute() {
        vec![word.as_bytes().to_vec()]
    } else {
        vec![
            format!("{root}/{word}").into_bytes(),
            format!("{outside}/{word}").into_bytes(),
        ]
    };
    assert_eq!(joined(word.as_bytes(), from), expected, "{word}");
    // A root and no drive stands on the drive it is read from; on Linux and macOS it is
    // absolute and stands alone.
    let expected = if Path::new("/srv").is_absolute() {
        vec![b"/srv".to_vec()]
    } else {
        vec![top("/srv").into_bytes(), top("/srv").into_bytes()]
    };
    assert_eq!(joined(b"/srv", from), expected);
    // The top keeps what makes it absolute when trailing slashes are dropped.
    let top_of_all = top("/");
    let expected = Form::Local(canonical(top_of_all.as_bytes()));
    assert_eq!(locals(forms(top_of_all.as_bytes(), from))[0], expected);
    let separator = if Path::new("/").is_absolute() {
        "/"
    } else {
        "\\"
    };
    let word = format!("{top_of_all}{separator}");
    assert_eq!(locals(forms(word.as_bytes(), from))[0], expected);
}

#[test]
fn a_relative_path_is_also_read_from_the_users_directory_outside_the_root() {
    let root = root();
    let outside = top("/nonexistent-git-dupe/w");
    let from = ReadFrom {
        root: Path::new(&root),
        outside: Some(Path::new(&outside)),
        home: None,
    };
    let mut expected = local_forms(&format!("{root}/repo"));
    expected.extend(local_forms(&root));
    assert_eq!(forms(b"repo", from), expected);
    let absolute = top("/srv/x");
    assert_eq!(
        locals(forms(absolute.as_bytes(), from)),
        local_forms(&absolute)
    );
}

#[test]
fn a_path_that_is_tilde_or_begins_with_tilde_slash_is_read_from_home() {
    let root = root();
    let home = top("/nonexistent-git-dupe/h");
    let outside = top("/nonexistent-git-dupe/w");
    let with = |home: Option<&str>, word: &str| {
        let from = ReadFrom {
            root: Path::new(&root),
            outside: Some(Path::new(&outside)),
            home: home.map(str::as_bytes),
        };
        forms(word.as_bytes(), from)
    };
    // `HOME` in place of the `~`, after the trailing slashes are dropped; never joined
    // to the root or the user's directory when `HOME` is absolute.
    for word in ["~", "~/", "~//"] {
        assert_eq!(with(Some(&home), word), local_forms(&home), "{word}");
    }
    for word in ["~/proj", "~/proj/", "~//proj"] {
        assert_eq!(
            with(Some(&home), word),
            local_forms(&format!("{home}/proj")),
            "{word}"
        );
    }
    assert_eq!(
        with(Some(&home), "~/../w/repo"),
        local_forms(&root),
        "cleaned as text where nothing exists"
    );
    // `.git` is appended to what `HOME` gives, its own trailing slash kept, as Git
    // appends it.
    let git = format!("{home}/.git");
    let slashed: Vec<Form> = [home.as_str(), &git, &git, &format!("{git}/.git")]
        .map(|path| Form::Local(path.as_bytes().to_vec()))
        .to_vec();
    assert_eq!(with(Some(&format!("{home}/")), "~"), slashed);
    // A relative `HOME` gives a relative path, read where any relative one is.
    let mut expected = local_forms(&format!("{root}/h/proj"));
    expected.extend(local_forms(&format!("{outside}/h/proj")));
    assert_eq!(with(Some("h"), "~/proj"), expected);
    // Where `HOME` is unset Git opens nothing for it.
    assert_eq!(with(None, "~/proj"), []);
    assert_eq!(with(None, "~"), []);
    // `~<user>`, and a `~` anywhere but first, are read as written.
    let mut expected = local_forms(&format!("{root}/~someone/proj"));
    expected.extend(local_forms(&format!("{outside}/~someone/proj")));
    assert_eq!(with(Some(&home), "~someone/proj"), expected);
    assert_eq!(with(None, "~someone/proj"), expected);
    let mut expected = local_forms(&format!("{root}/~"));
    expected.extend(local_forms(&format!("{outside}/~")));
    assert_eq!(with(Some(&home), "./~"), expected);
    // A URL's path is never read from `HOME`.
    assert_eq!(
        with(Some(&home), "ssh://example.com/~/p.git"),
        [url_form("example.com", "/~/p")]
    );
    assert!(with(Some(&home), "file://~/p").contains(&local_forms(&read("/p"))[0]));
    // Written with `\`, it is no such form, where that is a separator too (S13).
    assert_eq!(
        with(Some(&home), "~\\proj")[0],
        Form::Local(operand::with_slashes(format!("{root}/~\\proj").as_bytes()))
    );
}

#[test]
fn a_one_letter_host_is_also_a_local_path_whole_and_as_what_follows_its_colon() {
    let root = root();
    let outside = top("/u");
    let from = ReadFrom {
        root: Path::new(&root),
        outside: Some(Path::new(&outside)),
        home: None,
    };
    let local = |paths: &[String]| {
        paths
            .iter()
            .flat_map(|path| local_forms(path))
            .collect::<Vec<_>>()
    };

    let mut expected = vec![url_form("x", "/proj")];
    expected.extend(local(&[
        format!("{root}/x:proj"),
        format!("{outside}/x:proj"),
        format!("{root}/proj"),
        format!("{outside}/proj"),
    ]));
    assert_eq!(forms(b"x:proj", from), expected);
    assert_eq!(forms(b"X:proj", from)[0], url_form("x", "/proj"));

    // A separator after the colon: the word whole, read as the platform reads it, is the
    // local path, and what follows the colon no relative one.
    let mut expected = vec![url_form("x", "/abs")];
    for path in joined(b"x:/abs", from) {
        let canonical = canonical(&path);
        expected.extend(local_forms(std::str::from_utf8(&canonical).unwrap()));
    }
    assert_eq!(forms(b"x:/abs", from), expected);

    // Whatever follows the colon is a relative path, one a local path would read otherwise
    // included, and nothing at all is the root.
    assert!(forms(b"x:~/p", from).contains(&local_forms(&format!("{root}/~/p"))[0]));
    assert!(forms(b"x:", from).contains(&Form::Local(root.clone().into_bytes())));
    assert!(forms(b"x:.", from).contains(&Form::Local(root.clone().into_bytes())));

    // A user changes nothing of the host's length, and a longer host or a digit is no drive.
    assert!(forms(b"git@x:proj", from).contains(&local_forms(&format!("{root}/proj"))[0]));
    assert_eq!(forms(b"git@host:proj", from), [url_form("host", "/proj")]);
    assert_eq!(forms(b"xy:proj", from), [url_form("xy", "/proj")]);
    assert_eq!(forms(b"1:proj", from), [url_form("1", "/proj")]);
}

#[test]
fn a_file_url_on_a_drive_also_has_the_path_on_that_drive() {
    let root = root();
    let on_the_drive = joined(b"C:/repo", from(Path::new(&root)));
    let on_the_drive = Form::Local(canonical(&on_the_drive[0]));
    for word in ["file:///C:/repo", "file:///C:/repo/", "file:///C%3A/repo"] {
        let forms = of(word);
        assert_eq!(forms[0], url_form("", "/C:/repo"), "{word}");
        assert!(forms.contains(&on_the_drive), "{word}");
        for form in local_forms(&read("/C:/repo")) {
            assert!(forms.contains(&form), "{word}");
        }
    }
    let forms = of("file://C:/repo");
    assert_eq!(forms[0], url_form("c", "/repo"));
    assert!(forms.contains(&on_the_drive));
    for form in local_forms(&read("/repo")) {
        assert!(forms.contains(&form));
    }
    // No drive there: the forms of any `file://` URL and no others.
    let mut expected = vec![url_form("", "/ab:/repo")];
    expected.extend(local_forms(&read("/ab:/repo")));
    assert_eq!(of("file:///ab:/repo"), expected);
    let mut expected = vec![url_form("", "/C:repo")];
    expected.extend(local_forms(&read("/C:repo")));
    // A drive and no root, which where a root alone is not absolute is also a relative path.
    assert_eq!(
        of("file:///C:repo")[..expected.len()] == expected[..]
            && of("file:///C:repo").len() == expected.len(),
        Path::new("/").is_absolute()
    );
    // Any other scheme opens no path on a drive.
    assert_eq!(of("https://example.com/C:/repo")[1..], []);
}

#[test]
fn a_root_and_no_drive_is_opened_on_the_drive_it_is_read_from() {
    let path = "/nonexistent-git-dupe/p";
    let spelled = path.replace('/', "\\");
    for word in [path, &spelled] {
        let expected = if path::is_separator('\\') || word == path {
            local_forms(&top(path))
        } else {
            local_forms(&format!("{}/{word}", root()))
        };
        assert_eq!(of(word), expected, "{word}");
    }
}

#[test]
fn a_drive_written_as_a_directory_is_also_the_drive_where_a_root_alone_is_not_absolute() {
    let on_the_drive = |path: &str| {
        Form::Local(canonical(
            joined(path.as_bytes(), from(Path::new(&root())))[0].as_slice(),
        ))
    };
    for word in [
        "/c/repo",
        "/C/repo/",
        "file:///c/repo",
        "/c\\repo",
        "file:///c\\repo",
        "file:///c%5Crepo",
    ] {
        let forms = of(word);
        let shell =
            forms.contains(&on_the_drive("c:/repo")) || forms.contains(&on_the_drive("C:/repo"));
        assert_eq!(shell, !Path::new("/").is_absolute(), "{word}");
    }
    // Only a first component of one letter: anything else names no drive there.
    for word in ["/ab/repo", "/1/repo", "/c:repo"] {
        assert!(
            locals(of(word))
                .iter()
                .all(|form| { !matches!(form, Form::Local(path) if path.starts_with(b"C:/repo")) })
        );
    }
}

#[test]
fn what_the_shell_opens_past_a_drive_as_a_directory_is_also_a_form_where_a_root_alone_is_not_absolute()
 {
    let root = root();
    let form = |path: &[u8]| local(path, from(Path::new(&root)))[0].clone();
    let below_the_temporary_directory = std::env::temp_dir().join("repo");
    let places: [(&[&str], Vec<u8>); 3] = [
        (
            &["/c:/repo", "/c:/repo/", "/c:\\repo", "file:///c:\\repo"],
            b"c:/repo".to_vec(),
        ),
        (
            &["/g", "file:///g", "/g:", "/g:/", "/g:\\"],
            b"g:/".to_vec(),
        ),
        (
            &["/tmp/repo", "/TMP/repo", "/Tmp\\repo", "file:///tmp/repo"],
            bytes::of(below_the_temporary_directory.as_os_str()).to_vec(),
        ),
    ];
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for (words, place) in places {
        for word in words {
            assert!(
                of(word).contains(&form(&place)) || a_root_alone_is_absolute,
                "{word}"
            );
        }
    }
    // Where a root alone is absolute, the path is that path and no other.
    for word in ["/c:/repo", "/g", "/tmp/repo"] {
        let paths = joined(word.as_bytes(), from(Path::new(&root)));
        assert_eq!(paths.len() == 1, a_root_alone_is_absolute, "{word}");
    }
    // Any other first component opens nothing of the kind.
    for word in ["/tmpx/repo", "/ab", "/1", "/ab:/repo", "/1:/repo"] {
        let paths = through_the_shell(word.as_bytes(), from(Path::new(&root)));
        assert_eq!(paths, Vec::<Vec<u8>>::new(), "{word}");
    }
}

/// The form of the root itself, which a word Git opens from there has.
fn at_the_root(root: &str) -> Form {
    Form::Local(canonical(root.as_bytes()))
}

#[test]
fn a_drives_top_is_also_where_git_stands_on_that_drive_once_it_drops_the_slash() {
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for word in [
        "C:/",
        "c:/",
        "file:///C:/",
        "file://C:/",
        "/c",
        "/C",
        "/c/",
        "/c\\",
        "/c:",
        "/c:/",
        "/C:\\",
        "file:///c/",
        "/./c",
    ] {
        assert_eq!(
            of(word).contains(&at_the_root(&root())),
            !a_root_alone_is_absolute,
            "{word}"
        );
    }
    // Below the top, and on a drive the root does not stand on, Git stands nowhere of it.
    for word in ["C:/x", "G:/", "/g", "file:///G:/", "file://C:/x"] {
        assert!(!of(word).contains(&at_the_root(&root())), "{word}");
    }
}

#[test]
fn a_dot_before_the_first_component_is_no_component_to_the_shell() {
    let root = root();
    let form = |path: &[u8]| local(path, from(Path::new(&root)))[0].clone();
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for (word, place) in [
        ("/./c/repo", &b"c:/repo"[..]),
        ("/.//./C:/repo", b"C:/repo"),
        ("/.\\g", b"g:/"),
    ] {
        assert_eq!(
            of(word).contains(&form(place)),
            !a_root_alone_is_absolute,
            "{word}"
        );
    }
}

#[test]
fn a_drive_and_no_root_in_a_file_url_is_read_from_the_root_where_the_platform_has_drives() {
    let root = root();
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for word in [
        "file://C:",
        "file://c:",
        "file:///C:",
        "file://C:.",
        "file://%43:",
        "file:///C:.",
        "file://C:sub/..",
        "file:///C:sub/..",
    ] {
        assert_eq!(
            of(word).contains(&at_the_root(&root)),
            !a_root_alone_is_absolute,
            "{word}"
        );
    }
    let below = Form::Local(canonical(format!("{root}/.git").as_bytes()));
    assert_eq!(
        of("file://C:.git").contains(&below),
        !a_root_alone_is_absolute
    );
    let beside = Form::Local(canonical(format!("{root}/../x").as_bytes()));
    assert_eq!(
        of("file://C:../x").contains(&beside),
        !a_root_alone_is_absolute
    );
}

#[test]
fn a_drive_and_no_root_after_the_shells_first_slash_is_read_from_the_root() {
    let root = root();
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for word in ["/C:.", "/c:sub/..", "/C:sub\\.."] {
        assert_eq!(
            of(word).contains(&at_the_root(&root)),
            !a_root_alone_is_absolute,
            "{word}"
        );
    }
    let below = Form::Local(canonical(format!("{root}/.git").as_bytes()));
    assert_eq!(of("/C:.git").contains(&below), !a_root_alone_is_absolute);
}

#[test]
fn the_shells_proc_is_the_root_where_a_root_alone_is_not_absolute() {
    let root = root();
    let a_root_alone_is_absolute = Path::new("/").is_absolute();
    for word in [
        "/proc",
        "/proc/cygdrive/c/repo",
        "/proc/self/cwd/",
        "/proc\\cygdrive\\c",
        "/./proc/self/root/c/repo",
        "file:///proc/self/cwd/",
    ] {
        assert_eq!(
            of(word).contains(&at_the_root(&root)),
            !a_root_alone_is_absolute,
            "{word}"
        );
    }
    // The shell's `/proc` is spelled in lower case alone, and is a whole component.
    for word in ["/PROC/cygdrive/c/repo", "/procx/y", "/x/proc/y"] {
        assert!(!of(word).contains(&at_the_root(&root)), "{word}");
    }
}

#[test]
fn a_backslash_after_a_file_urls_drive_is_a_separator_where_the_platform_says_so() {
    let on_the_drive = Form::Local(canonical(
        joined(b"C:\\repo", from(Path::new(&root())))[0].as_slice(),
    ));
    for word in ["file://C:\\repo", "file:///C:\\repo", "file:///C:%5Crepo"] {
        assert_eq!(
            of(word).contains(&on_the_drive),
            path::is_separator('\\'),
            "{word}"
        );
    }
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
    fs::write(real.join("repo/beside"), b"").unwrap();
    symlink(real.join("repo"), real.join("to-repo")).unwrap();
    symlink(real.join("repo/sub"), real.join("elsewhere/link")).unwrap();
    symlink(real.join("loop"), real.join("loop")).unwrap();
    let bytes = |path: &Path| crate::bytes::of(path.as_os_str()).to_vec();
    let repo = bytes(&real.join("repo"));

    assert_eq!(canonical(&bytes(&real.join("to-repo"))), repo);
    // `link/..` beyond a link is where the platform opens it: the root, `..` taken after
    // the link, on Linux and macOS, and `elsewhere` on Windows, which reads it as text.
    let beyond_the_link = if real.join("elsewhere/link/../beside").exists() {
        repo.clone()
    } else {
        bytes(&real.join("elsewhere"))
    };
    assert_eq!(
        canonical(&bytes(&real.join("elsewhere/link/.."))),
        beyond_the_link
    );
    // Through a directory that does not exist: cleaned as text.
    assert_eq!(canonical(&bytes(&real.join("repo/absent/.."))), repo);
    // A loop of links, which Git cannot open either: cleaned as text.
    assert_eq!(
        canonical(&bytes(&real.join("loop/x/.."))),
        operand::clean_absolute(&bytes(&real.join("loop")))
    );
    // The root as Git prints it, which on Windows a real path is not.
    let root = scratch.0.join("repo");
    assert!(forms(b"../to-repo", from(&root)).contains(&Form::Local(repo.clone())));
    assert!(!forms(b"../repo-other", from(&root)).contains(&Form::Local(repo)));
}
