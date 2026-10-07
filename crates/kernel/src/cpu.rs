//! Safe CPU/table boundary (S05 slice 3).
//!
//! Privileged descriptor loads (`lgdt`/`lidt` + segment reloads) have no
//! safe equivalent in a `no_std` guest, so the `unsafe` stays confined to
//! the two bodies ([`load_gdt`] / [`load_idt`]) below plus the stable
//! statics they consume. Everything else in this module is a **safe**
//! wrapper or pure helper: [`gate_for`] pins the kernel code selector so
//! callers cannot wire a gate into the wrong segment, and
//! [`probe_addr_valid`] pins the live-`#PF` guard contract (canonical
//! low-half, 4 KiB-aligned, above firmware-mapped RAM) without touching
//! hardware. Slice 5 adds [`cr3_pml4_base`] / [`efer_nxe_enabled`] pure
//! decode wrappers over `page_table` so probe code shares one rule.
//! Slice 6 moves the live register reads ([`read_cr3`] / [`read_efer`],
//! ex-boot-uefi D7/D8) beside the decode wrappers so one module owns the
//! read + decode pair; `boot-uefi` probes call them via `crate::cpu::read_*`.
//! Caller contract for the unsafe loads (documented, not
//! compiler-enforced): post-`ExitBootServices` boot CPU, `cli` in effect,
//! statics outlive use.

use super::KERNEL_CODE_SEL;
#[cfg(not(test))]
use super::{GDT_CODE, GDT_DATA, GDT_NULL, VECTOR_DE, VECTOR_PF};
#[cfg(not(test))]
use crate::timer::VECTOR_TIMER;

/// Minimal long-mode GDT: null, kernel code (`0x08`), kernel data (`0x10`).
/// Static so `lgdt` keeps a stable base after the function returns.
/// Guest-only (`cfg(not(test))`) — host tests cover the descriptor constants
/// without constructing the table.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Gdt {
    entries: [u64; 3],
}

#[cfg(not(test))]
static mut GDT: Gdt = Gdt {
    entries: [GDT_NULL, GDT_CODE, GDT_DATA],
};

/// Packed `lgdt` operand: 16-bit limit + 64-bit base (Intel SDM format).
#[cfg(not(test))]
#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// One 16-byte x86-64 IDT gate descriptor.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub(crate) struct IdtEntry {
    pub(crate) offset_low: u16,
    pub(crate) selector: u16,
    pub(crate) ist: u8,
    pub(crate) type_attr: u8,
    pub(crate) offset_mid: u16,
    pub(crate) offset_high: u32,
    pub(crate) reserved: u32,
}

