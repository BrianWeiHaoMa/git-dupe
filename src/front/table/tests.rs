use super::*;

/// A table with `status`'s two value forms, as P1 gives them for `status`.
const TABLE: [Entry; 9] = [
    Entry::both(b'v', "verbose", Value::None),
    Entry::both(b's', "short", Value::None),
    Entry::both(b'b', "branch", Value::None),
    Entry::long("long", Value::None),
    Entry::long("porcelain", Value::Attached),
    Entry::both(b'u', "untracked-files", Value::Attached),
    Entry::long("ignored", Value::Attached),
    Entry::both(b'M', "find-renames", Value::Attached),
    Entry::long("no-renames", Value::None),
];

fn words(words: &[&[u8]]) -> Vec<OsString> {
    words
        .iter()
        .map(|word| OsStr::from_bytes(word).to_owned())
        .collect()
}

fn os(word: &[u8]) -> &OsStr {
    OsStr::from_bytes(word)
}

/// The long names of the entries the words spelled, in order.
fn spelled(given: &[&[u8]]) -> Vec<&'static [u8]> {
    match read(&TABLE, &words(given)) {
        Reading::Read(read) => read
            .options
            .iter()
            .flat_map(|option| &option.spelled)
            .map(|entry| entry.long.expect("every entry here has a long name"))
            .collect(),
        other => panic!("{given:?}: {other:?}"),
    }
}

#[test]
fn an_option_is_held_alone_bundled_and_with_its_value_attached() {
    let both: &[&[u8]] = &[b"short", b"branch"];
    assert_eq!(spelled(&[b"-sb"]), both);
    assert_eq!(spelled(&[b"-vv"]), [b"verbose", b"verbose"]);
    assert_eq!(spelled(&[b"--porcelain=v2"]), [b"porcelain"]);
    assert_eq!(spelled(&[b"--porcelain"]), [b"porcelain"]);
    assert_eq!(spelled(&[b"-uno"]), [&b"untracked-files"[..]]);
    let su: &[&[u8]] = &[b"short", b"untracked-files"];
    assert_eq!(spelled(&[b"-su"]), su);
    assert_eq!(spelled(&[b"-M50"]), [b"find-renames"]);
    assert_eq!(spelled(&[b"--find-renames=50"]), [b"find-renames"]);
    // The rest of a word after a letter with a value is that value, whatever it holds.
    assert_eq!(spelled(&[b"-uvh"]), [&b"untracked-files"[..]]);
    // An entry typed again is spelled again, in a later word as in a bundle.
    let thrice: &[&[u8]] = &[b"verbose", b"long", b"verbose"];
    assert_eq!(spelled(&[b"-v", b"--long", b"-v"]), thrice);
    assert_eq!(spelled(&[b"--no-renames"]), [b"no-renames"]);
    // The empty value as written.
    assert_eq!(spelled(&[b"--porcelain="]), [b"porcelain"]);
}

#[test]
fn options_and_operands_keep_their_order_and_their_spelling() {
    let given = words(&[b"-sb", b"notes", b"--porcelain=v2", b"docs/x", b"-uno"]);
    let Reading::Read(typed) = read(&TABLE, &given) else {
        panic!()
    };
    let options: Vec<_> = typed.options().collect();
    assert_eq!(options, [os(b"-sb"), os(b"--porcelain=v2"), os(b"-uno")]);
    assert_eq!(typed.operands, [os(b"notes"), os(b"docs/x")]);
    let untracked = &TABLE[5];
    assert!(typed.spells(untracked));
    assert!(!typed.spells(&TABLE[6]));
    // A value of `-u` taken as the next word is no value: `normal` is a path.
    for given in [
        &[&b"-u"[..], b"normal"][..],
        &[b"--untracked-files", b"normal"],
    ] {
        let given = words(given);
        let Reading::Read(separate) = read(&TABLE, &given) else {
            panic!()
        };
        assert!(separate.options().eq([os(given[0].as_bytes())]));
        assert_eq!(separate.operands, [os(b"normal")]);
        assert!(separate.spells(untracked));
    }
}

