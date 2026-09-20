// S01 integration harness (canonical copy, see `crates/serial-marker`).
//
// Run via `cargo test`: the wrapper at
// `crates/serial-marker/tests/serial_marker.rs` includes this file, since a
// virtual workspace root has no package of its own to own a `tests/` target.
// Covers the two S01 risks carried from review #12: (1) the guest marker
// path emits the exact contract bytes, (2) serial-log parsing distinguishes
// a booted guest (marker present) from the S00 0-byte state (absent).

use baseos_serial_marker::{contains_marker, contains_marker_bytes, marker, write_marker};
use std::path::PathBuf;

/// Collects the marker path output the way a serial driver would consume it.
#[test]
fn marker_path_emits_contract_bytes() {
    let mut serial: Vec<u8> = Vec::new();
    write_marker::<core::convert::Infallible>(|b| {
        serial.push(b);
        Ok(())
    })
    .unwrap();
    let expected = format!("{}\n", marker());
    assert_eq!(serial, expected.as_bytes());
}

/// 0-byte capture (today's SeaBIOS smoke state) must NOT parse as booted.
#[test]
fn empty_serial_log_parses_as_no_guest() {
    let log = write_temp_log(b"");
    let bytes = std::fs::read(&log).unwrap();
    assert!(
        !contains_marker_bytes(&bytes),
        "empty serial log must not contain the marker"
    );
    std::fs::remove_file(&log).ok();
}

/// A guest-written capture must parse as booted, via both bytes and str APIs.
#[test]
fn guest_serial_log_parses_as_booted() {
    let mut capture = b"SeaBIOS noise\n".to_vec();
    write_marker::<core::convert::Infallible>(|b| {
        capture.push(b);
        Ok(())
    })
    .unwrap();
    let log = write_temp_log(&capture);
    let bytes = std::fs::read(&log).unwrap();
    assert!(contains_marker_bytes(&bytes));
    let text = String::from_utf8_lossy(&bytes);
    assert!(contains_marker(&text));
    std::fs::remove_file(&log).ok();
}

/// `smoke.ps1 -ExpectSerialMarker` greps for the same literal; fail here if
/// the Rust contract and the PowerShell assert ever drift apart.
#[test]
fn smoke_script_assert_stays_in_sync_with_contract() {
    let ps1 = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("qemu")
        .join("smoke.ps1");
    let script = std::fs::read_to_string(&ps1).unwrap_or_else(|_| panic!("missing {}", ps1.display()));
    assert!(
        script.contains(marker()),
        "smoke.ps1 must grep for the contract marker {:?}",
        marker()
    );
}

fn write_temp_log(contents: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "baseos-s01-serial-{}-{}.log",
        std::process::id(),
        contents.len()
    ));
    std::fs::write(&path, contents).unwrap();
    path
}
