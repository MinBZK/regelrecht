//! Guards the **committed prose sidecar**: every `prose/*.md` must be
//! well-formed (parseable frontmatter, unique node ids). This runs the binary's
//! `prose status`, which loads the real sidecar.
//!
//! It deliberately does **not** assert that the prose is *in sync* with the code
//! (no missing/stale gate): keeping the narrative current is the job of the
//! scheduled drift flow, which opens a PR — not a CI gate that would block
//! unrelated changes. So this only proves the files parse and load.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

fn workspace_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml")
}

#[test]
fn committed_prose_loads_cleanly() {
    let out = Command::new(env!("CARGO_BIN_EXE_arch-extract"))
        .args([
            "prose",
            "status",
            "--manifest-path",
            workspace_manifest().to_str().expect("utf-8 manifest path"),
        ])
        .output()
        .expect("running arch-extract prose status");

    assert!(
        out.status.success(),
        "prose status failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Prose coverage:"),
        "expected a coverage line, got:\n{stdout}"
    );

    // A malformed sidecar file makes `load_prose` warn on stderr and skip it;
    // the committed files must all parse.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("skipping"),
        "a committed prose file failed to parse:\n{stderr}"
    );
}
