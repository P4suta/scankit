// SPDX-FileCopyrightText: 2026 scankit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every gate shares: where the repository is, how to read it, and how a finding is
//! reported.
//!
//! A gate is four things — the name it is invoked by, the sentence its holding justifies,
//! the document to read when it does not hold, and the check itself. Reporting is written
//! once here so that every gate speaks with the same voice and so that adding a gate is
//! writing a module and one line in the dispatcher's table.
//!
//! The list of product crates is *derived* rather than kept. `Cargo.toml` already names
//! the workspace members; this module subtracts the members that are deliberately not
//! product code. A crate added to the workspace and forgotten here is therefore checked
//! as product code and fails the gates, where a hand-maintained list would have skipped
//! it in silence.
//!
//! Everything here is hand-rolled over manifest text. The tool that enforces "the
//! vocabulary crate has no dependencies" declares none itself, so there is no TOML
//! parser and no `cargo metadata` JSON model to lean on; the readers below understand
//! the one style this repository writes and treat anything else as a finding rather
//! than a best effort.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Workspace members that are not product code.
///
/// A denylist rather than an allowlist, because the two fail in opposite directions and
/// only one of them fails safely. Each entry is a member path exactly as `Cargo.toml`
/// spells it, and an entry naming something that is no longer a member is an error rather
/// than a no-op, so this list cannot rot unnoticed.
const NON_PRODUCT_MEMBERS: &[&str] = &[
    // The repository's own tooling, which is this program. It is still linted, tested,
    // and dependency-checked; it is exempt only from the gates that scan product
    // sources, because a gate must not certify the tool that implements it.
    "xtask",
];

/// One repository gate: a subcommand of `xtask`.
#[derive(Debug)]
pub(crate) struct Gate {
    /// The name the gate is invoked by, after `cargo run -p xtask --`.
    pub(crate) name: &'static str,
    /// What holding this gate means, in one line. Printed when the check finds nothing,
    /// and listed in the usage message, so it states what was actually checked.
    pub(crate) purpose: &'static str,
    /// Where to read about the invariant. Printed after the violations when it fails.
    pub(crate) reference: &'static str,
    /// The check itself. It returns one message per violation, or an error when it
    /// could not run at all.
    pub(crate) run: fn() -> io::Result<Vec<String>>,
}

impl Gate {
    /// Run the gate and turn its findings into output and an exit code.
    ///
    /// A gate that cannot run is a failure, not a pass: an unreadable manifest tells us
    /// nothing about the invariant, and reporting success there would be the one failure
    /// mode a policy check must not have.
    pub(crate) fn report(&self) -> ExitCode {
        let Self {
            name,
            purpose,
            reference,
            run,
        } = self;
        match run() {
            Err(error) => {
                eprintln!("xtask: {name} could not run: {error}");
                ExitCode::FAILURE
            },
            Ok(violations) if violations.is_empty() => {
                println!("{name}: {purpose}");
                ExitCode::SUCCESS
            },
            Ok(violations) => {
                for violation in &violations {
                    eprintln!("{name}: {violation}");
                }
                eprintln!(
                    "{name}: {count} violation(s). See {reference}",
                    count = violations.len()
                );
                ExitCode::FAILURE
            },
        }
    }
}

/// One workspace member, located and named.
#[derive(Debug)]
pub(crate) struct Member {
    /// The package name, as its own manifest declares it.
    pub(crate) name: String,
    /// The directory holding that manifest.
    pub(crate) directory: PathBuf,
}

/// Locate the workspace root relative to this crate.
pub(crate) fn workspace_root() -> io::Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "the xtask manifest directory has no parent",
            )
        })
}

/// Every workspace member, in workspace order, named by the package name its own
/// manifest declares rather than by its directory, because a dependency is written with
/// the package name and the two are free to differ.
pub(crate) fn members() -> io::Result<Vec<Member>> {
    let root = workspace_root()?;
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let paths = workspace_member_paths(&manifest);
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cargo.toml declares no workspace members",
        ));
    }
    let mut found = Vec::new();
    for path in paths {
        let directory = root.join(&path);
        let manifest = fs::read_to_string(directory.join("Cargo.toml"))?;
        let name = package_name(&manifest).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{path}/Cargo.toml declares no package name"),
            )
        })?;
        found.push(Member {
            name: name.to_owned(),
            directory,
        });
    }
    Ok(found)
}

