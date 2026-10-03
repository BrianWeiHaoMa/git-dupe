//! What `add` stages (G13, G14, `Holds/G13, G14`): the scope of its operands, the skipped
//! set, the refusal of a publicly tracked file operand, and which pathspecs each of its
//! one or two runs carries. Decided over the listings, the `lstat` facts, and the ignore
//! answers the handler took, by the ancestor test alone (R9): nothing is matched and
//! nothing is asked of Git here.
//!
//! What the code below cannot show:
//!
//! - A path that a form or the all-flag brought into the scope is the private
//!   repository's to stage through Git's own walk, and Git refuses a whole run over a
//!   pathspec beyond a symbolic link, one that matches nothing, and, without `-f`, one
//!   that names an ignored path (S4): such a pathspec is dropped, and an ignored path
//!   with a privately tracked file at or below it is staged by a second run with `-u`.
//! - A literal operand is the user's path: none of that applies to it, and Git decides
//!   about it, its refusals and its status included.

use std::collections::BTreeSet;

use super::operand;
use super::scope;

/// An operand of `add`, after locating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    /// The directory form, `.` or `./`, at the user's directory; the root form, `:/` or
    /// an operand resolving to the root, at the root, the empty path. It acts on the
    /// hidden paths under that path.
    Form(Vec<u8>),
    /// Any other operand: its root-relative path, taken as given.
    Literal(Vec<u8>),
}

/// What `add`'s words come to.
#[derive(Debug, PartialEq, Eq)]
pub enum Scoped {
    /// No operand and the all-flag clear: the run is the user's words unchanged.
    Unchanged,
    /// Only forms, and nothing hidden but `.gitdupe` lies under them: no run, a `hint:`.
    NothingHidden,
    Scope(Scope),
}

/// The scope of a run, each path once.
#[derive(Debug, PartialEq, Eq)]
pub struct Scope {
    /// The literal operands, in the order given.
    pub literals: Vec<Vec<u8>>,
    /// The paths the forms or the all-flag brought in that are no literal operand, in
    /// byte order.
    pub forms: Vec<Vec<u8>>,
}

impl Scope {
    /// Every path of the scope: the literal operands, then the forms' paths.
    pub fn paths(&self) -> impl Iterator<Item = &[u8]> {
        self.literals.iter().chain(&self.forms).map(Vec::as_slice)
    }
}

/// The scope of `operands`, given the region paths and every hidden path: for a form, the
/// hidden paths under its path (`scope::hidden_under`); with no operand and the all-flag
/// set, every region path. The forms find nothing hidden but `.gitdupe` when they bring in
/// nothing else and no hidden path lies below `.gitdupe`, as a privately tracked file does
/// below a directory standing there.
pub fn scope(
    region: &[Vec<u8>],
    hidden: &BTreeSet<Vec<u8>>,
    gitdupe: &[u8],
    operands: &[Operand],
    all: bool,
) -> Scoped {
    if operands.is_empty() && !all {
        return Scoped::Unchanged;
    }
    let mut literals: Vec<Vec<u8>> = Vec::new();
    let mut forms = BTreeSet::new();
    if operands.is_empty() {
        forms.extend(region.iter().cloned());
    }
    for operand in operands {
        match operand {
            Operand::Form(path) => forms.extend(scope::hidden_under(region, path)),
            Operand::Literal(path) if !literals.contains(path) => literals.push(path.clone()),
            Operand::Literal(_) => {}
        }
    }
    let only_gitdupe = forms.iter().all(|path| path == gitdupe)
        && !(forms.contains(gitdupe) && operand::any_below(hidden, gitdupe));
    if literals.is_empty() && only_gitdupe {
        return Scoped::NothingHidden;
    }
    for literal in &literals {
        forms.remove(literal);
    }
    Scoped::Scope(Scope {
        literals,
        forms: forms.into_iter().collect(),
    })
}

/// The skipped set: each publicly tracked path at or below a scope path that the private
/// repository does not track, in byte order. A path tracked by both is staged as any
/// privately tracked file (G8, G14).
pub fn skipped(
    scope: &Scope,
    publicly_tracked: &BTreeSet<Vec<u8>>,
    privately_tracked: &BTreeSet<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let scope: BTreeSet<&[u8]> = scope.paths().collect();
    publicly_tracked
        .iter()
        .filter(|path| {
            scope.contains(path.as_slice())
                || operand::ancestors(path).any(|above| scope.contains(above))
        })
        .filter(|path| !privately_tracked.contains(*path))
        .cloned()
        .collect()
}

