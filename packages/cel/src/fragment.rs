//! The YAML fragment a step of an explanation points to: the block of an
//! article (by its `number`) or of a key in a configuration file, with the
//! line numbers in the file as loaded (spec "waarom in de aanvraag").

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fragment {
    /// The file, relative to the corpus root.
    pub file: String,
    /// First and last line of the block, 1-based, inclusive (an empty file
    /// is line 1 to 0).
    pub line: usize,
    pub end_line: usize,
    pub yaml: String,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Whether a line says nothing about the structure: blank or a comment.
fn is_filler(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

fn is_item(line: &str) -> bool {
    line.trim_start().starts_with("- ")
}

/// Where the block of line `i` starts: at `i`, or, if `i` is a later key of
/// a list item (`- intake: x` / `  name: y`), at the `- ` of that item.
fn item_start(lines: &[&str], i: usize) -> usize {
    if is_item(lines[i]) {
        return i;
    }
    let indent = indent_of(lines[i]);
    lines[..i]
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, l)| !is_filler(l))
        .find(|(_, l)| indent_of(l) < indent)
        .filter(|(_, l)| is_item(l) && indent_of(l) + 2 == indent)
        .map_or(i, |(j, _)| j)
}

/// The block that starts at the first line `starts` accepts (or at its list
/// item, see `item_start`): that line and every following line that is
/// indented deeper. Blank and comment lines inside are included, trailing
/// ones not. A key that opens an indentless sequence (`events:` followed by
/// `- ...` at the same indent) keeps those items.
pub fn block(text: &str, starts: impl Fn(&str) -> bool) -> Option<(usize, usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let i = item_start(&lines, lines.iter().position(|l| starts(l))?);
    let indent = indent_of(lines[i]);
    let sequence = !is_item(lines[i]) && lines[i].trim_end().ends_with(':');
    let mut end = i;
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        if is_filler(l) {
            continue;
        }
        let deeper = indent_of(l) > indent;
        if !(deeper || (sequence && indent_of(l) == indent && is_item(l))) {
            break;
        }
        end = j;
    }
    Some((i + 1, end + 1, lines[i..=end].join("\n")))
}

/// The value of `key` on a line that opens it (`key: v`, `- key: v` or in
/// flow style `- {key: v, ...}`), without quotes; `None` if the line does
/// not open `key`.
fn value_of<'l>(line: &'l str, key: &str) -> Option<&'l str> {
    let t = line.trim_start();
    let t = t.strip_prefix("- ").unwrap_or(t);
    let (t, flow) = match t.strip_prefix('{') {
        Some(rest) => (rest.trim_start(), true),
        None => (t, false),
    };
    let v = t.strip_prefix(key)?.strip_prefix(':')?;
    let v = if flow {
        v.split([',', '}']).next().unwrap_or_default()
    } else {
        v
    };
    Some(v.trim().trim_matches(|c| c == '\'' || c == '"'))
}

/// Whether a line opens the article with this number (`- number: '102'`).
pub fn is_article(line: &str, number: &str) -> bool {
    value_of(line, "number") == Some(number)
}

/// Whether a line opens the block of `anchor`: a key `anchor:`, or an item
/// `name: anchor` / `id: anchor` (also in flow style, `- {id: anchor, ...}`).
pub fn is_anchor(line: &str, anchor: &str) -> bool {
    value_of(line, anchor).is_some()
        || ["name", "id"]
            .iter()
            .any(|k| value_of(line, k) == Some(anchor))
}

/// A path as shown: relative to the corpus root, otherwise only the file
/// name (never an absolute path).
pub fn shown(root: &Path, file: &Path) -> String {
    let name = || {
        file.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    };
    match (root.canonicalize(), file.canonicalize()) {
        (Ok(r), Ok(f)) => f
            .strip_prefix(&r)
            .map_or_else(|_| name(), |p| p.display().to_string()),
        _ => name(),
    }
}

/// The file as shown and its text; `None` if it cannot be read.
fn source(root: &Path, file: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(file).ok()?;
    Some((shown(root, file), text))
}

/// The fragment of `file` that `starts` opens; `None` if the file cannot be
/// read or has no such block.
pub fn read(root: &Path, file: &Path, starts: impl Fn(&str) -> bool) -> Option<Fragment> {
    let (file, text) = source(root, file)?;
    let (line, end_line, yaml) = block(&text, starts)?;
    Some(Fragment {
        file,
        line,
        end_line,
        yaml,
    })
}

