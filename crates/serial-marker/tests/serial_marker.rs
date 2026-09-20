// Cargo wrapper: executes the canonical S01 harness at `tests/serial_marker.rs`
// (the virtual workspace root owns no test target of its own, so this
// package target includes it; single source of truth stays at `tests/`).

include!("../../../tests/serial_marker.rs");