/// The product crates: every workspace member except the exempted tooling.
pub(crate) fn product_crates() -> io::Result<Vec<Member>> {
    let root = workspace_root()?;
    let manifest = fs::read_to_string(root.join("Cargo.toml"))?;
    let paths = workspace_member_paths(&manifest);
    for excluded in NON_PRODUCT_MEMBERS {
        if !paths.iter().any(|member| member == excluded) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "`{excluded}` is exempted from the product set but is not a workspace \
                     member; the exemption list in xtask/src/shared.rs has gone stale"
                ),
            ));
        }
    }
    let all = members()?;
    Ok(all
        .into_iter()
        .filter(|member| {
            !NON_PRODUCT_MEMBERS.iter().any(|excluded| {
                let last = excluded.rsplit('/').next().unwrap_or(excluded);
                member.name == last
            })
        })
        .collect())
}

/// Read the member paths out of a workspace manifest.
///
/// It understands the one form this repository writes — a `members` array under
/// `[workspace]`, on one line or several — and reads nothing else.
fn workspace_member_paths(manifest: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut inside_workspace = false;
    let mut inside_members = false;

    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        if inside_members {
            paths.extend(quoted_values(line).into_iter().map(str::to_owned));
            inside_members = !line.contains(']');
            continue;
        }

        if let Some(header) = table_header(line) {
            inside_workspace = header == "workspace";
            continue;
        }

        if inside_workspace {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "members" {
                    paths.extend(quoted_values(value).into_iter().map(str::to_owned));
                    inside_members = !value.contains(']');
                }
            }
        }
    }

    paths
}

/// Read the package name out of a crate manifest.
pub(crate) fn package_name(manifest: &str) -> Option<&str> {
    let mut inside_package = false;
    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if let Some(header) = table_header(line) {
            inside_package = header == "package";
            continue;
        }
        if inside_package {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    return quoted_values(value).first().copied();
                }
            }
        }
    }
    None
}

/// Which manifest table a dependency was declared in.
///
/// `cargo` ships normal and build dependencies and keeps dev dependencies at home, and
/// the deps gate's rules differ across exactly that line, so the kind travels with every
/// parsed dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepKind {
    /// `[dependencies]`, in any target scope: ships in the artifact.
    Normal,
    /// `[dev-dependencies]`: compiled only for this crate's own tests.
    Development,
    /// `[build-dependencies]`: compiled into the build script.
    Build,
}

/// One declared dependency: the real crate it names, and the table it was found in.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Declared {
    /// The crate name as `cargo` resolves it — the `package = "..."` rename target when
    /// one is written, the key otherwise.
    pub(crate) name: String,
    /// The table the declaration lives in.
    pub(crate) kind: DepKind,
}