/// The whole of `file` as a fragment; `None` if it cannot be read.
pub fn whole(root: &Path, file: &Path) -> Option<Fragment> {
    let (file, text) = source(root, file)?;
    Some(Fragment {
        file,
        line: 1,
        end_line: text.lines().count(),
        yaml: text,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const LAW: &str = "$id: x\narticles:\n  - number: '1'\n    text: een\n\n    machine_readable:\n      a: 1\n  - number: '4:2'\n    text: twee\n";

    #[test]
    fn the_block_of_an_article() {
        let (line, end, yaml) = block(LAW, |l| is_article(l, "1")).unwrap();
        assert_eq!((line, end), (3, 7));
        assert!(
            yaml.starts_with("  - number: '1'") && yaml.ends_with("a: 1"),
            "{yaml}"
        );
        let (line, end, _) = block(LAW, |l| is_article(l, "4:2")).unwrap();
        assert_eq!((line, end), (8, 9));
        assert!(block(LAW, |l| is_article(l, "2")).is_none());
        assert!(is_article("  - {number: \"3\", text: drie}", "3"));
    }

    #[test]
    fn anchors_in_configuration() {
        let stream =
            "events:\n  - name: aanvraag_ontvangen\n    intake: portaal\n  - name: ander\n";
        let (line, end, _) = block(stream, |l| is_anchor(l, "aanvraag_ontvangen")).unwrap();
        assert_eq!((line, end), (2, 3));
        let process = "id: p\nportal:\n  cell: c\n  stream: s\nroles: {}\n";
        assert_eq!(block(process, |l| is_anchor(l, "portal")).unwrap().1, 4);
        let form = "velden:\n  - {id: adres_aanvrager, label: Adres}\n  - {id: naam}\n";
        assert_eq!(
            block(form, |l| is_anchor(l, "adres_aanvrager")).unwrap().0,
            2
        );
        assert!(!is_anchor("  - {id: adres_aanvrager_2}", "adres_aanvrager"));
        assert!(!is_anchor(
            "  - name: aanvraag_ontvangen later",
            "aanvraag_ontvangen"
        ));
        assert!(is_anchor(
            "  - name: 'aanvraag_ontvangen'",
            "aanvraag_ontvangen"
        ));
        assert!(!is_anchor("  portal_x: 1", "portal"));
    }

    /// A key that is not the first of its list item: the block is the whole
    /// item.
    #[test]
    fn a_later_key_opens_its_whole_item() {
        let stream = "events:\n  - intake: portaal\n    # de indiening\n    name: aanvraag_ontvangen\n    fields: {}\n  - name: ander\n";
        let (line, end, yaml) = block(stream, |l| is_anchor(l, "aanvraag_ontvangen")).unwrap();
        assert_eq!((line, end), (2, 5), "{yaml}");
        // A key under a mapping (not an item) stays where it is.
        let process = "portal:\n  cell: c\n  form:\n    path: f.yaml\n";
        assert_eq!(block(process, |l| is_anchor(l, "form")).unwrap().0, 3);
    }

    #[test]
    fn comments_and_indentless_sequences() {
        let text = "a:\n  b: 1\n# tussen\n  c: 2\n  # achteraan\n\nd: 3\n";
        assert_eq!(block(text, |l| is_anchor(l, "a")).unwrap().1, 4);
        let text = "events:\n- name: x\n  intake: y\n- name: z\nother: 1\n";
        assert_eq!(block(text, |l| is_anchor(l, "events")).unwrap().1, 4);
        assert_eq!(block(text, |l| is_anchor(l, "x")).unwrap().1, 3);
        let text = "events:\n- intake: y\n  name: x\n- name: z\n";
        assert_eq!(block(text, |l| is_anchor(l, "x")).unwrap().0, 2);
    }

    #[test]
    fn a_path_is_shown_relative_or_as_its_name() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("sub")).unwrap();
        std::fs::write(root.path().join("sub/a.yaml"), "").unwrap();
        assert_eq!(
            shown(root.path(), &root.path().join("sub/a.yaml")),
            "sub/a.yaml"
        );
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::write(elsewhere.path().join("b.yaml"), "").unwrap();
        assert_eq!(
            shown(root.path(), &elsewhere.path().join("b.yaml")),
            "b.yaml"
        );
        assert_eq!(
            shown(root.path(), Path::new("/bestaat/niet/c.yaml")),
            "c.yaml"
        );
        let empty = whole(root.path(), &root.path().join("sub/a.yaml")).unwrap();
        assert_eq!((empty.line, empty.end_line), (1, 0));
    }
}