impl IdtEntry {
    /// Absent gate (type_attr present-bit clear): a vector that hits this
    /// slot raises `#GP` instead of jumping to garbage.
    pub(crate) const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// Interrupt gate (type `0xE`, DPL 0, present) targeting `handler`
    /// with code selector `selector` (IST 0 — same stack this slice).
    /// Private to this module so [`gate_for`] is the only in-crate path to
    /// a present gate (REVIEW #240 R1: the `KERNEL_CODE_SEL` pin is
    /// compiler-enforced, not convention — `lib.rs` tests reach encoding
    /// via `gate_for`).
    const fn from_handler(handler: u64, selector: u16) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist: 0,
            type_attr: 0x8E,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    /// Reassemble the 64-bit handler offset from the split gate fields
    /// (host-testable encoding round-trip; guest `lidt` consumes raw bytes).
    #[cfg(test)]
    pub(crate) fn handler_offset(&self) -> u64 {
        u64::from(self.offset_low)
            | (u64::from(self.offset_mid) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

/// IDT covering the 32 exception vectors plus the timer IRQ0 vector 32
/// (S07 slice13); `#DE` + `#PF` + timer are wired this slice (others stay
/// [`IdtEntry::missing`] and would `#GP` if hit — with `cli` and
/// single-threaded bring-up only `#DE` is expected; `#PF` is
/// wired-but-untriggered until the live-injection slice; timer is
/// wired-but-masked until the PIC/PIT/`sti` slice).
/// Guest-only IDT storage; host tests exercise [`IdtEntry`] encoding
/// without instantiating the full table.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; IDT_VECTORS],
}

#[cfg(not(test))]
static mut IDT: Idt = Idt {
    entries: [IdtEntry::missing(); IDT_VECTORS],
};

/// Build a present DPL0 interrupt gate for `handler` in the kernel code
/// segment.
///
/// Safe wrapper: pins [`KERNEL_CODE_SEL`] so callers cannot wire a gate
/// into the null/data segment (which would `#GP` on first delivery).
/// Pure encoding — no hardware touched; [`load_idt`] consumes the result.
pub(crate) const fn gate_for(handler: u64) -> IdtEntry {
    IdtEntry::from_handler(handler, KERNEL_CODE_SEL)
}

/// Check the live-`#PF` probe-guard contract without touching hardware:
///
/// Safe pure helper sharing one contract between the guest probe constant
/// and host tests (mirrors `pf_probe_addr_is_canonical_unmapped_guard`).
/// Guest intentionally does not call this (no panic path post-EBS); the
/// host tests are the gate, so `dead_code` in guest builds is expected.
///
/// Single source (REVIEW #242 nit): the raw canonical/aligned/floor
/// predicate lives ONLY here — `lib.rs` probe-guard tests assert via this
/// function (table-driven), never with duplicated raw `& 0xFFF` / range
/// asserts, so the contract cannot drift across copies.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn probe_addr_valid(addr: u64) -> bool {
    addr & 0xFFF == 0 && addr < 0x0000_8000_0000_0000 && addr > 0x1_0000_0000
}

/// PML4 base from a live `CR3` value (S05 slice 5).
///
/// Safe pure wrapper over [`crate::page_table::cr3_decode`] so guest probe
/// code (`probe_paging`, `probe_live_flags_for`, `probe_rmbz` in
/// `boot-uefi`) and host tests share one decode rule beside the other
/// `cpu` wrappers. No hardware touched; the live [`read_cr3`] (`mov {}, cr3`)
/// lives beside it in this module (S05 slice 6 register-read move).
/// Guest calls this with the live `read_cr3()` value (slice 7 wiring);
/// host tests pin agreement with `page_table`.
#[inline(always)]
#[must_use]
pub const fn cr3_pml4_base(cr3: u64) -> u64 {
    super::page_table::cr3_decode(cr3)
}

/// `NXE` pin from a live `EFER` value (S05 slice 5).
///
/// Safe pure wrapper over [`crate::page_table::efer_nxe_enabled`] (bit 11)
/// so the `RMBZ nxen=` verdict and host tests share one decode rule.
/// No hardware touched; the live [`read_efer`] (`rdmsr` on `EFER_MSR`)
/// lives beside it in this module (S05 slice 6 register-read move).
/// Guest calls this with the live `read_efer()` value (slice 7 wiring);
/// host tests pin agreement with `page_table`.
#[inline(always)]
#[must_use]
pub const fn efer_nxe_enabled(efer: u64) -> bool {
    super::page_table::efer_nxe_enabled(efer)
}

/// Mask maskable interrupts on the boot CPU (`cli`, S06 preempt-IRQ gate).
///
/// Guest-only contract: post-`ExitBootServices`, CPL0 boot/exception path.
/// `cli` clears IF (hence NO `preserves_flags` on the asm — same rule as the
/// `efi_main` cli site). Host builds are a no-op stub (the harness must
/// never execute `cli`).
///
/// # Safety
/// Same contract as [`read_cr3`]: boot CPU only, post-EBS. Callers that need
/// a scoped critical section should use [`IrqGuard`] instead of raw `cli`.
#[cfg(not(test))]
pub unsafe fn cli() {
    core::arch::asm!("cli", options(nomem, nostack));
}

/// Host-test stub: no-op (never touches flags).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn cli() {}

/// Enable maskable interrupts on the boot CPU (`sti`, S07 slice14 timer
/// enable).
///
/// Guest-only contract: post-`ExitBootServices`, CPL0 boot path, IDT vector
/// 32 wired to the returning IRQ0 stub with PIC remapped + PIT programmed
/// first (otherwise the first tick vectors into a missing gate or the PIT
/// never fires). `sti` sets IF (hence NO `preserves_flags` — same rule as
/// [`cli`]). Host builds are a no-op stub (the harness must never execute
/// `sti`).
///
/// # Safety
/// Same contract as [`cli`]: boot CPU only, post-EBS. The timer wiring
/// slice calls this exactly once after the PIC/PIT program sequence.
#[cfg(not(test))]
pub unsafe fn sti() {
    core::arch::asm!("sti", options(nomem, nostack));
}

/// Host-test stub: no-op (never touches flags).
///
/// # Safety
/// Host `cargo test` only: no-op.
#[cfg(test)]
pub unsafe fn sti() {}

/// Preempt-IRQ gate token (S06 slice28 / task #19): proves the holder runs
/// with maskable IRQs masked (`cli` in effect) across a scheduler
/// pick-then-mark critical section (`dispatch` / `exit_current` pick the next
/// `Ready` thread and flip states — two preempted picks without a gate can
/// both observe the same `Ready` target and double-dispatch it).
///
/// Zero-cost proof token on ALL builds (host and guest): constructing it
/// executes NO `cli` itself. The live `cli` is established ONCE in
/// `efi_main` post-EBS (the pre-existing `asm!("cli")` site, still the only
/// `cli` in the tree); every gated scheduler site runs downstream of it on
/// the single-threaded boot thread, so holding the token means holding that
/// critical section. (A token that re-executed `cli` per dispatch would
/// fault host `cargo test`: `boot-uefi` links `baseos_kernel` as a
/// dependency, so the dependency's `cfg(test)` no-op never applies there —
/// the same reason the live-swap stubs stay guest-only. The token must be
/// asm-free to keep host and guest call sites identical.)
///
/// `Drop` is a no-op (never `sti`: enabling interrupts belongs to the timer
/// wiring slice, which programs the PIC/PIT and wires IDT 32 first — a
/// `Drop`-time `sti` would enable IRQs at an arbitrary pick site. The paired
/// [`sti`] lands with that slice, after the handler is wired).
///
/// Contract: create the guard BEFORE the pick and hold the `&` across the
/// whole pick + state-mark (`Scheduler::dispatch_gated` /
/// `exit_current_gated` take `&IrqGuard`, so an ungated call site fails to
/// compile where the gated path is required).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrqGuard {
    _private: (),
}

