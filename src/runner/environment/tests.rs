//! The rules of the environments, over a received environment written here.

use super::*;
use std::collections::BTreeMap;

/// A caller's environment as a hook of the public repository sees it, run with
/// `--literal-pathspecs` and `-c` before `dupe`.
fn received() -> Vec<(OsString, OsString)> {
    [
        ("HOME", "/home/u"),
        ("PATH", "/bin"),
        ("GIT_DIR", ".git"),
        ("GIT_WORK_TREE", "sub/.."),
        ("GIT_INDEX_FILE", "/w/repo/.git/next-index.lock"),
        ("GIT_COMMON_DIR", "/w/repo/.git"),
        ("GIT_OBJECT_DIRECTORY", "/w/repo/.git/objects"),
        ("GIT_ALTERNATE_OBJECT_DIRECTORIES", "/elsewhere/objects"),
        ("GIT_REFERENCE_BACKEND", "files:///w/repo/.git"),
        ("GIT_LITERAL_PATHSPECS", "1"),
        ("GIT_GLOB_PATHSPECS", "0"),
        ("GIT_NOGLOB_PATHSPECS", "0"),
        ("GIT_ICASE_PATHSPECS", "1"),
        ("GIT_CONFIG", "/w/other.config"),
        ("GIT_CONFIG_PARAMETERS", "'color.ui'='never'"),
        ("GIT_PAGER", "cat"),
        ("GIT_NAMESPACE", "space"),
    ]
    .into_iter()
    .map(|(name, value)| (name.into(), value.into()))
    .collect()
}

/// The lookup of a process that received `received`.
fn lookup(received: &[(OsString, OsString)]) -> impl Fn(&str) -> Option<OsString> {
    let received: BTreeMap<OsString, OsString> = received.iter().cloned().collect();
    move |name: &str| received.get(&OsString::from(name)).cloned()
}

/// `received` as the changes leave it.
fn applied(changes: Changes, received: Vec<(OsString, OsString)>) -> BTreeMap<OsString, OsString> {
    let mut environment: BTreeMap<OsString, OsString> = received.into_iter().collect();
    for name in changes.removed {
        environment.remove(&OsString::from(name));
    }
    for (name, value) in changes.set {
        environment.insert(name.into(), value);
    }
    environment
}

/// The user's directory these checks run from, joined to a relative location by what
/// separates a path on this platform.
fn user_directory() -> PathBuf {
    PathBuf::from("/w/repo/sub")
}

fn from_the_user(location: &str) -> String {
    user_directory().join(location).display().to_string()
}

fn built(
    against: &Against,
    command: &str,
    own_paths: bool,
    received: Vec<(OsString, OsString)>,
) -> BTreeMap<OsString, OsString> {
    let changes = of(
        against,
        (!command.is_empty()).then_some(command.as_bytes()),
        own_paths,
        false,
        lookup(&received),
        || Ok(user_directory()),
    )
    .expect("an environment");
    applied(changes, received)
}

/// The names `received` holds that `environment` lacks, and the ones it holds with
/// another value, each with the value it now has.
fn changes(environment: &BTreeMap<OsString, OsString>) -> Vec<(String, Option<String>)> {
    let before: BTreeMap<OsString, OsString> = received().into_iter().collect();
    let mut changed = Vec::new();
    for (name, value) in &before {
        match environment.get(name) {
            Some(now) if now == value => {}
            now => changed.push((
                name.to_string_lossy().into_owned(),
                now.map(|now| now.to_string_lossy().into_owned()),
            )),
        }
    }
    for (name, value) in environment {
        if !before.contains_key(name) {
            changed.push((
                name.to_string_lossy().into_owned(),
                Some(value.to_string_lossy().into_owned()),
            ));
        }
    }
    changed.sort();
    changed
}

fn named(pairs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    let mut pairs: Vec<_> = pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.map(str::to_string)))
        .collect();
    pairs.sort();
    pairs
}

