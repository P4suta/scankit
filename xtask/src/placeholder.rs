// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `placeholder` gate.
//!
//! Rejects `todo!`, `unimplemented!`, `#[allow(` and `#[expect(` in the sources of the
//! product crates. The hole it closes is measured rather than theoretical: a function
//! body of `todo!()` draws no diagnostic from rustc or clippy under this workspace's
//! configuration, so an unwritten answer compiles, formats, lints and ships. Every
//! other gate here asks whether the code says the right thing; this one asks whether it
//! says anything at all.
//!
//! The second half is the first non-negotiable rule in `CONTRIBUTING.md` — no `allow`
//! and no `ignore` — which would otherwise be held by review alone. A suppression is
//! caught wherever the attribute path is written, `cfg_attr` included, because what
//! matters is the silenced finding rather than its spelling.
//!
//! Comments and literals are blanked before the scan, so prose naming a forbidden
//! construct is not one and a string holding `"todo!()"` is data. Blanking keeps the
//! line breaks, so a finding names the line it is written on.
//!
//! See `CONTRIBUTING.md`.

use std::fs;
use std::io;

use crate::shared::{self, Gate};

/// The `placeholder` gate, as the dispatcher sees it.
pub(crate) const GATE: Gate = Gate {
    name: "placeholder",
    purpose: concat!(
        "the product crates contain no unwritten body ",
        "and no suppressed lint"
    ),
    reference: "CONTRIBUTING.md",
    run,
};

/// Scan every product source and gather the findings.
///
/// The census is printed rather than returned because a gate reports what it examined,
/// not only what it objected to: a scan that reached no files must not be readable as a
/// scan that found nothing wrong.
fn run() -> io::Result<Vec<String>> {
    let survey = survey()?;
    println!(
        "{name}: examined {files} source file(s) in {crates} product crate(s)",
        name = GATE.name,
        files = survey.files,
        crates = survey.crates
    );
    Ok(survey.violations)
}

/// Why a deferred body may not appear in the product crates.
const DEFERRED: &str = "a rule is written or it is absent, never deferred; rustc and clippy \
                        report nothing for an unwritten body, so it passes every other gate \
                        here";

/// Why a suppressed lint may not appear in the product crates.
const SUPPRESSED: &str = "every gate is strict on purpose; make the code pass instead of \
                          silencing the finding, or change the shared configuration and say \
                          why (CONTRIBUTING.md)";

/// One construct the product crates may not contain.
///
/// The same shape serves both halves of the gate: a macro name looked for ahead of a
/// delimiter, and an attribute path looked for inside an attribute.
#[derive(Debug)]
struct Forbidden {
    /// The identifier to look for, matched as a whole token.
    token: &'static str,
    /// How the report names the construct, as a noun phrase.
    construct: &'static str,
    /// Why the product crates may not contain it.
    because: &'static str,
}

/// The macros that stand in for an answer that was never written.
const PLACEHOLDERS: &[Forbidden] = &[
    Forbidden {
        token: "todo",
        construct: "a `todo!` body",
        because: DEFERRED,
    },
    Forbidden {
        token: "unimplemented",
        construct: "an `unimplemented!` body",
        because: DEFERRED,
    },
];

/// The attribute paths that turn a finding off.
const SUPPRESSIONS: &[Forbidden] = &[
    Forbidden {
        token: "allow",
        construct: "an `allow` attribute",
        because: SUPPRESSED,
    },
    Forbidden {
        token: "expect",
        construct: "an `expect` attribute",
        because: SUPPRESSED,
    },
];

/// One forbidden construct, located in the source it was written in.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    /// The one-based line the construct is written on.
    line: usize,
    /// How the report names the construct.
    construct: &'static str,
    /// Why the product crates may not contain it.
    because: &'static str,
}

/// What one run of the gate looked at and what it objected to.
#[derive(Debug)]
struct Survey {
    /// How many product crates were read.
    crates: usize,
    /// How many source files were read.
    files: usize,
    /// One message per violation, in the order the sources were read.
    violations: Vec<String>,
}

