use super::*;

/// A configuration as Git would answer for it: the records configured, then each
/// `-c alias.<name>=<value>` of the prefix asked under, as Git lists a `-c` last.
struct Configured {
    records: Vec<(&'static str, Option<&'static str>)>,
    listed: Vec<&'static str>,
    /// The prefix of each lookup, in order.
    lookups: Vec<Vec<OsString>>,
    lists: usize,
    failing: bool,
    /// An option Git cannot evaluate: a lookup carrying it fails.
    refused: Option<&'static str>,
}

fn configured(records: &[(&'static str, &'static str)]) -> Configured {
    Configured {
        records: records
            .iter()
            .map(|&(name, value)| (name, Some(value)))
            .collect(),
        listed: vec!["log", "show", "whatchanged", "rev-parse", "commit"],
        lookups: Vec::new(),
        lists: 0,
        failing: false,
        refused: None,
    }
}

impl Questions for Configured {
    type Failure = &'static str;

    fn aliases(&mut self, prefix: &[OsString]) -> Result<Vec<Record>, &'static str> {
        self.lookups.push(prefix.to_vec());
        if self.failing
            || self
                .refused
                .is_some_and(|refused| prefix.iter().any(|word| word == refused))
        {
            return Err("lookup");
        }
        let mut found: Vec<Record> = self
            .records
            .iter()
            .map(|&(name, value)| {
                (
                    name.as_bytes().to_vec(),
                    value.map(|v| v.as_bytes().to_vec()),
                )
            })
            .collect();
        for pair in prefix.windows(2) {
            let setting = pair[1].as_bytes();
            if pair[0] == "-c"
                && let Some(setting) = setting.strip_prefix(b"alias.")
            {
                let at = setting.iter().position(|&b| b == b'=').expect("a value");
                found.push((setting[..at].to_vec(), Some(setting[at + 1..].to_vec())));
            }
        }
        Ok(found)
    }

    fn commands(&mut self) -> Result<BTreeSet<Vec<u8>>, &'static str> {
        self.lists += 1;
        Ok(self
            .listed
            .iter()
            .map(|word| word.as_bytes().to_vec())
            .collect())
    }
}

fn owned(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}

fn resolved(config: &mut Configured, typed: &[&str]) -> Resolved {
    resolve_with(&owned(typed), config).expect("no failure")
}

fn dispatch(command: Command, words: &[&str], prefix: &[&str]) -> Resolved {
    Resolved::Dispatch {
        command,
        words: owned(words),
        prefix: owned(prefix),
    }
}

#[test]
fn a_word_without_a_record_or_with_a_shell_alias_passes_through() {
    let mut config = configured(&[("sh1", "!echo x"), ("Other", "status")]);
    assert_eq!(resolved(&mut config, &["log", "-1"]), Resolved::PassThrough);
    assert_eq!(resolved(&mut config, &["nosuch"]), Resolved::PassThrough);
    assert_eq!(resolved(&mut config, &["sh1"]), Resolved::PassThrough);
    // A shell alias and a word without a record need no list.
    assert_eq!(config.lists, 0);
    assert_eq!(config.lookups.len(), 3);
}

#[test]
fn a_chain_that_reaches_a_command_of_f6_is_dispatched_with_its_words() {
    let mut config = configured(&[
        ("sp", "status --porc"),
        ("aa", "add -A"),
        ("su", "stash -u"),
        ("s", "stash"),
        ("wipe", "clean -fdx"),
        ("g", "git status --porc"),
        ("st", "stage notes"),
        ("i", "init"),
    ]);
    let cases: [(&[&str], Command, &[&str]); 9] = [
        (&["sp"], Command::Status, &["status", "--porc"]),
        (&["SP", "-v"], Command::Status, &["status", "--porc", "-v"]),
        (&["aa"], Command::Add, &["add", "-A"]),
        (&["su"], Command::Stash, &["stash", "-u"]),
        (&["s", "-u"], Command::Stash, &["stash", "-u"]),
        (&["wipe", "-h"], Command::Clean, &["clean", "-fdx", "-h"]),
        (&["g"], Command::Git, &["git", "status", "--porc"]),
        (&["st"], Command::Add, &["add", "notes"]),
        (&["i"], Command::Init, &["init"]),
    ];
    for (typed, command, words) in cases {
        assert_eq!(
            resolved(&mut config, typed),
            dispatch(command, words, &[]),
            "{typed:?}"
        );
    }
}

