//! The YAML fragment a step of an explanation points to: the block of an
//! article (by its `number`) or of a key in a configuration file, with the
//! line numbers in the file as loaded (spec "waarom in de aanvraag").

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fragment {
    /// The file, relative to the corpus root.
    pub file: String,
    /// First and last line of the block, 1-based, inclusive.
    pub line: usize,
    pub end_line: usize,
    pub yaml: String,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The block that starts at the first line `starts` accepts: that line and
/// every following line that is indented deeper (blank lines inside
/// included, trailing blank lines not).
pub fn block(text: &str, starts: impl Fn(&str) -> bool) -> Option<(usize, usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let i = lines.iter().position(|l| starts(l))?;
    let indent = indent_of(lines[i]);
    let mut end = i;
    for (j, l) in lines.iter().enumerate().skip(i + 1) {
        if l.trim().is_empty() {
            continue;
        }
        if indent_of(l) <= indent {
            break;
        }
        end = j;
    }
    Some((i + 1, end + 1, lines[i..=end].join("\n")))
}

/// Whether a line opens the article with this number (`- number: '102'`).
pub fn is_article(line: &str, number: &str) -> bool {
    let t = line.trim_start();
    let t = t.strip_prefix("- ").unwrap_or(t).trim();
    t.strip_prefix("number:")
        .is_some_and(|v| v.trim().trim_matches(|c| c == '\'' || c == '"') == number)
}

/// Whether a line opens the block of `anchor`: a key `anchor:`, or an item
/// `name: anchor` / `id: anchor` (also in flow style, `- {id: anchor, ...}`).
pub fn is_anchor(line: &str, anchor: &str) -> bool {
    let t = line.trim_start();
    let t = t
        .strip_prefix("- ")
        .unwrap_or(t)
        .trim_start_matches('{')
        .trim_end();
    if t == format!("{anchor}:") || t.starts_with(&format!("{anchor}: ")) {
        return true;
    }
    ["name", "id"].iter().any(|k| {
        let p = format!("{k}: {anchor}");
        t == p
            || [",", " ", "}"]
                .iter()
                .any(|n| t.starts_with(&format!("{p}{n}")))
    })
}

/// A path as shown: relative to the corpus root, otherwise the file name.
pub fn shown(root: &Path, file: &Path) -> String {
    let (Ok(r), Ok(f)) = (root.canonicalize(), file.canonicalize()) else {
        return file.display().to_string();
    };
    f.strip_prefix(&r).map_or_else(
        |_| {
            f.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        },
        |p| p.display().to_string(),
    )
}

/// The fragment of `file` that `starts` opens; `None` if the file cannot be
/// read or has no such block.
pub fn read(root: &Path, file: &Path, starts: impl Fn(&str) -> bool) -> Option<Fragment> {
    let text = std::fs::read_to_string(file).ok()?;
    let (line, end_line, yaml) = block(&text, starts)?;
    Some(Fragment {
        file: shown(root, file),
        line,
        end_line,
        yaml,
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
    }

    #[test]
    fn anchors_in_configuration() {
        let stream =
            "events:\n  - name: aanvraag_ontvangen\n    intake: portaal\n  - name: ander\n";
        assert_eq!(
            block(stream, |l| is_anchor(l, "aanvraag_ontvangen"))
                .unwrap()
                .0,
            2
        );
        assert_eq!(
            block(stream, |l| is_anchor(l, "aanvraag_ontvangen"))
                .unwrap()
                .1,
            3
        );
        let process = "id: p\nportal:\n  cell: c\n  stream: s\nroles: {}\n";
        assert_eq!(block(process, |l| is_anchor(l, "portal")).unwrap().1, 4);
        let form = "velden:\n  - {id: adres_aanvrager, label: Adres}\n  - {id: naam}\n";
        assert_eq!(
            block(form, |l| is_anchor(l, "adres_aanvrager")).unwrap().0,
            2
        );
        assert!(!is_anchor("  - {id: adres_aanvrager_2}", "adres_aanvrager"));
    }
}