/// The first literal operand that is itself a skipped path: `add` refuses it (G14).
pub fn refused<'s>(scope: &'s Scope, skipped: &[Vec<u8>]) -> Option<&'s [u8]> {
    scope
        .literals
        .iter()
        .find(|literal| skipped.binary_search(literal).is_ok())
        .map(Vec::as_slice)
}

/// What the handler found by `lstat`, without following a symbolic link at the path.
#[derive(Debug, Default)]
pub struct Found {
    /// Paths with a symbolic link among their ancestors.
    pub beyond_a_link: BTreeSet<Vec<u8>>,
    /// Paths at which nothing stands.
    pub absent: BTreeSet<Vec<u8>>,
}

/// The pathspecs of the runs, as far as each step has decided them.
#[derive(Debug, PartialEq, Eq)]
pub struct Pathspecs {
    /// The scope paths the first run carries: the literal operands, then the forms' paths
    /// that keep their pathspec.
    pub first: Vec<Vec<u8>>,
    /// The forms' paths staged by the second run, with `-u`.
    pub second: Vec<Vec<u8>>,
    /// The skipped paths that keep their exclusion, in both runs.
    pub exclusions: Vec<Vec<u8>>,
}

impl Pathspecs {
    /// The pathspecs a form's path and an exclusion keep before the ignore question: a
    /// form's path loses its own beyond a symbolic link, and where no privately tracked
    /// file lies at or below it while nothing stands at it or `updating` (`-u`,
    /// `--update`, `--refresh`); an exclusion is lost beyond a symbolic link.
    pub fn kept(
        scope: &Scope,
        skipped: &[Vec<u8>],
        privately_tracked: &BTreeSet<Vec<u8>>,
        found: &Found,
        updating: bool,
    ) -> Pathspecs {
        let forms = scope.forms.iter().filter(|path| {
            !found.beyond_a_link.contains(*path)
                && (operand::any_at_or_below(privately_tracked, path)
                    || !(updating || found.absent.contains(*path)))
        });
        Pathspecs {
            first: scope.literals.iter().chain(forms).cloned().collect(),
            second: Vec::new(),
            exclusions: skipped
                .iter()
                .filter(|path| !found.beyond_a_link.contains(*path))
                .cloned()
                .collect(),
        }
    }