impl IrqGuard {
    /// Enter the IRQ-masked critical section: proof token only (no asm on
    /// any build — see the type docs for why the live `cli` stays in
    /// `efi_main` instead of here).
    ///
    /// # Safety
    /// Same contract as [`cli`]: boot CPU only, post-EBS on the
    /// single-threaded boot/exception path with `cli` already in effect
    /// (established by `efi_main`). Host `cargo test` never executes
    /// privileged instructions through this token.
    #[must_use]
    pub unsafe fn enter() -> Self {
        Self { _private: () }
    }
}
/// Read the live `CR3` (PML4 base + flags) on the boot CPU (S05 slice 6).
///
/// Moved from `boot-uefi` (D7) so the privileged register read lives
/// behind the `cpu` boundary with the decode rule ([`cr3_pml4_base`]).
/// Pure register read; valid on x86-64 at CPL0 (the guest runs with
/// privilege post-`ExitBootServices`).
///
/// # Safety
/// Boot CPU only, post-`ExitBootServices` on the single-threaded
/// boot/exception path. Host `cargo test` never calls this (would fault
/// outside the guest); host coverage is the `unsafe fn` shape pin below.
pub unsafe fn read_cr3() -> u64 {
    let value: u64;
    core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    value
}

/// Read the live `EFER` (Extended Feature Enable Register) on the boot CPU
/// (S05 slice 6).
///
/// Moved from `boot-uefi` (D8) so the `rdmsr` read lives behind the `cpu`
/// boundary with the decode rule ([`efer_nxe_enabled`]). Bit 11 (`NXE`)
/// gates whether `NO_EXEC`/bit 63 in a PTE is legal or MBZ (#139 note 1).
///
/// # Safety
/// Same contract as [`read_cr3`]: CPL0 guest, boot thread post-`EBS`;
/// `ECX` is pinned to [`crate::page_table::EFER_MSR`]. Host tests never
/// call this; host coverage is the `unsafe fn` shape pin below.
pub unsafe fn read_efer() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdmsr",
        in("ecx") super::page_table::EFER_MSR,
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// TSC warm-up iterations before a latency run (S07 slice52, #21 R3).
///
/// Warms the branch predictor / i-cache on the `lfence; rdtsc` path so the
/// first recorded sample is not a cold outlier. Discarded reads only; the
/// guest runs this before filling `SWITCH_LATENCIES`.
pub const TSC_WARMUP_ITERS: usize = 128;