#[test]
fn the_end_of_options_makes_every_later_word_an_operand() {
    let given = words(&[b"-s", b"--", b"--porc", b"-h", b"--", b"x"]);
    let Reading::Read(typed) = read(&TABLE, &given) else {
        panic!()
    };
    assert!(typed.options().eq([os(b"-s")]));
    assert_eq!(
        typed.operands,
        [os(b"--porc"), os(b"-h"), os(b"--"), os(b"x")]
    );
    let alone = words(&[b"--"]);
    assert_eq!(
        read(&TABLE, &alone),
        Reading::Read(Read {
            options: Vec::new(),
            operands: Vec::new(),
            end_of_options: Some(0),
            lacking_a_value: None,
        })
    );
}

#[test]
fn a_word_the_table_does_not_hold_is_named() {
    for word in [
        &b"--porc"[..],
        b"--no-verbose",
        b"--end-of-options",
        b"--short=1",
        b"--verbose=",
        b"-sX",
        b"-",
        b"-vh",
        b"-x",
        b"--porcelian",
        b"---",
        b"--Porcelain",
        b"--help=x",
        b"-\xe9",
    ] {
        let given = words(&[b"notes", word, b"--porc"]);
        assert_eq!(
            read(&TABLE, &given),
            Reading::NotHeld(os(word)),
            "{}",
            word.escape_ascii()
        );
    }
}

#[test]
fn the_first_help_request_or_word_not_held_decides() {
    for (given, decided) in [
        (
            &[&b"--porc"[..], b"-h"][..],
            Reading::NotHeld(os(b"--porc")),
        ),
        (&[b"-h", b"--porc"], Reading::Help),
        (&[b"x", b"--help"], Reading::Help),
        (&[b"-s", b"--help", b"-x"], Reading::Help),
        (&[b"-x", b"--help"], Reading::NotHeld(os(b"-x"))),
    ] {
        assert_eq!(read(&TABLE, &words(given)), decided, "{given:?}");
    }
    let given = words(&[b"--", b"-h"]);
    assert!(matches!(read(&TABLE, &given), Reading::Read(_)));
}