fn private() -> Against<'static> {
    Against::Private {
        git_directory: Path::new("/w/repo/.git/dupe"),
        root: Path::new("/w/repo"),
    }
}

#[test]
fn a_public_run_makes_relative_locations_absolute_and_changes_nothing_else() {
    let environment = built(&Against::Public, "rev-parse", false, received());
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some(&from_the_user(".git"))),
            ("GIT_WORK_TREE", Some(&from_the_user("sub/.."))),
        ])
    );
}

#[test]
fn a_public_listing_of_git_dupes_own_paths_reads_the_public_index() {
    let environment = built(&Against::Public, "ls-files", true, received());
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some(&from_the_user(".git"))),
            ("GIT_WORK_TREE", Some(&from_the_user("sub/.."))),
            ("GIT_INDEX_FILE", None),
            ("GIT_LITERAL_PATHSPECS", None),
            ("GIT_GLOB_PATHSPECS", None),
            ("GIT_NOGLOB_PATHSPECS", None),
            ("GIT_ICASE_PATHSPECS", None),
        ])
    );
}

#[test]
fn the_listing_in_the_users_index_keeps_it_and_a_private_run_never_does() {
    let in_the_users_index = |against: &Against| {
        let changes = of(
            against,
            Some(b"ls-files"),
            true,
            true,
            lookup(&received()),
            || Ok(user_directory()),
        )
        .expect("an environment");
        applied(changes, received())
    };
    let environment = in_the_users_index(&Against::Public);
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some(&from_the_user(".git"))),
            ("GIT_WORK_TREE", Some(&from_the_user("sub/.."))),
            ("GIT_LITERAL_PATHSPECS", None),
            ("GIT_GLOB_PATHSPECS", None),
            ("GIT_NOGLOB_PATHSPECS", None),
            ("GIT_ICASE_PATHSPECS", None),
        ])
    );
    let environment = in_the_users_index(&private());
    assert!(!environment.contains_key(&OsString::from("GIT_INDEX_FILE")));
}

#[test]
fn a_public_check_ignore_keeps_the_index_variable_it_does_not_read() {
    let environment = built(&Against::Public, "check-ignore", true, received());
    assert_eq!(
        environment.get(&OsString::from("GIT_INDEX_FILE")),
        Some(&OsString::from("/w/repo/.git/next-index.lock"))
    );
    assert!(!environment.contains_key(&OsString::from("GIT_LITERAL_PATHSPECS")));
}

#[test]
fn a_private_run_names_the_private_repository_whatever_the_caller_set() {
    let environment = built(&private(), "cat-file", false, received());
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some("/w/repo/.git/dupe")),
            ("GIT_WORK_TREE", Some("/w/repo")),
            ("GIT_INDEX_FILE", None),
            ("GIT_COMMON_DIR", None),
            ("GIT_OBJECT_DIRECTORY", None),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", None),
            ("GIT_REFERENCE_BACKEND", None),
        ])
    );
}

#[test]
fn a_private_run_of_git_dupes_own_paths_drops_the_pathspec_variables() {
    let environment = built(&private(), "ls-files", true, received());
    for name in PATHSPEC {
        assert!(!environment.contains_key(&OsString::from(name)), "{name}");
    }
    // What Git set for the global options before `dupe` still reaches the run.
    for name in [
        "GIT_CONFIG_PARAMETERS",
        "GIT_PAGER",
        "GIT_NAMESPACE",
        "GIT_CONFIG",
    ] {
        assert!(environment.contains_key(&OsString::from(name)), "{name}");
    }
}

#[test]
fn a_config_run_of_either_kind_drops_git_config() {
    for against in [&Against::Public, &private()] {
        let environment = built(against, "config", false, received());
        assert!(!environment.contains_key(&OsString::from("GIT_CONFIG")));
        assert!(environment.contains_key(&OsString::from("GIT_LITERAL_PATHSPECS")));
    }
}