#[test]
fn each_expansion_stands_before_what_remains_and_its_options_lead_the_command() {
    let mut config = configured(&[
        ("a", "-c x.y=1 b two"),
        ("b", "-p status one"),
        ("st", "-c status.short=true status"),
    ]);
    assert_eq!(
        resolved(&mut config, &["a", "three"]),
        dispatch(
            Command::Status,
            &["status", "one", "two", "three"],
            &["-c", "x.y=1", "-p"]
        )
    );
    // The second lookup carried the first link's options, and a third the last link's
    // before the dispatch; the list was asked once.
    assert_eq!(
        config.lookups,
        [
            owned(&[]),
            owned(&["-c", "x.y=1"]),
            owned(&["-c", "x.y=1", "-p"])
        ]
    );
    assert_eq!(config.lists, 1);
    assert_eq!(
        resolved(&mut config, &["st"]),
        dispatch(Command::Status, &["status"], &["-c", "status.short=true"])
    );
}

#[test]
fn options_no_lookup_carried_are_evaluated_before_the_dispatch() {
    let mut config = configured(&[
        ("bad", "--config-env=broken status -h"),
        ("ok", "-p status"),
        ("outer", "-c x.y=1 inner"),
        ("inner", "status"),
    ]);
    config.refused = Some("--config-env=broken");
    assert_eq!(resolve_with(&owned(&["bad"]), &mut config), Err("lookup"));
    assert_eq!(
        config.lookups,
        [owned(&[]), owned(&["--config-env=broken"])]
    );
    config.lookups.clear();
    assert_eq!(
        resolved(&mut config, &["ok"]),
        dispatch(Command::Status, &["status"], &["-p"])
    );
    assert_eq!(config.lookups, [owned(&[]), owned(&["-p"])]);
    // Options a later link's lookup carried are not asked about again.
    config.lookups.clear();
    assert_eq!(
        resolved(&mut config, &["outer"]),
        dispatch(Command::Status, &["status"], &["-c", "x.y=1"])
    );
    assert_eq!(config.lookups, [owned(&[]), owned(&["-c", "x.y=1"])]);
}

#[test]
fn a_later_lookup_sees_the_alias_a_leading_option_defines() {
    let mut config = configured(&[("a", "-c alias.b=stash\\ -u b")]);
    assert_eq!(
        resolved(&mut config, &["a"]),
        dispatch(
            Command::Stash,
            &["stash", "-u"],
            &["-c", "alias.b=stash -u"]
        )
    );
}

#[test]
fn a_chain_git_cannot_follow_passes_the_typed_words_through() {
    let mut config = configured(&[
        ("l1", "l2"),
        ("l2", "l1"),
        ("self", "self"),
        ("bad", "status 'x"),
        ("env", "--git-dir=. status"),
        ("q", "--exec-path status"),
        ("v", "--version"),
        ("empty", ""),
        ("blanks", "   "),
        ("lead", " status"),
        ("opts", "-c x.y=1"),
    ]);
    config.records.push(("novalue", None));
    for typed in [
        "l1", "self", "bad", "env", "q", "v", "empty", "blanks", "lead", "opts", "novalue",
    ] {
        assert_eq!(
            resolved(&mut config, &[typed]),
            Resolved::PassThrough,
            "{typed}"
        );
    }
}

#[test]
fn a_chain_followed_to_another_command_carries_the_words_git_runs() {
    let mut config = configured(&[
        ("co", "checkout"),
        ("cod", "checkout --detach"),
        ("a", "b one"),
        ("b", "-c x.y=1 rebase two"),
        ("lg", "log"),
        ("log", "!echo"),
        ("sw", "show -1"),
        ("show", "commit"),
        ("x", "sh"),
        ("sh", "!git checkout HEAD~1"),
    ]);
    let cases: [(&[&str], &[&str]); 6] = [
        (&["co", "HEAD~1"], &["checkout", "HEAD~1"]),
        (&["cod", "HEAD~1"], &["checkout", "--detach", "HEAD~1"]),
        // The innermost expansion first, its options in no word.
        (&["a", "three"], &["rebase", "two", "one", "three"]),
        // A builtin runs before its own alias, a `!` one or one only some releases read.
        (&["lg", "-1"], &["log", "-1"]),
        (&["sw"], &["show", "-1"]),
        // A `!` alias reached through a link: what Git runs is no command word.
        (&["x"], &["sh"]),
    ];
    for (typed, words) in cases {
        assert_eq!(
            resolved(&mut config, typed),
            Resolved::PassThroughReaching(owned(words)),
            "{typed:?}"
        );
    }
    // No link followed: the typed words alone.
    for typed in [&["sh"][..], &["checkout", "HEAD~1"], &["log", "-1"]] {
        assert_eq!(
            resolved(&mut config, typed),
            Resolved::PassThrough,
            "{typed:?}"
        );
    }
    // A chain followed to a command Git runs, with options of its own, as a chain that
    // ends anywhere else: one lookup per link, the list asked once.
    let mut config = configured(&[("down", "-c x.y=1 log --oneline")]);
    assert_eq!(
        resolved(&mut config, &["down", "-2"]),
        Resolved::PassThroughReaching(owned(&["log", "--oneline", "-2"]))
    );
    assert_eq!(config.lookups, [owned(&[]), owned(&["-c", "x.y=1"])]);
    assert_eq!(config.lists, 1);
}