/// Read every product source and record both the extent of the scan and its findings.
///
/// Build output is skipped: a crate-local `target` directory holds generated sources
/// that nobody can fix, and a finding a contributor cannot act on is worse than no
/// finding.
fn survey() -> io::Result<Survey> {
    let product = shared::product_crates()?;
    let mut survey = Survey {
        crates: product.len(),
        files: 0,
        violations: Vec::new(),
    };
    for each in &product {
        let build_output = each.directory.join("target");
        for source in shared::rust_sources(&each.directory)? {
            if source.starts_with(&build_output) {
                continue;
            }
            survey.files = survey.files.saturating_add(1);
            let name = shared::relative_name(&source, &each.directory);
            for finding in findings(&fs::read_to_string(&source)?) {
                survey.violations.push(message(&each.name, &name, &finding));
            }
        }
    }
    Ok(survey)
}

/// Word one finding as the line the gate prints.
///
/// Written once, and here rather than inline, so that the sentence a contributor reads
/// is a thing this module tests rather than a thing it happens to produce.
fn message(crate_name: &str, source: &str, finding: &Finding) -> String {
    format!(
        "{crate_name}: {source}:{line} has {construct}; {because}",
        line = finding.line,
        construct = finding.construct,
        because = finding.because
    )
}

/// Every forbidden construct in one source file, in line order.
fn findings(source: &str) -> Vec<Finding> {
    let code = without_prose(source);
    let mut found = placeholder_findings(&code);
    found.extend(suppression_findings(&code));
    found.sort_by_key(|finding| finding.line);
    found
}

/// Find the placeholder macros: a whole-token name, `!`, and an opening delimiter.
///
/// The delimiter is what separates `todo!()` from a variable compared with `todo != 0`,
/// and requiring the `!` to touch the name is what separates it from a variable named
/// `todo`.
fn placeholder_findings(code: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    for forbidden in PLACEHOLDERS {
        for offset in token_occurrences(code, forbidden.token) {
            let after = code
                .get(offset.saturating_add(forbidden.token.len())..)
                .unwrap_or_default();
            if is_macro_invocation(after) {
                found.push(Finding {
                    line: line_of(code, offset),
                    construct: forbidden.construct,
                    because: forbidden.because,
                });
            }
        }
    }
    found
}

/// Find the suppression attributes, at any depth inside an attribute.
///
/// Searching the whole body of the attribute rather than only its head is what makes
/// `#[cfg_attr(test, allow(dead_code))]` a finding, and confining the search to an
/// attribute body is what keeps `value.expect("present")` from being one.
fn suppression_findings(code: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    for (hash, _) in code.match_indices('#') {
        let Some(open) = attribute_body_start(code, hash) else {
            continue;
        };
        let Some(close) = matching_bracket(code, open) else {
            continue;
        };
        let start = open.saturating_add(1);
        let body = code.get(start..close).unwrap_or_default();
        for forbidden in SUPPRESSIONS {
            for offset in token_occurrences(body, forbidden.token) {
                let after = body
                    .get(offset.saturating_add(forbidden.token.len())..)
                    .unwrap_or_default();
                if after.trim_start().starts_with('(') {
                    found.push(Finding {
                        line: line_of(code, start.saturating_add(offset)),
                        construct: forbidden.construct,
                        because: forbidden.because,
                    });
                }
            }
        }
    }
    found
}

/// The byte offset of every occurrence of `needle` in `code` that is a whole token.
///
/// A token boundary on both sides, so `todo_later` and `retodo` are their own names,
/// while `core::todo` still names the macro.
fn token_occurrences(code: &str, needle: &str) -> Vec<usize> {
    code.match_indices(needle)
        .filter(|(offset, _)| {
            let before = code.get(..*offset).unwrap_or_default();
            let after = code
                .get(offset.saturating_add(needle.len())..)
                .unwrap_or_default();
            !before.chars().next_back().is_some_and(is_identifier_char)
                && !after.chars().next().is_some_and(is_identifier_char)
        })
        .map(|(offset, _)| offset)
        .collect()
}