    /// The paths to ask the ignore question about: every form's path and every exclusion
    /// that still has its pathspec. Never a literal operand.
    pub fn asked<'p>(&'p self, scope: &Scope) -> Vec<&'p [u8]> {
        self.first
            .iter()
            .filter(|path| scope.forms.binary_search(path).is_ok())
            .chain(&self.exclusions)
            .map(Vec::as_slice)
            .collect()
    }

    /// The pathspecs once the question has named the `ignored` paths: each loses its
    /// pathspec, and a form's path with a privately tracked file at or below it moves to
    /// the second run.
    pub fn answered(
        self,
        scope: &Scope,
        ignored: &BTreeSet<Vec<u8>>,
        privately_tracked: &BTreeSet<Vec<u8>>,
    ) -> Pathspecs {
        let mut first = Vec::new();
        let mut second = Vec::new();
        for path in self.first {
            let form = scope.forms.binary_search(&path).is_ok();
            if !form || !ignored.contains(&path) {
                first.push(path);
            } else if operand::any_at_or_below(privately_tracked, &path) {
                second.push(path);
            }
        }
        let exclusions = self
            .exclusions
            .into_iter()
            .filter(|path| !ignored.contains(path))
            .collect();
        Pathspecs {
            first,
            second,
            exclusions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(paths: &[&[u8]]) -> Vec<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn set(paths: &[&[u8]]) -> BTreeSet<Vec<u8>> {
        paths.iter().map(|path| path.to_vec()).collect()
    }

    fn form(path: &[u8]) -> Operand {
        Operand::Form(path.to_vec())
    }

    fn literal(path: &[u8]) -> Operand {
        Operand::Literal(path.to_vec())
    }

    const GITDUPE: &[u8] = b".gitdupe";

    const REGION: [&[u8]; 5] = [
        b".env.local",
        b".gitdupe",
        b".vscode",
        b"docs/notes.md",
        b"notes",
    ];

    fn scoped(operands: &[Operand], all: bool) -> Scoped {
        scope(&paths(&REGION), &set(&REGION), GITDUPE, operands, all)
    }

    /// The region `.gitdupe` alone, with `hidden` the hidden paths.
    fn bare(hidden: &[&[u8]], operands: &[Operand], all: bool) -> Scoped {
        scope(&paths(&[GITDUPE]), &set(hidden), GITDUPE, operands, all)
    }

    fn of(forms: &[&[u8]], literals: &[&[u8]]) -> Scoped {
        Scoped::Scope(Scope {
            literals: paths(literals),
            forms: paths(forms),
        })
    }

    #[test]
    fn a_form_acts_on_the_hidden_paths_under_its_path_and_the_root_on_all_of_them() {
        // The root form, `.` at the root, and the all-flag without an operand.
        assert_eq!(scoped(&[form(b"")], false), of(&REGION, &[]));
        assert_eq!(scoped(&[], true), of(&REGION, &[]));
        // `.` from a directory holding hidden paths, below a hidden path, and beside one.
        assert_eq!(
            scoped(&[form(b"docs")], false),
            of(&[b"docs/notes.md"], &[])
        );
        assert_eq!(
            scoped(&[form(b"notes/sub")], false),
            of(&[b"notes/sub"], &[])
        );
        assert_eq!(scoped(&[form(b"notes")], false), of(&[b"notes"], &[]));
        // Each path once over several forms.
        assert_eq!(
            scoped(&[form(b"notes"), form(b""), form(b"notes")], false),
            of(&REGION, &[])
        );
    }

    #[test]
    fn with_an_operand_the_all_flag_brings_nothing_in() {
        assert_eq!(scoped(&[literal(b"scratch")], true), of(&[], &[b"scratch"]));
        assert_eq!(scoped(&[form(b"src")], true), Scoped::NothingHidden);
        assert_eq!(scoped(&[form(b"notes")], true), of(&[b"notes"], &[]));
    }

    #[test]
    fn without_an_operand_or_the_all_flag_the_words_are_unchanged() {
        assert_eq!(scoped(&[], false), Scoped::Unchanged);
        assert_eq!(bare(&[GITDUPE], &[], false), Scoped::Unchanged);
    }

    #[test]
    fn forms_with_nothing_hidden_but_gitdupe_under_them_run_nothing() {
        // An empty scope, from a directory with nothing hidden.
        assert_eq!(scoped(&[form(b"src")], false), Scoped::NothingHidden);
        // `.gitdupe` alone.
        for operands in [&[form(b"")][..], &[], &[form(b""), form(b"src")]] {
            assert_eq!(bare(&[GITDUPE], operands, true), Scoped::NothingHidden);
        }
        // A literal operand beside a form is always a run.
        assert_eq!(
            bare(&[GITDUPE], &[form(b""), literal(b"x")], false),
            Scoped::Scope(Scope {
                literals: paths(&[b"x"]),
                forms: paths(&[GITDUPE]),
            })
        );
        // A file privately tracked below a directory standing at `.gitdupe` is hidden,
        // and the forms that reach `.gitdupe` stage it; one that does not finds nothing.
        let below: &[&[u8]] = &[GITDUPE, b".gitdupe/x"];
        assert_eq!(
            bare(below, &[form(b"")], false),
            Scoped::Scope(Scope {
                literals: Vec::new(),
                forms: paths(&[GITDUPE]),
            })
        );
        assert_eq!(bare(below, &[], true), bare(below, &[form(b"")], false));
        assert_eq!(bare(below, &[form(b"src")], false), Scoped::NothingHidden);
    }

    #[test]
    fn a_path_both_brought_in_by_a_form_and_named_is_a_literal_operand() {
        assert_eq!(
            scoped(
                &[
                    form(b""),
                    literal(b"notes"),
                    literal(b"x"),
                    literal(b"notes")
                ],
                false
            ),
            of(
                &[b".env.local", b".gitdupe", b".vscode", b"docs/notes.md"],
                &[b"notes", b"x"]
            )
        );
    }

    fn scope_of(literals: &[&[u8]], forms: &[&[u8]]) -> Scope {
        Scope {
            literals: paths(literals),
            forms: paths(forms),
        }
    }

    #[test]
    fn the_skipped_set_is_what_the_project_tracks_under_the_scope_and_the_private_one_does_not() {
        let scope = scope_of(&[b"README.md", b"conf"], &[b".gitdupe", b"notes"]);
        let publicly_tracked = set(&[
            b".gitdupe",
            b"README.md",
            b"conf/local.ini",
            b"docs/design.md",
            b"notes/both.md",
            b"notes/shared.md",
            b"notes-old/x",
        ]);
        let privately_tracked = set(&[b"notes/both.md", b"notes/a.md"]);
        let skipped = skipped(&scope, &publicly_tracked, &privately_tracked);
        assert_eq!(
            skipped,
            paths(&[
                b".gitdupe",
                b"README.md",
                b"conf/local.ini",
                b"notes/shared.md"
            ])
        );
        // Only a literal operand that is itself a skipped path is refused.
        assert_eq!(refused(&scope, &skipped), Some(&b"README.md"[..]));
        let directory = scope_of(&[b"conf", b"notes/shared.md"], &[]);
        assert_eq!(refused(&directory, &skipped), Some(&b"notes/shared.md"[..]));
        assert_eq!(refused(&scope_of(&[b"conf"], &[b"notes"]), &skipped), None);
        // A form's path is skipped, not refused.
        assert_eq!(refused(&scope_of(&[], &[b".gitdupe"]), &skipped), None);
    }

    #[test]
    fn a_forms_path_loses_its_pathspec_beyond_a_link_or_where_nothing_private_is_left_to_add() {
        let scope = scope_of(
            &[b"lit", b"link/lit", b"gone-literal"],
            &[b"gone", b"held", b"link/x", b"new", b"notes", b"scratch"],
        );
        let privately_tracked = set(&[b"gone/a.md", b"held/a.md", b"link/x/a.md"]);
        let found = Found {
            beyond_a_link: set(&[b"link/x", b"link/lit", b"link/skipped"]),
            absent: set(&[b"gone", b"new", b"gone-literal"]),
        };
        let skipped = paths(&[b"link/skipped", b"notes/shared.md"]);
        let kept = Pathspecs::kept(&scope, &skipped, &privately_tracked, &found, false);
        // `new` has nothing at it and nothing private below; `gone` still holds a
        // privately tracked file whose deletion Git stages; a literal operand is kept
        // whatever `lstat` found.
        assert_eq!(
            kept.first,
            paths(&[
                b"lit",
                b"link/lit",
                b"gone-literal",
                b"gone",
                b"held",
                b"notes",
                b"scratch"
            ])
        );
        assert_eq!(kept.exclusions, paths(&[b"notes/shared.md"]));
        // Under `-u`, `--update`, or `--refresh`, a path with nothing private at or below
        // it is dropped even where something stands.
        let updating = Pathspecs::kept(&scope, &skipped, &privately_tracked, &found, true);
        assert_eq!(
            updating.first,
            paths(&[b"lit", b"link/lit", b"gone-literal", b"gone", b"held"])
        );
    }

    #[test]
    fn the_question_asks_about_kept_forms_and_exclusions_and_moves_private_ones_to_a_second_run() {
        let scope = scope_of(
            &[b".vscode", b"x"],
            &[b".env.local", b".gitdupe", b"build", b"notes"],
        );
        let privately_tracked = set(&[b".env.local", b".gitdupe", b".vscode/settings.json"]);
        let skipped = paths(&[b"notes/shared.log", b"notes/shared.md"]);
        let found = Found::default();
        let kept = Pathspecs::kept(&scope, &skipped, &privately_tracked, &found, false);
        // A literal operand is never asked about, ignored or not.
        assert_eq!(
            kept.asked(&scope),
            [
                &b".env.local"[..],
                b".gitdupe",
                b"build",
                b"notes",
                b"notes/shared.log",
                b"notes/shared.md",
            ]
        );
        let ignored = set(&[b".env.local", b".vscode", b"build", b"notes/shared.log"]);
        let answered = kept.answered(&scope, &ignored, &privately_tracked);
        assert_eq!(
            answered,
            Pathspecs {
                // The literal `.vscode` stays, ignored or not: Git decides.
                first: paths(&[b".vscode", b"x", b".gitdupe", b"notes"]),
                // An ignored form's path holding private files goes to the second run;
                // one holding none, `build`, to neither.
                second: paths(&[b".env.local"]),
                exclusions: paths(&[b"notes/shared.md"]),
            }
        );
    }
}
