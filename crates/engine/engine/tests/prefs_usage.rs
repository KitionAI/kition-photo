//! Every preference does something, or the Preferences dialog hides it (issue #204).
//!
//! Walks every key of `Preferences::default()` serialised to JSON (`section.key`) and checks
//! the workspace sources: each one is either read outside `prefs.rs` or listed in
//! `prefs::HIDDEN_UNTIL_IMPLEMENTED`, never both. A setting that appears in the dialog but that
//! nothing reads is a control that silently does nothing.
//!
//! "Read" is a source scan (no type information), so it is deliberately simple:
//! - a member access `.<snake_key>` in a non-test source file that also names the section
//!   (`<snake_section>`), e.g. `p.file_handling.recent_files` or `let fh = &p.file_handling;
//!   … fh.autosave`;
//! - or the JSON path as a string literal (`"fileHandling.autosave"`), for code that reads
//!   preferences by path;
//! - or `self.<snake_key>` in an inherent `impl` of the section's struct in `prefs.rs` (helpers
//!   such as `TransparencyAndGamut::colors`);
//! - or, in the web editor (`web/src`, which reads preferences with `prefs.get`), the JSON path
//!   in either kind of quotes (`'tools.useShiftKeyForToolSwitch'`), or a whole section read as
//!   a `prefs.get` path (`path: 'unitsAndRulers'`) and the camelCase key named as a member
//!   (`.rulers`, `rulers?:`) in any web source.

use kition_photo_engine::prefs::{HIDDEN_UNTIL_IMPLEMENTED, Preferences};
use std::path::{Path, PathBuf};

fn snake(camel: &str) -> String {
    let mut out = String::new();
    for c in camel.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `needle` occurs in `text` as a whole identifier (after `prefix`, e.g. "." or "self."). A
/// range such as `0..colors` is not a member access.
fn has_ident(text: &str, prefix: &str, ident: &str) -> bool {
    let needle = format!("{prefix}{ident}");
    text.match_indices(&needle).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + needle.len()..].chars().next();
        let starts = if prefix.is_empty() { before.is_none_or(|c| !is_ident(c)) } else { prefix != "." || before != Some('.') };
        starts && after.is_none_or(|c| !is_ident(c))
    })
}

/// Production source of a file: everything before its unit-test module.
fn production(text: &str) -> &str {
    // Windows checkouts may use CRLF; test-only reads must stay excluded.
    let cut = ["#[cfg(test)]\nmod ", "#[cfg(test)]\r\nmod ", "#[cfg(test)]\npub mod ", "#[cfg(test)]\r\npub mod "]
        .into_iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());
    &text[..cut]
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() {
            if !matches!(name.as_str(), "tests" | "examples" | "benches" | "target") {
                rust_sources(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(p);
        }
    }
}

/// The web editor's TypeScript sources, without its tests.
fn web_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() {
            if name != "node_modules" {
                web_sources(&p, out);
            }
        } else if (name.ends_with(".ts") || name.ends_with(".tsx")) && !name.contains(".test.") && !name.contains(".spec.") {
            out.push(p);
        }
    }
}

/// `text` quotes `path` as a whole string literal, in single or double quotes.
fn quotes(text: &str, path: &str) -> bool {
    text.contains(&format!("'{path}'")) || text.contains(&format!("\"{path}\""))
}

/// `text` passes `section` whole as a `prefs.get` path (`{ path: 'unitsAndRulers' }`).
fn reads_section(text: &str, section: &str) -> bool {
    text.contains(&format!("path: '{section}'")) || text.contains(&format!("path: \"{section}\""))
}

/// `text` names `key` as a member: an access `.key` or a declared field `key:` / `key?:`.
fn names_member(text: &str, key: &str) -> bool {
    has_ident(text, ".", key)
        || [":", "?:"]
            .iter()
            .any(|suffix| text.match_indices(&format!("{key}{suffix}")).any(|(i, _)| text[..i].chars().next_back().is_none_or(|c| !is_ident(c))))
}

/// `pub struct Name { … }` field names, and the bodies of `impl Name { … }` blocks, in prefs.rs.
fn struct_fields(src: &str, name: &str) -> Vec<String> {
    let Some(start) = src.find(&format!("pub struct {name} {{")) else { return Vec::new() };
    let body = &src[start..];
    let end = body.find("\n}").unwrap_or(body.len());
    body[..end].lines().filter_map(|l| l.trim().strip_prefix("pub ")?.split_once(':').map(|(f, _)| f.trim().to_string())).collect()
}