/// Whether the text following an identifier turns it into a macro invocation.
fn is_macro_invocation(after: &str) -> bool {
    let Some(rest) = after.strip_prefix('!') else {
        return false;
    };
    rest.trim_start().starts_with(['(', '[', '{'])
}

/// The offset of the `[` opening an attribute that starts at the `#` at `hash`.
///
/// Accepts the inner form `#![..]` as well as the outer one, because an inner attribute
/// at the crate root is the widest suppression there is.
fn attribute_body_start(code: &str, hash: usize) -> Option<usize> {
    let after = code.get(hash.saturating_add(1)..)?.trim_start();
    let after = after.strip_prefix('!').unwrap_or(after).trim_start();
    if !after.starts_with('[') {
        return None;
    }
    Some(code.len().saturating_sub(after.len()))
}

/// The offset of the `]` closing the `[` at `open`.
fn matching_bracket(code: &str, open: usize) -> Option<usize> {
    let mut depth: usize = 0;
    for (offset, character) in code.get(open..)?.char_indices() {
        match character {
            '[' => depth = depth.saturating_add(1),
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open.saturating_add(offset));
                }
            },
            _ => {},
        }
    }
    None
}

/// The one-based line a byte offset falls on.
fn line_of(code: &str, offset: usize) -> usize {
    code.get(..offset)
        .unwrap_or(code)
        .matches('\n')
        .count()
        .saturating_add(1)
}

/// Whether a character can appear inside an identifier.
fn is_identifier_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The source with every comment, string literal and character literal blanked out.
///
/// Blanked rather than removed, and with the line breaks kept, so that an offset in the
/// result falls on the line it falls on in the file and two tokens either side of a
/// stripped comment do not join into a third.
///
/// This is stricter than `shared::code_only`, which strips `//` comments only: here an
/// unstripped literal would *invent* a violation rather than hide one, so the stricter
/// scan is written locally.
fn without_prose(source: &str) -> String {
    let characters: Vec<char> = source.chars().collect();
    let mut kept = String::with_capacity(source.len());
    let mut index = 0;
    while let Some(&character) = characters.get(index) {
        let prose = match character {
            '/' if at(&characters, index, 1) == Some('/') => Some(line_comment(&characters, index)),
            '/' if at(&characters, index, 1) == Some('*') => {
                Some(block_comment(&characters, index))
            },
            '"' => Some(string_literal(&characters, index)),
            '\'' => character_literal(&characters, index),
            'r' => raw_string(&characters, index),
            _ => None,
        };
        if let Some(end) = prose {
            // Clamped so that a malformed literal running past the end of the file ends
            // the scan rather than repeating it forever.
            let end = end.clamp(index.saturating_add(1), characters.len());
            blank(&characters, index, end, &mut kept);
            index = end;
        } else {
            kept.push(character);
            index = index.saturating_add(1);
        }
    }
    kept
}

/// Copy the characters in `[start, end)` into `kept` as spaces, keeping the line breaks.
fn blank(characters: &[char], start: usize, end: usize, kept: &mut String) {
    for character in characters.get(start..end).unwrap_or_default() {
        kept.push(if *character == '\n' { '\n' } else { ' ' });
    }
}

/// The character `offset` positions past `index`, if the source has one.
fn at(characters: &[char], index: usize, offset: usize) -> Option<char> {
    characters.get(index.saturating_add(offset)).copied()
}

/// The end of a `//` comment: the line break, which is code and stays.
fn line_comment(characters: &[char], index: usize) -> usize {
    let mut end = index;
    while let Some(&character) = characters.get(end) {
        if character == '\n' {
            break;
        }
        end = end.saturating_add(1);
    }
    end
}

/// The end of a `/* */` comment, which Rust allows to nest.
fn block_comment(characters: &[char], index: usize) -> usize {
    let mut end = index.saturating_add(2);
    let mut depth: usize = 1;
    while depth > 0 {
        let Some(&character) = characters.get(end) else {
            return end;
        };
        match (character, at(characters, end, 1)) {
            ('/', Some('*')) => {
                depth = depth.saturating_add(1);
                end = end.saturating_add(2);
            },
            ('*', Some('/')) => {
                depth = depth.saturating_sub(1);
                end = end.saturating_add(2);
            },
            _ => end = end.saturating_add(1),
        }
    }
    end
}

