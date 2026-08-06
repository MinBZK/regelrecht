//! What `law-source --rewrite` does to the file on disk.
//!
//! The rewrite replaces a law file with what the official toestand says.
//! When that toestand comes back empty or unrecognisably small, the file
//! must survive untouched: an empty source set means the fetch or the
//! parse failed, never that the law lost its articles. These tests run the
//! real binary against a real file in a temp directory with a cached
//! toestand, because the failure that motivated them erased a complete law
//! from the corpus while the process reported success.

use std::path::Path;
use std::process::Command;

const BWB_ID: &str = "BWBR0000001";
const VALID_FROM: &str = "2024-01-01";

/// A toestand that parses but holds no articles, the shape a changed
/// response format or a wrong BWB id produces.
const EMPTY_TOESTAND: &str =
    r#"<toestand bwb-id="BWBR0000001"><wet-besluit><wettekst/></wet-besluit></toestand>"#;

/// A toestand with the two articles the law actually has.
const TWO_ARTICLE_TOESTAND: &str = r#"<toestand bwb-id="BWBR0000001">
  <wettekst>
    <artikel><kop><label>Artikel</label><nr>1</nr></kop>
      <al>Eerste artikel, officiele tekst.</al>
    </artikel>
    <artikel><kop><label>Artikel</label><nr>2</nr></kop>
      <al>Tweede artikel, officiele tekst.</al>
    </artikel>
  </wettekst>
</toestand>"#;

/// A law file with content worth protecting: two articles, one of them
/// carrying a translation.
const LAW_YAML: &str = r#"bwb_id: BWBR0000001
valid_from: '2024-01-01'
url: https://example.com/BWBR0000001
articles:
  - number: '1'
    text: Eerste artikel, officiele tekst.
    url: https://example.com/BWBR0000001#Artikel1
    machine_readable:
      outputs: []
  - number: '2'
    text: Tweede artikel, verouderde tekst.
    url: https://example.com/BWBR0000001#Artikel2
"#;

struct Setup {
    _dirs: (tempfile::TempDir, tempfile::TempDir),
    law: std::path::PathBuf,
    cache: std::path::PathBuf,
}

/// A law file in one temp directory, a cached toestand in another, so a
/// permission change on the law directory cannot touch the cache.
fn setup(law_yaml: &str, toestand_xml: &str) -> Setup {
    let law_dir = tempfile::tempdir().expect("law dir");
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let law = law_dir.path().join(format!("{VALID_FROM}.yaml"));
    std::fs::write(&law, law_yaml).expect("write law");
    std::fs::write(
        cache_dir.path().join(format!("{BWB_ID}_{VALID_FROM}.xml")),
        toestand_xml,
    )
    .expect("write cache");
    Setup {
        law,
        cache: cache_dir.path().to_path_buf(),
        _dirs: (law_dir, cache_dir),
    }
}

fn run_rewrite(law: &Path, cache: &Path) -> (Option<i32>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_law-source"))
        .args([
            "--offline",
            "--rewrite",
            "--cache",
            cache.to_str().expect("utf-8 path"),
            law.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("the binary is built by cargo test");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.code(), text)
}

/// The failure that started this: an empty official set must not be read
/// as "this law has no articles any more". The file keeps every article,
/// every `machine_readable` and every link, and the run fails instead of
/// reporting success.
#[test]
fn test_an_empty_official_set_leaves_the_law_file_untouched() {
    let s = setup(LAW_YAML, EMPTY_TOESTAND);
    let (code, out) = run_rewrite(&s.law, &s.cache);

    assert_eq!(code, Some(1), "the gate must fail, not pass: {out}");
    assert!(out.contains("rewrite refused"), "{out}");
    assert_eq!(
        std::fs::read_to_string(&s.law).expect("law still readable"),
        LAW_YAML,
        "the law file must be byte-for-byte what it was"
    );
    assert!(
        !s.law.with_file_name(".source-context.yaml").exists(),
        "a refused rewrite must not leave a sidecar"
    );
}

/// A source set that would remove the majority of the file is the same
/// emergency with something left over: refuse and keep the file.
#[test]
fn test_a_rewrite_that_removes_most_of_the_law_is_refused() {
    // Six per-lid entries that all belong to official article 1. The
    // whole-article rewrite would flatten them to one bare article,
    // dropping the leden and the extra fields.
    let fragmented = r#"bwb_id: BWBR0000001
valid_from: '2024-01-01'
url: https://example.com/BWBR0000001
articles:
  - number: '1.1'
    text: Eerste lid.
  - number: '1.2'
    text: Tweede lid.
  - number: 1.2.a
    text: onderdeel a
  - number: 1.2.b
    text: onderdeel b
  - number: 2.1
    text: Ander eerste lid.
  - number: 2.2
    text: Ander tweede lid.
"#;
    let single_article = r#"<toestand bwb-id="BWBR0000001"><wettekst>
      <artikel><kop><label>Artikel</label><nr>1</nr></kop>
        <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
        <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
      </artikel>
    </wettekst></toestand>"#;

    let s = setup(fragmented, single_article);
    let (code, out) = run_rewrite(&s.law, &s.cache);

    assert_eq!(code, Some(1), "{out}");
    assert!(out.contains("rewrite refused"), "{out}");
    assert_eq!(
        std::fs::read_to_string(&s.law).expect("law still readable"),
        fragmented
    );
}

/// The guards must not block the job the rewrite exists for: correcting a
/// drifted article against a sound official set.
#[test]
fn test_a_drifted_article_is_still_corrected() {
    let s = setup(LAW_YAML, TWO_ARTICLE_TOESTAND);
    let (code, out) = run_rewrite(&s.law, &s.cache);

    assert_eq!(code, Some(0), "{out}");
    let rewritten = std::fs::read_to_string(&s.law).expect("law readable");
    assert!(
        rewritten.contains("Tweede artikel, officiele tekst."),
        "the drifted text must be replaced: {rewritten}"
    );
    assert!(
        rewritten.contains("machine_readable"),
        "the translation must be carried over: {rewritten}"
    );
    assert!(
        s.law.with_file_name(".source-context.yaml").exists(),
        "a completed rewrite writes the sidecar"
    );
}

/// A write that cannot complete must leave the law file exactly as it
/// was. The old in-place write clobbered the file first and discovered the
/// problem afterwards.
#[cfg(unix)]
#[test]
fn test_a_failing_write_leaves_the_law_file_untouched() {
    use std::os::unix::fs::PermissionsExt;

    let s = setup(LAW_YAML, TWO_ARTICLE_TOESTAND);
    let law_dir = s.law.parent().expect("law dir").to_path_buf();

    // Read-only directory: the existing file can still be opened and
    // truncated in place, but no new file can be created in it, so the
    // temp-file route fails cleanly where an in-place write would not.
    std::fs::set_permissions(&law_dir, std::fs::Permissions::from_mode(0o555))
        .expect("make law dir read-only");
    let (code, out) = run_rewrite(&s.law, &s.cache);
    std::fs::set_permissions(&law_dir, std::fs::Permissions::from_mode(0o755))
        .expect("restore law dir");

    assert_eq!(code, Some(1), "{out}");
    assert_eq!(
        std::fs::read_to_string(&s.law).expect("law still readable"),
        LAW_YAML,
        "a failed write must not have modified the law file"
    );
}
