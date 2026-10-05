//! S07 slice 7a — real-timer IRQ contract (host-testable, no guest wiring yet).
//!
//! Pure `core`-only constants + counter the future PIT/PIC/IDT32 slice will
//! consume. No port I/O, no `lidt`/`sti` here — the guest keeps `cli` in
//! effect until the wiring slice programs the hardware and installs the
//! returning IRQ0 stub (with EOI + `iretq`) first.
//!
//! Contracts pinned here:
//! - `PIT_BASE_HZ = 1_193_182`, [`pit_divisor`] (integer division, `0` Hz
//!   fails closed to `0`), [`TIMER_HZ = 100`] + [`TIMER_DIVISOR`].
//! - PIC remap bases `PICM_OFFSET = 0x20` / `PICS_OFFSET = 0x28`, so IRQ0
//!   lands on [`VECTOR_TIMER = 32`] (first maskable vector past the 32
//!   exception vectors).
//! - [`TimerTicks`] saturating HW-tick counter the future IRQ0 handler
//!   increments (one `tick()` per IRQ0). Wraps nothing, saturates at
//!   `u64::MAX`; the scheduler slice will drain it via `take()` and feed
//!   `tick_gated` once per HW tick (dispatch-once on expiry, never a tight
//!   re-tick loop — see `sched.rs` residual 2).
//!
//! Failure model: all pure, never panics, `core`-only.

/// PIT input clock (Intel 8254, 1.193182 MHz).
pub const PIT_BASE_HZ: u32 = 1_193_182;

/// Target timer IRQ rate (100 Hz → 10 ms period).
pub const TIMER_HZ: u32 = 100;

/// PIC master remap base (IRQ0 lands on VECTOR_TIMER).
pub const PICM_OFFSET: u8 = 0x20;
/// PIC slave remap base.
pub const PICS_OFFSET: u8 = 0x28;

/// IDT vector for PIT IRQ0 (first maskable vector past exceptions 0..=31).
pub const VECTOR_TIMER: usize = 32;

/// PIT channel-0 data port.
pub const PIT_CH0: u16 = 0x40;
/// PIT mode/command port.
pub const PIT_CMD: u16 = 0x43;

/// PIT mode byte for channel 0, lobyte/hibyte, mode 3 square-wave.
pub const PIT_MODE_CH0_SQUARE: u8 = 0x36;

/// Integer PIT divisor for `hz` (`PIT_BASE_HZ / hz`); `0` Hz fails closed
/// to `0` (caller must refuse a zero divisor — programming it latches
/// `0x0000` = 65536, not "stop").
#[must_use]
pub const fn pit_divisor(hz: u32) -> u16 {
    if hz == 0 {
        return 0;
    }
    (PIT_BASE_HZ / hz) as u16
}

/// Divisor for [`TIMER_HZ`] (1193182 / 100 = 11931).
pub const TIMER_DIVISOR: u16 = pit_divisor(TIMER_HZ);

/// Saturating HW-tick counter: the future IRQ0 handler calls [`tick`]
/// once per interrupt; the scheduler drain calls [`take`].
///
/// [`tick`]: TimerTicks::tick
/// [`take`]: TimerTicks::take
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimerTicks {
    pending: u64,
}

impl TimerTicks {
    /// New empty counter.
    #[must_use]
    pub const fn new() -> Self {
        Self { pending: 0 }
    }

    /// Record one HW tick (saturates at `u64::MAX`).
    pub const fn tick(&mut self) {
        self.pending = self.pending.saturating_add(1);
    }

    /// Pending (undrained) ticks.
    #[must_use]
    pub const fn pending(self) -> u64 {
        self.pending
    }

    /// Drain pending ticks (returns count, resets to 0).
    pub fn take(&mut self) -> u64 {
        let n = self.pending;
        self.pending = 0;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_divisor_matches_100hz_contract() {
        assert_eq!(PIT_BASE_HZ, 1_193_182);
        assert_eq!(TIMER_HZ, 100);
        assert_eq!(pit_divisor(100), 11931);
        assert_eq!(TIMER_DIVISOR, 11931);
        assert_eq!(TIMER_DIVISOR as u32 * TIMER_HZ, 1_193_100);
    }

    #[test]
    fn pit_divisor_zero_hz_fails_closed() {
        assert_eq!(pit_divisor(0), 0);
    }

    #[test]
    fn pic_offsets_land_irq0_on_vector_32() {
        assert_eq!(PICM_OFFSET, 0x20);
        assert_eq!(PICS_OFFSET, 0x28);
        assert_eq!(VECTOR_TIMER, 32);
        assert_eq!(PIT_CH0, 0x40);
        assert_eq!(PIT_CMD, 0x43);
        assert_eq!(PIT_MODE_CH0_SQUARE, 0x36);
    }

    #[test]
    fn timer_ticks_drain_and_saturate() {
        let mut t = TimerTicks::new();
        assert_eq!(t.pending(), 0);
        assert_eq!(t.take(), 0);
        t.tick();
        t.tick();
        t.tick();
        assert_eq!(t.pending(), 3);
        assert_eq!(t.take(), 3);
        assert_eq!(t.pending(), 0);
        // Saturate, never wrap.
        t.pending = u64::MAX - 1;
        t.tick();
        t.tick();
        t.tick();
        assert_eq!(t.pending(), u64::MAX);
    }
}