#[test]
fn a_run_whose_command_word_is_the_users_takes_no_rule_from_that_word() {
    // The empty word stands for the user's: `config` and `ls-files` typed by the user
    // keep `GIT_CONFIG` and `GIT_INDEX_FILE` as plain Git would, and the pathspec
    // variables reach the user's own pathspecs.
    let environment = built(&Against::Public, "", false, received());
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some(&from_the_user(".git"))),
            ("GIT_WORK_TREE", Some(&from_the_user("sub/.."))),
        ])
    );
    let environment = built(&private(), "", false, received());
    for name in ["GIT_CONFIG", "GIT_LITERAL_PATHSPECS", "GIT_ICASE_PATHSPECS"] {
        assert!(environment.contains_key(&OsString::from(name)), "{name}");
    }
    assert!(!environment.contains_key(&OsString::from("GIT_INDEX_FILE")));
}

#[test]
fn where_the_locate_named_only_a_git_directory_the_run_has_no_working_tree() {
    let against = Against::PrivateWithoutWorkTree {
        git_directory: Path::new("/w/bare.git/dupe"),
    };
    let environment = built(&against, "", false, received());
    assert_eq!(
        changes(&environment),
        named(&[
            ("GIT_DIR", Some("/w/bare.git/dupe")),
            ("GIT_WORK_TREE", None),
            ("GIT_INDEX_FILE", None),
            ("GIT_COMMON_DIR", None),
            ("GIT_OBJECT_DIRECTORY", None),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", None),
            ("GIT_REFERENCE_BACKEND", None),
        ])
    );
    let environment = built(&against, "config", false, received());
    assert!(!environment.contains_key(&OsString::from("GIT_CONFIG")));
}

#[test]
fn a_private_run_sets_the_locations_where_the_caller_set_none() {
    let environment = built(&private(), "ls-files", true, Vec::new());
    assert_eq!(
        environment,
        BTreeMap::from([
            ("GIT_DIR".into(), "/w/repo/.git/dupe".into()),
            ("GIT_WORK_TREE".into(), "/w/repo".into()),
        ])
    );
}

#[test]
fn absolute_locations_need_no_directory_and_are_kept_as_they_came() {
    // Locations this platform calls absolute, `C:\\…` on Windows and `/…` elsewhere.
    let absolute = std::env::temp_dir();
    let received = vec![
        (OsString::from("GIT_DIR"), absolute.join(".git").into()),
        (
            OsString::from("GIT_WORK_TREE"),
            absolute.join("caf\u{e9}").into(),
        ),
    ];
    let changes = of(
        &Against::Public,
        Some(b"ls-files"),
        true,
        false,
        lookup(&received),
        || Err(io::Error::other("no directory")),
    )
    .expect("no directory needed");
    assert!(changes.set.is_empty(), "{:?}", changes.set);
    assert_eq!(
        applied(changes, received.clone()),
        received.into_iter().collect()
    );
}

#[test]
fn a_relative_location_without_a_user_directory_is_a_failure() {
    let failed = of(
        &Against::Public,
        Some(b"ls-files"),
        true,
        false,
        lookup(&received()),
        || Err(io::Error::other("no directory")),
    );
    assert!(failed.is_err());
}

#[test]
fn a_lookup_that_folds_names_finds_a_received_git_dir_and_sets_it_back_as_git_dir() {
    // The lookup of a process on Windows, which compares names without regard to case.
    let folded = |name: &str| name.eq_ignore_ascii_case("Git_Dir").then(|| ".git".into());
    let changes = of(
        &Against::Public,
        Some(b"rev-parse"),
        false,
        false,
        folded,
        || Ok(user_directory()),
    )
    .expect("an environment");
    assert_eq!(
        changes,
        Changes {
            removed: Vec::new(),
            set: vec![("GIT_DIR", from_the_user(".git").into())],
        }
    );
}
