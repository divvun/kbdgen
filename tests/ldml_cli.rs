//! `kbdgen ldml` as a process: exit status and what it leaves behind.

use std::process::Command;

// [spec:kbdgen:def:ldml.cli.commands/test]
#[test]
fn errors_exit_non_zero_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("layouts")).unwrap();
    std::fs::write(
        dir.path().join("layouts/sme.yaml"),
        "displayNames: {sme: Davvisámegiella}\n",
    )
    .unwrap();
    let out = dir.path().join("out");
    for command in ["export", "compile"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kbdgen"))
            .args(["ldml", command, "-b"])
            .arg(dir.path())
            .arg("-o")
            .arg(&out)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{command}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("kbdgen ldml migrate"),
            "{command}: {stderr}"
        );
        assert!(!out.exists(), "{command} wrote output");
    }
}
