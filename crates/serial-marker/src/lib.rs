//! S01 serial-marker contract: the fixed guest→host proof string.
//!
//! The future UEFI guest writes [`SERIAL_MARKER`] to the serial port; host
//! tooling (`tools/qemu/smoke.ps1 -ExpectSerialMarker`) and the `tests/`
//! harness assert its presence in `target/qemu-smoke-serial.log`, closing
//! the S00 0-byte-serial gap. Core-only so the UEFI crate can depend on
//! this without pulling in `std` (`no_std` outside `cfg(test)`).

#![cfg_attr(not(test), no_std)]

/// Fixed serial marker emitted by the S01 UEFI guest.
///
/// Stability is the point: host scripts grep for exactly this string.
/// Do not change without updating `tools/qemu/smoke.ps1` (`-ExpectSerialMarker`),
/// `tools/qemu/boot-uefi.ps1` (required assert), and the `tests/` harness
/// in the same commit.
pub const SERIAL_MARKER: &str = "BASEOS-S01-BOOT-MARKER";

/// Returns the fixed serial marker.
pub fn marker() -> &'static str {
    SERIAL_MARKER
}

/// Returns the fixed serial marker as bytes.
pub fn marker_bytes() -> &'static [u8] {
    SERIAL_MARKER.as_bytes()
}

/// Emits the marker plus a trailing newline through a byte callback.
///
/// The UEFI guest passes a serial-port writer (e.g. `outb` to COM1);
/// tests pass a `Vec` collector. Generic over the error type so both
/// infallible and fallible sinks work.
pub fn write_marker<E>(mut emit: impl FnMut(u8) -> Result<(), E>) -> Result<(), E> {
    for &b in marker_bytes().iter().chain(core::iter::once(&b'\n')) {
        emit(b)?;
    }
    Ok(())
}

/// Returns true when `haystack` contains the marker bytes.
///
/// Subsequence search over raw bytes (no allocation, works on serial
/// captures that may not be valid UTF-8).
pub fn contains_marker_bytes(haystack: &[u8]) -> bool {
    let needle = marker_bytes();
    haystack.len() >= needle.len() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Returns true when `haystack` contains the marker string.
pub fn contains_marker(haystack: &str) -> bool {
    contains_marker_bytes(haystack.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_is_stable_and_nonempty() {
        assert_eq!(marker(), "BASEOS-S01-BOOT-MARKER");
        assert_eq!(marker_bytes(), b"BASEOS-S01-BOOT-MARKER");
    }

    #[test]
    fn write_marker_emits_marker_plus_newline() {
        let mut out = [0u8; 64];
        let mut n = 0usize;
        write_marker::<core::convert::Infallible>(|b| {
            out[n] = b;
            n += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(&out[..n], b"BASEOS-S01-BOOT-MARKER\n");
    }

    #[test]
    fn contains_marker_bytes_detects_embedded_marker() {
        assert!(contains_marker_bytes(
            b"noise\x00BASEOS-S01-BOOT-MARKER\ntrailing"
        ));
        assert!(!contains_marker_bytes(b""));
        assert!(!contains_marker_bytes(b"BASEOS-S01-BOOT-MARKE"));
        assert!(!contains_marker_bytes(b"no marker here"));
    }

    #[test]
    fn contains_marker_str_wrapper_agrees() {
        assert!(contains_marker("boot ok\nBASEOS-S01-BOOT-MARKER\n"));
        assert!(!contains_marker("empty serial, 0 bytes"));
    }
}