fn inherent_impls(src: &str, name: &str) -> String {
    let marker = format!("\nimpl {name} {{");
    src.match_indices(&marker)
        .map(|(i, _)| {
            let body = &src[i + 1..];
            body[..body.find("\n}").unwrap_or(body.len())].to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The code of a Rust source with the contents of its string and character literals and its
/// comments blanked out, so a description that only mentions `cursors` or `.other` in prose
/// (`"filter.other.highPass"`) is not mistaken for a member access.
fn code_only(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r' && matches!(next, Some('"') | Some('#')) && chars[..i].last().is_none_or(|p| !is_ident(*p)) {
            // A raw string r"…" or r#"…"#: blank everything up to the matching quote and hashes.
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            out.push('r');
            out.extend(std::iter::repeat_n('#', hashes));
            out.push('"');
            j += 1;
            while j < chars.len() {
                if chars[j] == '"' && (0..hashes).all(|h| chars.get(j + 1 + h) == Some(&'#')) {
                    out.push('"');
                    out.extend(std::iter::repeat_n('#', hashes));
                    j += 1 + hashes;
                    break;
                }
                out.push(blank(chars[j]));
                j += 1;
            }
            i = j;
        } else if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    out.push(' ');
                    i += 1;
                }
                if i < chars.len() {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            if i < chars.len() {
                out.push('"');
                i += 1;
            }
        } else if c == '\'' && (next == Some('\\') || chars.get(i + 2) == Some(&'\'')) {
            // A character literal ('x', '\n', '\u{1F600}'); a lifetime ('a) has no closing quote.
            let from = if next == Some('\\') { i + 3 } else { i + 2 };
            let end = (from..chars.len()).find(|&j| chars[j] == '\'').unwrap_or(chars.len() - 1);
            out.push('\'');
            out.extend(std::iter::repeat_n(' ', end - i - 1));
            out.push('\'');
            i = end + 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// A Rust source reads `section.key` (`section_rs`, `field` in snake case; `path` the JSON path):
/// a member access `.field` in code that also names the section, or the JSON path as a string.
fn rust_reads(raw: &str, code: &str, section_rs: &str, field: &str, path: &str) -> bool {
    (has_ident(code, ".", field) && has_ident(code, "", section_rs)) || raw.contains(&format!("\"{path}\""))
}

#[test]
fn every_preference_is_read_or_hidden() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let prefs_rs = std::fs::read_to_string(root.join("crates/engine/engine/src/prefs.rs")).unwrap();
    let mut files = Vec::new();
    rust_sources(&root.join("crates"), &mut files);
    rust_sources(&root.join("bindings"), &mut files);
    let sources: Vec<(String, String)> = files
        .iter()
        .filter(|p| !p.ends_with("engine/src/prefs.rs"))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|t| {
            let raw = production(&t).to_string();
            let code = code_only(&raw);
            (raw, code)
        })
        .collect();
    assert!(sources.len() > 100, "found only {} source files under {}", sources.len(), root.display());
    let mut web_files = Vec::new();
    web_sources(&root.join("web/src"), &mut web_files);
    let web: Vec<String> = web_files.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect();
    assert!(web.len() > 10, "found only {} web sources under {}", web.len(), root.join("web/src").display());
    let web_reads = |section: &str, key: Option<&str>| {
        let whole = web.iter().any(|t| reads_section(t, section));
        match key {
            None => whole,
            Some(k) => web.iter().any(|t| quotes(t, &format!("{section}.{k}"))) || (whole && web.iter().any(|t| names_member(t, k))),
        }
    };

    // Section key → struct name, from `pub <field>: <Type>,` in `struct Preferences`.
    let section_types: Vec<(String, String)> = {
        let start = prefs_rs.find("pub struct Preferences {").unwrap();
        let body = &prefs_rs[start..];
        body[..body.find("\n}").unwrap()]
            .lines()
            .filter_map(|l| {
                let (f, ty) = l.trim().strip_prefix("pub ")?.split_once(':')?;
                Some((f.trim().to_string(), ty.trim().trim_end_matches(',').to_string()))
            })
            .collect()
    };

    let json = serde_json::to_value(Preferences::default()).unwrap();
    let mut paths = Vec::new();
    for (section, v) in json.as_object().unwrap() {
        match v.as_object() {
            Some(o) if !o.is_empty() => paths.extend(o.keys().map(|k| (section.clone(), Some(k.clone())))),
            _ => paths.push((section.clone(), None)),
        }
    }

    let mut unread = Vec::new();
    let mut stale = Vec::new();
    for (section, key) in &paths {
        let section_rs = if section == "type" { "type_".to_string() } else { snake(section) };
        let (path, read) = match key {
            None => (section.clone(), sources.iter().any(|(_, code)| has_ident(code, ".", &section_rs)) || web_reads(section, None)),
            Some(k) => {
                let field = snake(k);
                let path = format!("{section}.{k}");
                let ty = section_types.iter().find(|(f, _)| *f == section_rs).map(|(_, t)| t.clone()).unwrap_or_default();
                let in_helpers =
                    !ty.is_empty() && struct_fields(&prefs_rs, &ty).contains(&field) && has_ident(&inherent_impls(&prefs_rs, &ty), "self.", &field);
                let read = in_helpers || sources.iter().any(|(raw, code)| rust_reads(raw, code, &section_rs, &field, &path)) || web_reads(section, Some(k));
                (path, read)
            }
        };
        let hidden = HIDDEN_UNTIL_IMPLEMENTED.contains(&path.as_str());
        if !read && !hidden {
            unread.push(path);
        } else if read && hidden {
            stale.push(path);
        }
    }
    for h in HIDDEN_UNTIL_IMPLEMENTED {
        assert!(
            paths.iter().any(|(s, k)| k.as_ref().map_or_else(|| s.clone(), |k| format!("{s}.{k}")) == *h),
            "HIDDEN_UNTIL_IMPLEMENTED lists `{h}`, which is not a preference"
        );
    }
    assert!(
        unread.is_empty(),
        "these preferences are shown in Edit › Preferences but nothing reads them: wire them up or add them to prefs::HIDDEN_UNTIL_IMPLEMENTED: {unread:#?}"
    );
    assert!(stale.is_empty(), "these preferences are read now: remove them from prefs::HIDDEN_UNTIL_IMPLEMENTED so the dialog shows them: {stale:#?}");
}

#[test]
fn the_scanner_finds_reads() {
    assert!(has_ident("p.file_handling.recent_files.clone()", ".", "recent_files"));
    assert!(!has_ident("p.file_handling.recent_files_x", ".", "recent_files"));
    assert!(has_ident("let fh = &p.file_handling;", "", "file_handling"));
    assert!(!has_ident("my_file_handling", "", "file_handling"));
    assert!(!has_ident("for k in 0..colors {", ".", "colors"));
    assert_eq!(snake("recentFileCount"), "recent_file_count");
    assert_eq!(production("fn a() {}\n#[cfg(test)]\nmod tests {\n p.x.y\n}"), "fn a() {}\n");
    assert!(quotes("useEngineQuery('prefs.get', { path: 'tools.useShiftKeyForToolSwitch' })", "tools.useShiftKeyForToolSwitch"));
    assert!(!quotes("'tools.useShiftKeyForToolSwitchX'", "tools.useShiftKeyForToolSwitch"));
    assert!(reads_section("useEngineQuery<RulerUnits>('prefs.get', { path: 'unitsAndRulers' })", "unitsAndRulers"));
    assert!(!reads_section("export type HelpTopic = 'about' | 'shortcuts'", "shortcuts"));
    assert!(names_member("type RulerUnits = { rulers?: string; pointSize?: string }", "pointSize"));
    assert!(names_member("const unit = units?.rulers ?? 'pixels'", "rulers"));
    assert!(!names_member("const subrulers: string[] = []", "rulers"));
}

#[test]
fn production_excludes_test_modules_with_lf_or_crlf() {
    for newline in ["\n", "\r\n"] {
        for visibility in ["", "pub "] {
            let prefix = format!("// Préférences{newline}fn a() {{}}{newline}");
            let source = format!(
                "{prefix}#[cfg(test)]{newline}{visibility}mod tests {{{newline}    #[test]{newline}    fn hidden_setting() {{ let _ = \"type.smartQuotes\"; }}{newline}}}{newline}"
            );
            assert_eq!(production(&source), prefix, "newline={newline:?}, visibility={visibility:?}");
        }
    }
}

#[test]
fn production_keeps_sources_without_a_test_module() {
    for source in ["", "fn a() {}\n", "// Préférences\r\nfn a() {}\r\n"] {
        assert_eq!(production(source), source);
    }
}

#[test]
fn a_mention_in_a_string_or_comment_is_not_a_read() {
    let reads = |source: &str| rust_reads(source, &code_only(source), "cursors", "other", "cursors.other");
    // A real read of a hidden preference is still caught.
    assert!(reads("fn cursor(p: &Preferences) -> Cursor { p.cursors.other }"));
    assert!(reads("let c = &prefs.cursors;\nlet o = c.other;"));
    assert!(reads("prefs.get(\"cursors.other\")"));
    // Prose that only names the section and a key in literals or comments is not.
    assert!(!reads("\"filter.other.highPass\" => \"…\",\n\"edit.preferences.cursors\" => \"Painting cursors…\","));
    assert!(!reads("// the cursors section; see filter.other\nfn a() {}"));
    assert!(!reads("/* cursors */ let x = r#\"y.other\"#;"));
    assert!(!reads("let c = 'x'; let s = \"cursors\"; let t = \".other\";"));
    assert!(!reads("let q = '\\''; let s = \"cursors\"; let t = \".other\"; fn f<'a>(x: &'a str) {}"));
    assert!(reads("fn f<'a>(p: &'a Preferences) -> bool { let q = '\\''; p.cursors.other }"));
}
