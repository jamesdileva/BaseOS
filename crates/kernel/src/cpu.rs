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
use super::{EXCEPTION_VECTORS, GDT_CODE, GDT_DATA, GDT_NULL, VECTOR_DE, VECTOR_PF};

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

/// IDT covering the 32 exception vectors; `#DE` + `#PF` are wired this
/// slice (others stay [`IdtEntry::missing`] and would `#GP` if hit — with
/// `cli` and single-threaded bring-up only `#DE` is expected; `#PF` is
/// wired-but-untriggered until the live-injection slice).
/// Guest-only IDT storage; host tests exercise [`IdtEntry`] encoding
/// without instantiating the full table.
#[cfg(not(test))]
#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; EXCEPTION_VECTORS],
}

#[cfg(not(test))]
static mut IDT: Idt = Idt {
    entries: [IdtEntry::missing(); EXCEPTION_VECTORS],
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
/// Guest intentionally does not call this yet (wiring rides with the
/// register-read move); the host tests are the gate, so `dead_code` in
/// guest builds is expected.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn cr3_pml4_base(cr3: u64) -> u64 {
    super::page_table::cr3_decode(cr3)
}

/// `NXE` pin from a live `EFER` value (S05 slice 5).
///
/// Safe pure wrapper over [`crate::page_table::efer_nxe_enabled`] (bit 11)
/// so the `RMBZ nxen=` verdict and host tests share one decode rule.
/// No hardware touched; the live [`read_efer`] (`rdmsr` on `EFER_MSR`)
/// lives beside it in this module (S05 slice 6 register-read move).
/// Guest intentionally does not call this yet; the host tests are the gate,
/// so `dead_code` in guest builds is expected.
#[allow(dead_code)]
#[must_use]
pub(crate) const fn efer_nxe_enabled(efer: u64) -> bool {
    super::page_table::efer_nxe_enabled(efer)
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

// Forward-declared asm stubs (defined in `global_asm!` in the parent).
#[cfg(not(test))]
extern "C" {
    fn baseos_isr_div0();
    fn baseos_isr_pf();
}

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

/// Populate `#DE` (vector 0) + `#PF` (vector 14) and `lidt` the table.
///
/// # Safety
/// Same guest-only contract as [`load_gdt`]: `IDT` static must outlive the
/// CPU, gates target [`baseos_isr_div0`] / [`baseos_isr_pf`] in the kernel
/// code segment (via [`gate_for`]), and `cli` is already in effect so
/// unhandled vectors cannot fire this slice. Host builds are a no-op; e2e
/// proves BOTH gates (`PF ok=1` via the live probe touch, then `EXC ok=1`
/// via the chained `#DE`).
#[cfg(not(test))]
pub(crate) unsafe fn load_idt() {
    let de_handler = baseos_isr_div0 as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_DE] = gate_for(de_handler);
    let pf_handler = baseos_isr_pf as *const () as u64;
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_PF] = gate_for(pf_handler);
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
}