/// Every dependency a manifest declares, with its kind.
///
/// Understands `[dependencies]` tables, `[dependencies.name]` subtables, their `dev-`
/// and `build-` forms, and the `[target.'cfg(...)'.dependencies]` scoping around any of
/// them. A `package = "real-name"` rename is honored, because the rename is exactly the
/// spelling that would otherwise smuggle a workspace edge past a name check.
pub(crate) fn declared_dependencies(manifest: &str) -> Vec<Declared> {
    /// What the scanner is inside of, between table headers.
    enum Context {
        /// A table whose entries are not dependencies.
        Other,
        /// A `[dependencies]`-family table: each key is one dependency.
        Table(DepKind),
        /// A `[dependencies.name]` subtable: the keys describe one dependency, whose
        /// index in the result is carried so a `package` rename can correct its name.
        Subtable(usize),
    }

    let mut found: Vec<Declared> = Vec::new();
    let mut context = Context::Other;

    for line in manifest.lines() {
        let line = before_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = table_header(line) {
            // The whole header first: `[target.'cfg(..)'.dependencies]` ends in a name
            // that looks like a subtable component, so the table reading must win.
            context = if let Some(kind) = dependency_kind(header) {
                Context::Table(kind)
            } else if let Some((prefix, name)) = header.rsplit_once('.') {
                match dependency_kind(prefix) {
                    Some(kind) => {
                        found.push(Declared {
                            name: name.trim().trim_matches('"').to_owned(),
                            kind,
                        });
                        Context::Subtable(found.len().saturating_sub(1))
                    },
                    None => Context::Other,
                }
            } else {
                Context::Other
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        match context {
            Context::Other => {},
            Context::Table(kind) => {
                if !key.is_empty() {
                    let name = renamed(value).unwrap_or(key).to_owned();
                    found.push(Declared { name, kind });
                }
            },
            Context::Subtable(index) => {
                if key == "package" {
                    if let Some(real) = quoted_values(value).first() {
                        if let Some(entry) = found.get_mut(index) {
                            (*real).clone_into(&mut entry.name);
                        }
                    }
                }
            },
        }
    }

    found
}

/// The `package = "real-name"` rename inside an inline dependency value, if present.
fn renamed(value: &str) -> Option<&str> {
    let (_, tail) = value.split_once("package")?;
    let tail = tail.trim_start().strip_prefix('=')?;
    quoted_values(tail).first().copied()
}

/// The dependency kind a table header (with any `target.*` scope stripped) names.
fn dependency_kind(header: &str) -> Option<DepKind> {
    let bare = header.rsplit_once('.').map_or(header, |(scope, last)| {
        if scope.starts_with("target") {
            last
        } else {
            header
        }
    });
    match bare {
        "dependencies" => Some(DepKind::Normal),
        "dev-dependencies" => Some(DepKind::Development),
        "build-dependencies" => Some(DepKind::Build),
        _ => None,
    }
}

/// The name inside a `[table]` header, if the line is one.
///
/// Written to accept the `[[array]]` form through the same door, so a `[[bin]]` header
/// is `bin` here and the callers that care about the doubled form ask for it by name.
pub(crate) fn table_header(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let inner = inner
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(inner);
    Some(inner.trim())
}

/// Everything before the first `#` that is not inside a string.
pub(crate) fn before_comment(line: &str) -> &str {
    let mut inside = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match character {
            '\\' if inside => escaped = !escaped,
            '"' if !escaped => inside = !inside,
            '#' if !inside => return line.get(..index).unwrap_or(line),
            _ => escaped = false,
        }
    }
    line
}

/// The string literals on a line, in order, without their quotes.
pub(crate) fn quoted_values(line: &str) -> Vec<&str> {
    let mut values = Vec::new();
    let mut rest = line;
    while let Some((_, after)) = rest.split_once('"') {
        let Some((value, remainder)) = after.split_once('"') else {
            break;
        };
        values.push(value);
        rest = remainder;
    }
    values
}

/// Gather every `.rs` file under `dir`, recursively, in a stable order.
pub(crate) fn rust_sources(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    collect_rust_sources(dir, &mut found)?;
    found.sort();
    Ok(found)
}

/// Walk one directory, appending in whatever order the filesystem reports.
fn collect_rust_sources(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_sources(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Name a source file relative to the directory it was found under.
pub(crate) fn relative_name(source: &Path, dir: &Path) -> String {
    source
        .strip_prefix(dir)
        .unwrap_or(source)
        .display()
        .to_string()
}

/// Drop `//` comments so prose that names a forbidden token is not itself a violation.
///
/// This is a line-oriented approximation that does not track string literals or block
/// comments. It is adequate because it reads only this repository's own sources, and the
/// worst case is ignoring a token inside a string literal, which is not a violation.
pub(crate) fn code_only(source: &str) -> String {
    source
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `line` contains `token` as a whole word rather than inside a longer
/// identifier, so `Instant` is found and `Instantiation` is left alone.
pub(crate) fn holds_token(line: &str, token: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(token) {
        let before = rest.get(..at).and_then(|head| head.chars().next_back());
        let after = rest
            .get(at.saturating_add(token.len())..)
            .and_then(|tail| tail.chars().next());
        let bounded = |edge: Option<char>| {
            edge.is_none_or(|character| !character.is_alphanumeric() && character != '_')
        };
        if bounded(before) && bounded(after) {
            return true;
        }
        let Some(next) = rest.get(at.saturating_add(1)..) else {
            break;
        };
        rest = next;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        DepKind, before_comment, declared_dependencies, holds_token, members, package_name,
        product_crates, quoted_values, table_header, workspace_member_paths,
    };

    #[test]
    fn reads_members_from_a_multi_line_array() {
        let manifest = "[workspace]\nresolver = \"3\"\nmembers = [\n  \"crates/a\",\n  \"xtask\",\n]\nexclude = [\"fuzz\"]\n";
        assert_eq!(workspace_member_paths(manifest), ["crates/a", "xtask"]);
    }

    #[test]
    fn reads_members_only_from_the_workspace_table() {
        let manifest = "[workspace.metadata]\nmembers = [\"decoy\"]\n\n[workspace]\nmembers = [\"crates/a\"]\n";
        assert_eq!(workspace_member_paths(manifest), ["crates/a"]);
    }

    #[test]
    fn a_commented_out_member_is_not_a_member() {
        let manifest = "[workspace]\nmembers = [\n  \"crates/a\",\n  # \"crates/b\",\n]\n";
        assert_eq!(workspace_member_paths(manifest), ["crates/a"]);
    }

    #[test]
    fn reads_the_package_name_and_not_another_tables_name() {
        let manifest =
            "[package]\nname = \"scankit-core\"\nedition = \"2024\"\n\n[lints]\nname = \"decoy\"\n";
        assert_eq!(package_name(manifest), Some("scankit-core"));
        assert_eq!(package_name("[lints]\nname = \"decoy\"\n"), None);
    }

    #[test]
    fn a_table_header_is_a_whole_line_and_arrays_pass_through() {
        assert_eq!(table_header("[package]"), Some("package"));
        assert_eq!(table_header("[[bin]]"), Some("bin"));
        assert_eq!(table_header("members = [\"crates/a\"]"), None);
        assert_eq!(quoted_values("name = \"a\" # \"b\""), ["a", "b"]);
        assert_eq!(quoted_values(before_comment("name = \"a\" # \"b\"")), ["a"]);
    }

    #[test]
    fn dependencies_carry_their_kind() {
        let manifest = concat!(
            "[dependencies]\n",
            "scankit-core = { path = \"../scankit-core\" }\n",
            "[dev-dependencies]\n",
            "scankit-testkit = { path = \"../scankit-testkit\" }\n",
            "[build-dependencies]\n",
            "cc = \"1\"\n",
        );
        let found = declared_dependencies(manifest);
        let kind_of = |name: &str| {
            found
                .iter()
                .find(|declared| declared.name == name)
                .map(|declared| declared.kind)
        };
        assert_eq!(kind_of("scankit-core"), Some(DepKind::Normal));
        assert_eq!(kind_of("scankit-testkit"), Some(DepKind::Development));
        assert_eq!(kind_of("cc"), Some(DepKind::Build));
    }

    #[test]
    fn a_subtable_and_a_target_scope_are_dependencies_too() {
        let manifest = concat!(
            "[dependencies.quick-xml]\n",
            "version = \"0.41\"\n",
            "[target.'cfg(windows)'.dependencies]\n",
            "windows-sys = \"0.60\"\n",
        );
        let found = declared_dependencies(manifest);
        assert!(found.iter().any(|declared| declared.name == "quick-xml"));
        assert!(found.iter().any(|declared| declared.name == "windows-sys"));
        assert!(
            !found.iter().any(|declared| declared.name == "version"),
            "keys inside a subtable describe it, they are not further dependencies"
        );
    }

    #[test]
    fn a_rename_names_the_real_crate() {
        let inline = "[dependencies]\ncore2 = { package = \"scankit-core\", path = \"x\" }\n";
        let found = declared_dependencies(inline);
        assert!(found.iter().any(|declared| declared.name == "scankit-core"));
        assert!(!found.iter().any(|declared| declared.name == "core2"));

        let subtable = "[dependencies.core2]\npackage = \"scankit-core\"\n";
        let found = declared_dependencies(subtable);
        assert!(found.iter().any(|declared| declared.name == "scankit-core"));
    }

    #[test]
    fn an_empty_manifest_declares_nothing() {
        assert!(declared_dependencies("").is_empty());
        assert!(declared_dependencies("[dependencies]\n").is_empty());
    }

    #[test]
    fn a_token_is_a_whole_word() {
        assert!(holds_token("let t = Instant::now();", "Instant"));
        assert!(!holds_token("let t = Instantiation::new();", "Instant"));
        assert!(!holds_token("let re_sleep = 1;", "sleep"));
    }

    #[test]
    fn the_workspace_is_readable_and_the_product_set_excludes_the_tooling() {
        let all = members().expect("the workspace manifest is readable");
        let names: Vec<&str> = all.iter().map(|member| member.name.as_str()).collect();
        assert!(names.contains(&"scankit-core"), "found {names:?}");
        assert!(names.contains(&"scankit"), "found {names:?}");
        assert!(names.contains(&"xtask"), "found {names:?}");

        let product = product_crates().expect("the workspace manifest is readable");
        let names: Vec<&str> = product.iter().map(|member| member.name.as_str()).collect();
        assert!(!names.contains(&"xtask"), "xtask is not product code");
        for member in &product {
            assert!(
                member.directory.join("Cargo.toml").is_file(),
                "{name} has a manifest",
                name = member.name
            );
        }
    }
}