/// Raw `rdtsc` read (S07 slice52, #21 R3).
///
/// Unfenced: callers that need ordering must use [`read_tsc_fenced`].
/// User-mode legal on x86-64; host tests may call this.
///
/// # Safety
/// No privilege requirement, but the value is only meaningful on x86-64
/// with a running TSC. Guest: post-EBS boot thread. Host: test harness.
pub unsafe fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "rdtsc",
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Fenced `rdtsc` read (S07 slice52, #21 R3): `lfence` before and after
/// so the timestamp brackets exactly the intended region and prior loads
/// cannot bleed into the sample.
///
/// # Safety
/// Same contract as [`read_tsc`].
pub unsafe fn read_tsc_fenced() -> u64 {
    let lo: u32;
    let hi: u32;
    core::arch::asm!(
        "lfence",
        "rdtsc",
        "lfence",
        out("eax") lo,
        out("edx") hi,
        options(nomem, nostack, preserves_flags),
    );
    (u64::from(hi) << 32) | u64::from(lo)
}

/// Saturating TSC delta (S07 slice52, #21 R3): `end - start`, floored at 0
/// so a reordered/coarse pair never underflows; pure so host tests pin it.
#[inline(always)]
#[must_use]
pub const fn tsc_delta(start: u64, end: u64) -> u64 {
    end.saturating_sub(start)
}

/// Empty-pair overhead floor (S07 slice52, #21 R3): two back-to-back
/// fenced reads with no work between; the delta is the per-sample
/// measurement overhead the guest subtracts (saturating) from raw switch
/// deltas before recording into `LatencySamples`.
///
/// # Safety
/// Same contract as [`read_tsc_fenced`].
pub unsafe fn empty_tsc_pair() -> u64 {
    let a = read_tsc_fenced();
    let b = read_tsc_fenced();
    tsc_delta(a, b)
}

/// Discard `TSC_WARMUP_ITERS` fenced reads (S07 slice52, #21 R3).
///
/// # Safety
/// Same contract as [`read_tsc_fenced`].
pub unsafe fn warm_tsc() {
    let mut i = 0usize;
    while i < TSC_WARMUP_ITERS {
        let _ = read_tsc_fenced();
        i += 1;
    }
}

// Forward-declared asm stubs (defined in `global_asm!` in the parent).
#[cfg(not(test))]
extern "C" {
    fn baseos_isr_div0();
    fn baseos_isr_pf();
    /// S07 slice13: timer IRQ0 stub (defined in `timer.rs` `global_asm!`).
    /// Wired into the IDT by [`load_idt`] below; still `cli` so it never
    /// fires until the PIC/PIT/`sti` slice lands.
    fn baseos_isr_timer();
}

/// Number of IDT vectors the guest table covers (S07 slice13).
///
/// 32 exception vectors (0..=31) plus the timer IRQ0 vector 32
/// ([`crate::timer::VECTOR_TIMER`]). The timer gate is wired but never
/// fires this slice (guest keeps `cli`, no PIC/PIT programming yet), so
/// QEMU behavior is unchanged. Un-gated (all builds) so host tests pin the
/// coverage without instantiating the guest-only table. Single-sourced off
/// [`super::EXCEPTION_VECTORS`] (32 exception vectors + timer vector 32).
const IDT_VECTORS: usize = super::EXCEPTION_VECTORS + 1;

/// Load the S02 GDT and reload visible segment registers + CS.
///
/// # Safety
/// Guest-only post-`ExitBootServices` path: `GDT` is a stable static, the
/// far-return reloads CS to [`KERNEL_CODE_SEL`], and data selectors point
/// at the kernel data selector. Host builds are a no-op (no `lgdt` on the
/// test harness); the OVMF e2e path proves the descriptors by surviving past
/// load into the intentional `#DE`.
#[cfg(not(test))]
pub(crate) unsafe fn load_gdt() {
    use super::KERNEL_DATA_SEL;
    let ptr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Gdt>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };
    core::arch::asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
    core::arch::asm!(
        "mov ss, {data:x}",
        "mov ds, {data:x}",
        "mov es, {data:x}",
        "xor eax, eax",
        "mov fs, ax",
        "mov gs, ax",
        "push {code}",
        "lea rax, [rip + 771f]",
        "push rax",
        "retfq",
        "771:",
        data = in(reg) KERNEL_DATA_SEL,
        code = in(reg) u64::from(KERNEL_CODE_SEL),
        options(preserves_flags),
    );
}