#[test]
fn a_loop_ends_without_another_lookup() {
    let mut config = configured(&[("l1", "l2"), ("l2", "l1")]);
    assert_eq!(resolved(&mut config, &["l1"]), Resolved::PassThrough);
    assert_eq!(config.lookups.len(), 2);
}

#[test]
fn a_listed_word_with_a_record_is_refused_only_where_a_chain_reaches_f6() {
    let mut config = configured(&[
        ("log", "add -A"),
        ("show", "log"),
        ("whatchanged", "stash -u"),
        ("a", "whatchanged"),
        ("b", "commit -m x"),
    ]);
    config.records.push(("commit", Some("show")));
    assert_eq!(
        resolved(&mut config, &["log"]),
        Resolved::Ambiguous(b"log".to_vec())
    );
    assert_eq!(
        resolved(&mut config, &["whatchanged"]),
        Resolved::Ambiguous(b"whatchanged".to_vec())
    );
    // One link down.
    assert_eq!(
        resolved(&mut config, &["a"]),
        Resolved::Ambiguous(b"whatchanged".to_vec())
    );
    // `show` is listed and aliased to `log`, which is listed and aliased to `add -A`.
    assert_eq!(
        resolved(&mut config, &["show"]),
        Resolved::Ambiguous(b"show".to_vec())
    );
    // `commit` reaches `show`, `log`, then `add`: refused too; `b` reaches `commit`.
    assert_eq!(
        resolved(&mut config, &["b"]),
        Resolved::Ambiguous(b"commit".to_vec())
    );
    let mut config = configured(&[("log", "show"), ("show", "!echo")]);
    assert_eq!(resolved(&mut config, &["log", "-1"]), Resolved::PassThrough);
}

#[test]
fn a_form_only_some_releases_read_is_refused_only_where_a_chain_reaches_f6() {
    let mut config = configured(&[
        ("wipe.command", "stash -u"),
        (".w", "stash -u"),
        ("Foo.x", "stash -u"),
        ("both", "log"),
        ("both.command", "stash -u"),
        ("lg.command", "log --oneline"),
        ("Up.command", "add -A"),
    ]);
    for (typed, named) in [
        ("wipe", "wipe"),
        ("w", "w"),
        ("W", "W"),
        ("foo.x", "foo.x"),
        ("both", "both"),
        ("wipe.command", "wipe.command"),
        ("Up", "Up"),
    ] {
        assert_eq!(
            resolved(&mut config, &[typed]),
            Resolved::Ambiguous(named.as_bytes().to_vec()),
            "{typed}"
        );
    }
    // `.command` compares exactly; the dotted form without regard to case.
    assert_eq!(resolved(&mut config, &["up"]), Resolved::PassThrough);
    assert_eq!(resolved(&mut config, &["lg"]), Resolved::PassThrough);
}

#[test]
fn a_chain_from_a_word_read_differently_follows_plain_links_and_its_own_prefix() {
    let mut config = configured(&[
        ("odd.command", "-c alias.inner=stash\\ -u outer"),
        ("outer", "inner"),
    ]);
    assert_eq!(
        resolved(&mut config, &["odd"]),
        Resolved::Ambiguous(b"odd".to_vec())
    );
    assert_eq!(
        config.lookups.last(),
        Some(&owned(&["-c", "alias.inner=stash -u"]))
    );
    assert_eq!(config.lists, 1);
}

#[test]
fn a_failed_question_ends_the_resolution() {
    let mut config = configured(&[]);
    config.failing = true;
    assert_eq!(resolve_with(&owned(&["log"]), &mut config), Err("lookup"));
}

#[test]
fn the_usage_error_names_the_alias_and_the_unguarded_route() {
    let same = ambiguous(b"log", b"log");
    assert!(same.starts_with(b"'log' is an alias"));
    assert!(same.ends_with(b"'git dupe git log' runs it as this Git reads it"));
    let reached = ambiguous(b"whatchanged", b"a");
    assert!(reached.starts_with(b"'a' leads to the alias 'whatchanged'"));
    assert!(reached.ends_with(b"'git dupe git a' runs it as this Git reads it"));
}