/// The end of a `"` string literal, respecting backslash escapes.
fn string_literal(characters: &[char], index: usize) -> usize {
    let mut end = index.saturating_add(1);
    while let Some(&character) = characters.get(end) {
        match character {
            '\\' => end = end.saturating_add(2),
            '"' => return end.saturating_add(1),
            _ => end = end.saturating_add(1),
        }
    }
    end
}

/// How far past the quote an escaped character literal can close.
///
/// `'\u{10FFFF}'` is the longest one Rust admits, and its closing quote is eleven
/// characters past the opening one. Bounding the search is what keeps a lifetime
/// followed much later by a quote from swallowing the code between them.
const ESCAPE_LIMIT: usize = 11;

/// The end of a `'` character literal, or `None` when the quote opens a lifetime.
///
/// The distinction earns its place: `'"'` is a character literal holding a quotation
/// mark, and reading it as the start of a string would blank the rest of the file.
///
/// The search for the closing quote of an escaped literal starts three past the opening
/// one, because the shortest escape occupies two characters and the quote in `'\''` is
/// the escaped one rather than the terminator.
fn character_literal(characters: &[char], index: usize) -> Option<usize> {
    if at(characters, index, 1) == Some('\\') {
        for step in 3..=ESCAPE_LIMIT {
            if at(characters, index, step) == Some('\'') {
                return Some(index.saturating_add(step).saturating_add(1));
            }
        }
        return None;
    }
    if at(characters, index, 2) == Some('\'') {
        return Some(index.saturating_add(3));
    }
    None
}

/// The end of a raw string literal starting at `r`, or `None` when `r` is just a letter.
fn raw_string(characters: &[char], index: usize) -> Option<usize> {
    if !opens_raw_string(characters, index) {
        return None;
    }
    let mut hashes: usize = 0;
    while at(characters, index, hashes.saturating_add(1)) == Some('#') {
        hashes = hashes.saturating_add(1);
    }
    if at(characters, index, hashes.saturating_add(1)) != Some('"') {
        return None;
    }
    let mut end = index.saturating_add(hashes).saturating_add(2);
    while let Some(&character) = characters.get(end) {
        if character == '"' && closes_raw_string(characters, end.saturating_add(1), hashes) {
            return Some(end.saturating_add(1).saturating_add(hashes));
        }
        end = end.saturating_add(1);
    }
    Some(end)
}

/// Whether the `r` at `index` begins a raw string rather than continuing an identifier.
///
/// A single `b` or `c` before it is the byte-string and C-string prefix, not a letter.
fn opens_raw_string(characters: &[char], index: usize) -> bool {
    let Some(previous_index) = index.checked_sub(1) else {
        return true;
    };
    let Some(&previous) = characters.get(previous_index) else {
        return true;
    };
    if previous == 'b' || previous == 'c' {
        return previous_index
            .checked_sub(1)
            .and_then(|before| characters.get(before))
            .is_none_or(|character| !is_identifier_char(*character));
    }
    !is_identifier_char(previous)
}

/// Whether `hashes` hash marks follow the quote at `from`, closing a raw string.
fn closes_raw_string(characters: &[char], from: usize, hashes: usize) -> bool {
    (0..hashes).all(|offset| at(characters, from, offset) == Some('#'))
}

#[cfg(test)]
mod tests {
    use super::{findings, message, survey, without_prose};