/// Populate `#DE` (vector 0) + `#PF` (vector 14) + timer IRQ0 (vector 32)
/// and `lidt` the table.
///
/// The timer gate is a present DPL0 interrupt gate (type `0xE`, same
/// [`gate_for`] encoding as the exception gates): on delivery the CPU
/// clears IF in hardware, which is what makes the
/// [`crate::timer::timer_irq_rust`] `IrqGuard::enter()` proof true on
/// entry. Still `cli` with no PIC/PIT programming this slice, so the timer
/// gate never fires and QEMU behavior is unchanged.
///
/// # Safety
/// Same guest-only contract as [`load_gdt`]: `IDT` static must outlive the
/// CPU, gates target [`baseos_isr_div0`] / [`baseos_isr_pf`] /
/// [`baseos_isr_timer`] in the kernel code segment (via [`gate_for`]), and
/// `cli` is already in effect so unhandled vectors cannot fire this slice.
/// Host builds are a no-op; e2e proves BOTH exception gates (`PF ok=1` via
/// the live probe touch, then `EXC ok=1` via the chained `#DE`).
#[cfg(not(test))]
pub(crate) unsafe fn load_idt() {
    let de_handler = baseos_isr_div0 as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_DE] = gate_for(de_handler);
    let pf_handler = baseos_isr_pf as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_PF] = gate_for(pf_handler);
    let timer_handler = baseos_isr_timer as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_TIMER] = gate_for(timer_handler);
    let ptr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Idt>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_for_pins_kernel_code_selector() {
        // Safe wrapper must pin KERNEL_CODE_SEL so a gate can never target
        // the null/data segment; encoding still round-trips the handler.
        let handler = 0x0000_0000_DEAD_BEEF_u64;
        let e = gate_for(handler);
        assert_eq!(e.handler_offset(), handler);
        let (selector, type_attr, ist, reserved) = (e.selector, e.type_attr, e.ist, e.reserved);
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(selector, 0x08);
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
    }

    #[test]
    fn timer_gate_uses_interrupt_gate_covering_vector_32() {
        // S07 slice13: the wiring slice wires VECTOR_TIMER through the same
        // `gate_for` path as the exception gates — a present DPL0 interrupt
        // gate (type 0xE, which clears IF in HW so the IrqGuard entry-proof
        // holds), in the kernel code segment, with the table covering the
        // vector. Still `cli` + no PIC/PIT this slice, so QEMU is unchanged.
        use super::super::timer::VECTOR_TIMER;
        assert_eq!(VECTOR_TIMER, 32);
        assert_eq!(IDT_VECTORS, 33);
        // `black_box` keeps the coverage assert runtime (not
        // const-evaluable) so `clippy -D warnings`
        // (`assertions_on_constants`) stays clean — same pattern as the
        // probe-guard test in `lib.rs`.
        assert!(core::hint::black_box(VECTOR_TIMER) < IDT_VECTORS);
        let handler = 0x0000_0000_1234_5678_u64;
        let e = gate_for(handler);
        assert_eq!(e.handler_offset(), handler);
        let (selector, type_attr, ist, reserved) = (e.selector, e.type_attr, e.ist, e.reserved);
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(
            type_attr & 0x0F,
            0x0E,
            "gate type must be 0xE (IF cleared by HW)"
        );
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
    }

    #[test]
    fn probe_addr_valid_matches_guard_contract() {
        use super::super::PF_PROBE_ADDR;
        // The live-probe constant must pass its own gate; neighbors that
        // break alignment / canonicity / RAM-floor must fail.
        assert!(probe_addr_valid(PF_PROBE_ADDR));
        assert_eq!(PF_PROBE_ADDR, 0x0000_5000_0000_0000);
        assert!(!probe_addr_valid(PF_PROBE_ADDR + 1), "unaligned must fail");
        assert!(!probe_addr_valid(0x0000_8000_0000_0000), "hole must fail");
        assert!(!probe_addr_valid(0x8000_0000), "firmware RAM must fail");
        assert!(!probe_addr_valid(0), "null must fail");
    }

    #[test]
    fn cr3_pml4_base_agrees_with_page_table() {
        // S05 slice 5: safe wrapper must match `page_table::cr3_decode` on
        // flag-masked, zero, and all-ones inputs (PML4 base = addr bits).
        use super::super::page_table::{cr3_decode, PTE_ADDR_MASK};
        for cr3 in [0u64, 0x7F00_1000, 0x7F00_1003, u64::MAX] {
            assert_eq!(cr3_pml4_base(cr3), cr3_decode(cr3));
        }
        assert_eq!(cr3_pml4_base(0xFFFF_FFFF_FFFF_FFFF), PTE_ADDR_MASK);
        assert_eq!(cr3_pml4_base(0x7F00_1000), 0x7F00_1000);
    }

    #[test]
    fn efer_nxe_enabled_agrees_with_page_table() {
        // S05 slice 5: safe wrapper must match
        // `page_table::efer_nxe_enabled` (bit 11) exactly.
        use super::super::page_table::{efer_nxe_enabled as raw_nxe, EFER_MSR, EFER_NXE_BIT};
        assert_eq!(EFER_MSR, 0xC000_0080);
        for efer in [0u64, EFER_NXE_BIT, !EFER_NXE_BIT, u64::MAX] {
            assert_eq!(efer_nxe_enabled(efer), raw_nxe(efer));
        }
        assert!(efer_nxe_enabled(EFER_NXE_BIT));
        assert!(!efer_nxe_enabled(0));
    }

    #[test]
    fn cr3_efer_wrappers_are_safe_without_unsafe_block() {
        // S05 slice 5: the decode wrappers are safe pure `const fn` — bind
        // to the plain `fn` type to prove safeness at compile time (an
        // `unsafe fn` would not coerce), mirroring the UART pin in lib.rs.
        let cr3_base: fn(u64) -> u64 = cr3_pml4_base;
        let nxe: fn(u64) -> bool = efer_nxe_enabled;
        let probe: fn(u64) -> bool = probe_addr_valid;
        let _ = (cr3_base, nxe, probe);
    }

    #[test]
    fn live_register_reads_are_unsafe_fn() {
        // S05 slice 6: the moved `read_cr3` / `read_efer` must stay
        // `unsafe fn` (privileged `mov {}, cr3` / `rdmsr`) — bind to the
        // `unsafe fn` type to prove callers need an `unsafe` block. A
        // safe-ified signature would fail to coerce here. Never called
        // (would fault outside the guest).
        let cr3: unsafe fn() -> u64 = read_cr3;
        let efer: unsafe fn() -> u64 = read_efer;
        let _ = (cr3, efer);
    }

    #[test]
    fn irq_gate_is_unsafe_enter_and_host_noop() {
        // S06 preempt-IRQ gate (task #19): `cli` + `IrqGuard::enter` must
        // stay `unsafe fn` (privileged `cli` on the guest) — bind to the
        // `unsafe fn` type so a safe-ified signature fails to coerce.
        // `enter` is a pure token on every build (no asm — the live `cli`
        // stays once in `efi_main`), so constructing it on the host never
        // touches flags; the guest runs it downstream of `cli`.
        let cli_fn: unsafe fn() = cli;
        let enter: unsafe fn() -> IrqGuard = IrqGuard::enter;
        let _ = (cli_fn, enter);
        let gate = unsafe { IrqGuard::enter() };
        let gate2 = gate;
        assert_eq!(gate, gate2, "guard is a Copy token on the host");
        unsafe { cli() };
    }

    #[test]
    fn sti_gate_is_unsafe_fn_and_host_noop() {
        // S07 slice14: `sti` is the paired enable for the timer IRQ0 path —
        // must stay `unsafe fn` (privileged, same as `cli`) so callers need
        // an `unsafe` block. Host stub is a no-op (never touches flags).
        let sti_fn: unsafe fn() = sti;
        let _ = sti_fn;
        unsafe { sti() };
    }

    #[test]
    fn tsc_reads_are_unsafe_fn_and_delta_saturates() {
        // S07 slice52 (#21 R3): fenced/unfenced reads stay `unsafe fn`;
        // `tsc_delta` is a pure saturating `const fn` (reorder -> 0).
        let raw: unsafe fn() -> u64 = read_tsc;
        let fenced: unsafe fn() -> u64 = read_tsc_fenced;
        let pair: unsafe fn() -> u64 = empty_tsc_pair;
        let warm: unsafe fn() = warm_tsc;
        let _ = (raw, fenced, pair, warm);
        assert_eq!(TSC_WARMUP_ITERS, 128);
        let d: fn(u64, u64) -> u64 = tsc_delta;
        assert_eq!(d(10, 25), 15);
        assert_eq!(d(25, 25), 0);
        assert_eq!(d(25, 10), 0);
        assert_eq!(d(0, u64::MAX), u64::MAX);
    }

    #[test]
    fn fenced_tsc_is_monotonic_with_finite_empty_floor() {
        // S07 slice52 (#21 R3): warm-up completes, back-to-back fenced
        // pairs are monotonic with a finite floor. No tight upper bound:
        // hosted TSC granularity varies (TCG/WHPX comparison is C2).
        unsafe {
            warm_tsc();
            let a = read_tsc_fenced();
            let b = read_tsc_fenced();
            assert!(b >= a);
            assert_eq!(tsc_delta(a, b), b - a);
            let mut worst = 0u64;
            let mut i = 0usize;
            while i < 16 {
                let floor = empty_tsc_pair();
                assert!(floor <= 10_000_000, "empty pair must stay finite");
                if floor > worst {
                    worst = floor;
                }
                i += 1;
            }
            let _ = worst;
        }
    }
}