#[test]
fn an_operand_that_is_no_literal_path_is_found_first_in_order() {
    let fault = |given: &[&[u8]]| {
        let given = words(given);
        match read(&TABLE, &given) {
            Reading::Read(read) => read.not_literal().map(|fault| fault.text(b"status")),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(
        fault(&[b"notes", b":/", b":/notes/x", b"-s", b"--", b"-x"]),
        None
    );
    let expected = Fault::Pattern(os(b"a*")).text(b"status");
    assert_eq!(fault(&[b"x", b"a*", b":y"]), Some(expected));
    let expected = Fault::Magic(os(b":(glob)x")).text(b"status");
    assert_eq!(fault(&[b"--", b":(glob)x", b"a*"]), Some(expected));
    assert!(fault(&[b":/a?"]).is_some());
}

/// A table with the third value form, long and short, beside `add`'s all-flag
/// entries.
const NEXT: [Entry; 8] = [
    Entry::both(b'n', "dry-run", Value::None),
    Entry::both(b'v', "verbose", Value::None),
    Entry::both(b'A', "all", Value::None),
    Entry::long("no-all", Value::None),
    Entry::long("no-ignore-removal", Value::None),
    Entry::long("chmod", Value::Next),
    Entry::both(b'e', "exclude", Value::Next),
    Entry::both(b'u', "untracked-files", Value::Attached),
];

fn read_next(given: &[&[u8]]) -> Read<'static> {
    let given: &'static [OsString] = Box::leak(words(given).into_boxed_slice());
    match read(&NEXT, given) {
        Reading::Read(read) => read,
        other => panic!("{other:?}"),
    }
}

fn all_os(words: &[&[u8]]) -> Vec<OsString> {
    words.iter().map(|word| os(word).to_owned()).collect()
}

#[test]
fn a_value_attached_or_the_next_word_is_taken_whatever_it_is() {
    for (given, options, operands) in [
        (
            &[&b"--chmod"[..], b"+x", b"x"][..],
            &[&b"--chmod"[..], b"+x"][..],
            &[&b"x"[..]][..],
        ),
        (&[b"--chmod=+x", b"x"], &[b"--chmod=+x"], &[b"x"]),
        (&[b"--chmod", b"--", b"x"], &[b"--chmod", b"--"], &[b"x"]),
        (&[b"--chmod", b"-h", b"x"], &[b"--chmod", b"-h"], &[b"x"]),
        (&[b"--chmod", b"--help"], &[b"--chmod", b"--help"], &[]),
        (&[b"--chmod", b"-A"], &[b"--chmod", b"-A"], &[]),
        // Typed last, it stands as written.
        (&[b"x", b"--chmod"], &[b"--chmod"], &[b"x"]),
        (&[b"-e", b"*.o", b"x"], &[b"-e", b"*.o"], &[b"x"]),
        (&[b"-e*.o"], &[b"-e*.o"], &[]),
        (&[b"-ne", b"--"], &[b"-ne", b"--"], &[]),
        (&[b"-nex", b"y"], &[b"-nex"], &[b"y"]),
    ] {
        let read = read_next(given);
        assert!(
            read.options().eq(options.iter().map(|word| os(word))),
            "{given:?}"
        );
        assert_eq!(
            read.operands,
            operands.iter().map(|word| os(word)).collect::<Vec<_>>()
        );
    }
    // A word taken as a value spells no entry and requests no help.
    let read = read_next(&[b"--chmod", b"-A"]);
    assert!(!read.spells(&NEXT[2]));
    assert!(read.spells(&NEXT[5]));
}

#[test]
fn the_last_spelling_of_a_flag_decides() {
    let flag = [&NEXT[2], &NEXT[3], &NEXT[4]];
    let last = |given: &[&[u8]]| read_next(given).last_of(&flag).and_then(|entry| entry.long);
    assert_eq!(last(&[b"-A", b"--no-all", b"-A"]), Some(&b"all"[..]));
    assert_eq!(last(&[b"-A", b"--no-all"]), Some(&b"no-all"[..]));
    assert_eq!(last(&[b"--no-all", b"-nA"]), Some(&b"all"[..]));
    assert_eq!(
        last(&[b"-An", b"--no-ignore-removal", b"-v"]),
        Some(&b"no-ignore-removal"[..])
    );
    assert_eq!(last(&[b"-nv", b"--chmod", b"-A"]), None);
}

#[test]
fn the_words_without_some_entries_keep_every_other_word_as_typed() {
    let flag = [&NEXT[2], &NEXT[3], &NEXT[4]];
    let without = |given: &[&[u8]]| read_next(given).options_without(&flag);
    assert_eq!(without(&[b"-A", b"--all", b"x"]), all_os(&[]));
    assert_eq!(without(&[b"-Av"]), all_os(&[b"-v"]));
    assert_eq!(without(&[b"-vAn"]), all_os(&[b"-vn"]));
    assert_eq!(without(&[b"-nA", b"--no-ignore-removal"]), all_os(&[b"-n"]));
    assert_eq!(without(&[b"-AA", b"--no-all", b"-n"]), all_os(&[b"-n"]));
    assert_eq!(
        without(&[b"--chmod", b"+x", b"-A", b"--chmod=-x", b"-ne", b"-A"]),
        all_os(&[b"--chmod", b"+x", b"--chmod=-x", b"-ne", b"-A"])
    );
    // A letter taken out with the value it holds, and a value kept with its letter.
    let untracked = [&NEXT[7]];
    let read = read_next(&[b"-nuno", b"-vu"]);
    assert_eq!(read.options_without(&untracked), all_os(&[b"-n", b"-v"]));
    let read = read_next(&[b"-nuno", b"-ve", b"x"]);
    assert_eq!(
        read.options_without(&[&NEXT[0]]),
        all_os(&[b"-uno", b"-ve", b"x"])
    );
    assert_eq!(
        read.options_without(&[&NEXT[6]]),
        all_os(&[b"-nuno", b"-v"])
    );
}

/// A table with an entry that has a letter and no long name, beside one that takes
/// the next word.
const LETTERS: [Entry; 3] = [
    Entry::short(b'd', Value::None),
    Entry::both(b'f', "force", Value::None),
    Entry::both(b'e', "exclude", Value::Next),
];

#[test]
fn a_letter_without_a_long_name_is_spelled_by_no_long_word() {
    let given = words(&[b"-fd", b"-d"]);
    let Reading::Read(typed) = read(&LETTERS, &given) else {
        panic!()
    };
    assert!(typed.spells(&LETTERS[0]));
    for word in [&b"--d"[..], b"--=x", b"--=", b"--D"] {
        let given = words(&[word]);
        assert_eq!(read(&LETTERS, &given), Reading::NotHeld(os(word)));
    }
}

#[test]
fn the_end_of_options_is_kept_where_it_stood_and_a_value_is_none() {
    let position = |given: &[&[u8]]| {
        let given = words(given);
        match read(&LETTERS, &given) {
            Reading::Read(read) => read.end_of_options(),
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(position(&[b"-f", b"x", b"--", b"--", b"y"]), Some(2));
    // A `--` taken as a value ends nothing; the next one does.
    assert_eq!(position(&[b"-e", b"--", b"--", b"y"]), Some(2));
    assert_eq!(position(&[b"--exclude", b"--"]), None);
    assert_eq!(position(&[b"-f", b"x"]), None);
}

#[test]
fn an_option_typed_last_without_the_value_it_takes_is_known() {
    let lacking = |given: &[&[u8]]| {
        let given = words(given);
        match read(&LETTERS, &given) {
            Reading::Read(read) => read.lacking_a_value(),
            other => panic!("{other:?}"),
        }
    };
    let exclude = Some(&LETTERS[2]);
    assert_eq!(lacking(&[b"-f", b"-e"]), exclude);
    assert_eq!(lacking(&[b"-fde"]), exclude);
    assert_eq!(lacking(&[b"x", b"--exclude"]), exclude);
    assert_eq!(lacking(&[b"-e", b"x"]), None);
    assert_eq!(lacking(&[b"-ex"]), None);
    assert_eq!(lacking(&[b"--exclude="]), None);
    assert_eq!(lacking(&[b"-e", b"--"]), None);
    assert_eq!(lacking(&[b"--", b"-e"]), None);
    assert_eq!(lacking(&[b"-e", b"x", b"-f"]), None);
}

#[test]
fn a_fault_names_the_word_on_one_line_and_the_unguarded_route() {
    let word = os(b"caf\xe9 *");
    for fault in [
        Fault::NotHeld(word),
        Fault::Pattern(word),
        Fault::Magic(word),
        Fault::Outside(word),
    ] {
        let text = fault.text(b"status");
        assert!(text.starts_with(b"'caf\xe9 *' "), "{fault:?}");
        assert!(!text.contains(&b'\n'), "{fault:?}");
        let unguarded = text.windows(19).any(|part| part == b"git dupe git status");
        assert_eq!(unguarded, !matches!(fault, Fault::Outside(_)), "{fault:?}");
    }
    // A word holding a newline is still named, on one line, in Git's quoting.
    let text = Fault::NotHeld(os(b"--a\nb\t\"\\\x1b\xe9")).text(b"add");
    assert!(
        text.starts_with(b"\"--a\\nb\\t\\\"\\\\\\033\xe9\" "),
        "{}",
        text.escape_ascii()
    );
    let text = Fault::Pattern(os(b"a\n*")).text(b"add");
    assert!(text.starts_with(b"\"a\\n*\" "), "{}", text.escape_ascii());
    assert!(!text.contains(&b'\n'));
}