    /// The constructs a fixture is found to contain, in line order.
    fn constructs(source: &str) -> Vec<&'static str> {
        findings(source)
            .into_iter()
            .map(|finding| finding.construct)
            .collect()
    }

    /// The lines a fixture's findings sit on, in order.
    fn lines(source: &str) -> Vec<usize> {
        findings(source)
            .into_iter()
            .map(|finding| finding.line)
            .collect()
    }

    #[test]
    fn an_unwritten_body_is_a_violation() {
        let source = "pub fn scan() -> Page {\n    todo!()\n}\n";
        assert_eq!(constructs(source), ["a `todo!` body"]);
        assert_eq!(lines(source), [2]);
    }

    #[test]
    fn an_unimplemented_body_is_a_violation() {
        let source = "pub fn scan() -> Page {\n    unimplemented!(\"M1\")\n}\n";
        assert_eq!(constructs(source), ["an `unimplemented!` body"]);
        assert_eq!(lines(source), [2]);
    }

    #[test]
    fn a_placeholder_is_found_behind_any_delimiter_and_behind_a_path() {
        assert_eq!(constructs("todo![];"), ["a `todo!` body"]);
        assert_eq!(constructs("todo! {};"), ["a `todo!` body"]);
        assert_eq!(constructs("core::todo!();"), ["a `todo!` body"]);
    }

    #[test]
    fn prose_naming_a_forbidden_construct_is_not_a_violation() {
        let source = concat!(
            "//! A page is never left as todo!() or #[allow(dead_code)].\n",
            "/// Callers reaching for todo!() should read CONTRIBUTING.md.\n",
            "pub fn scan() {} // not #[expect(clippy::pedantic)] either\n"
        );
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn a_block_comment_naming_a_forbidden_construct_is_not_a_violation() {
        let source = concat!(
            "/* todo!() */\n",
            "/* outer /* nested todo!() */ still a comment #[allow(x)] */\n",
            "pub fn scan() {}\n"
        );
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn a_literal_naming_a_forbidden_construct_is_not_a_violation() {
        assert!(constructs("const M: &str = \"todo!()\";").is_empty());
        assert!(constructs("const M: &str = r#\"#[allow(x)] todo!()\"#;").is_empty());
        assert!(constructs("const M: &str = \"a \\\" todo!() b\";").is_empty());
        assert!(constructs("const M: &[u8] = br\"todo!()\";").is_empty());
    }

    #[test]
    fn code_around_stripped_prose_is_still_checked() {
        let source = "/* note */ todo!(); // and a note after it\n";
        assert_eq!(constructs(source), ["a `todo!` body"]);
        assert_eq!(lines(source), [1]);
    }

    #[test]
    fn stripping_prose_does_not_join_the_tokens_either_side_of_it() {
        let source = "to/* split */do!();";
        let code = without_prose(source);
        assert_eq!(code.len(), source.len(), "blanked, not removed: {code:?}");
        assert!(!code.contains("todo"), "{code:?}");
        assert!(constructs(source).is_empty());
    }

    #[test]
    fn blanking_keeps_every_line_break() {
        let source = "/*\n a\n b\n*/\nlet text = \"x\ny\";\n";
        assert_eq!(
            without_prose(source).lines().count(),
            source.lines().count()
        );
    }

    #[test]
    fn an_allow_attribute_is_a_violation() {
        let source = "#[allow(dead_code)]\npub fn scan() {}\n";
        assert_eq!(constructs(source), ["an `allow` attribute"]);
        assert_eq!(lines(source), [1]);
    }

    #[test]
    fn an_expect_attribute_is_a_violation() {
        let source = "#[expect(clippy::pedantic)]\npub fn scan() {}\n";
        assert_eq!(constructs(source), ["an `expect` attribute"]);
    }

    #[test]
    fn an_inner_attribute_suppression_is_a_violation() {
        assert_eq!(
            constructs("#![allow(missing_docs)]\n"),
            ["an `allow` attribute"]
        );
        assert_eq!(
            constructs("# ! [ allow(missing_docs) ]\n"),
            ["an `allow` attribute"]
        );
    }

    #[test]
    fn a_suppression_wrapped_in_cfg_attr_is_a_violation() {
        let source = "#[cfg_attr(test, allow(dead_code))]\npub fn scan() {}\n";
        assert_eq!(constructs(source), ["an `allow` attribute"]);
    }

    #[test]
    fn an_attribute_spanning_lines_is_reported_where_the_suppression_is_written() {
        let source = "#[\n    allow(dead_code)\n]\npub fn scan() {}\n";
        assert_eq!(lines(source), [2]);
    }

    #[test]
    fn an_ordinary_attribute_is_not_a_violation() {
        let source = concat!(
            "#[derive(Debug, Clone, Copy)]\n",
            "#[non_exhaustive]\n",
            "#[must_use]\n",
            "#[cfg(test)]\n",
            "#[should_panic(expected = \"allow\")]\n",
            "#[doc(alias = \"expect(\")]\n",
            "pub struct Dpi(u16);\n"
        );
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn a_method_named_expect_is_not_an_attribute() {
        let source = "let value = candidate.expect(\"present\");\nlet other = allow(x);\n";
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn an_identifier_containing_a_forbidden_name_is_not_a_violation() {
        let source = concat!(
            "let todo_count = 0;\n",
            "let retodo = 0;\n",
            "not_todo!();\n",
            "todo_later!();\n",
            "unimplemented_yet!();\n"
        );
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn a_comparison_is_not_a_macro_call() {
        assert!(constructs("if todo != 0 {}").is_empty());
        assert!(constructs("if todo!= 0 {}").is_empty());
        assert!(constructs("let flag = !todo;").is_empty());
    }

    #[test]
    fn a_character_literal_holding_a_quote_does_not_hide_the_code_after_it() {
        let source = "let quote = '\"';\ntodo!();\n";
        assert_eq!(constructs(source), ["a `todo!` body"]);
        assert_eq!(lines(source), [2]);
    }

    #[test]
    fn an_escaped_character_literal_does_not_hide_the_code_after_it() {
        let source = "let tick = '\\'';\nlet nul = '\\u{0}';\ntodo!();\n";
        assert_eq!(lines(source), [3]);
    }

    #[test]
    fn a_lifetime_is_not_a_character_literal() {
        let source = "pub fn name<'a>(text: &'a str) -> &'a str {\n    todo!()\n}\n";
        assert_eq!(constructs(source), ["a `todo!` body"]);
        assert_eq!(lines(source), [2]);
    }

    #[test]
    fn every_violation_is_reported_and_in_line_order() {
        let source = concat!(
            "#[allow(dead_code)]\n",
            "pub fn one() -> Dpi {\n",
            "    todo!()\n",
            "}\n",
            "#[expect(clippy::pedantic)]\n",
            "pub fn two() -> Dpi {\n",
            "    unimplemented!()\n",
            "}\n"
        );
        assert_eq!(
            constructs(source),
            [
                "an `allow` attribute",
                "a `todo!` body",
                "an `expect` attribute",
                "an `unimplemented!` body",
            ]
        );
        assert_eq!(lines(source), [1, 3, 5, 7]);
    }

    #[test]
    fn a_clean_source_is_not_a_violation() {
        let source = concat!(
            "//! A capability table.\n",
            "\n",
            "/// The optical resolution, in dots per inch.\n",
            "pub const fn resolution() -> u16 {\n",
            "    300\n",
            "}\n"
        );
        assert!(
            constructs(source).is_empty(),
            "found {:?}",
            findings(source)
        );
    }

    #[test]
    fn a_violation_names_the_crate_the_file_the_line_and_the_reason() {
        let source = "pub fn scan() -> Page {\n    todo!()\n}\n";
        let finding = findings(source).pop().expect("the body is unwritten");
        let reported = message("scankit-escl", "src/job.rs", &finding);
        assert!(
            reported.starts_with("scankit-escl: src/job.rs:2 has a `todo!` body; "),
            "{reported}"
        );
        assert!(reported.contains("never deferred"), "{reported}");
    }

    #[test]
    fn the_gate_reads_the_product_crates_and_finds_nothing_to_object_to() {
        let survey = survey().expect("the workspace manifest and sources are readable");
        assert!(survey.crates > 0, "the product set has crates to examine");
        assert!(
            survey.files > 0,
            "the product set has sources to examine; a gate that reads nothing proves nothing"
        );
        assert!(
            survey.violations.is_empty(),
            "run `cargo run -p xtask -- placeholder`: {violations:?}",
            violations = survey.violations
        );
    }
}
