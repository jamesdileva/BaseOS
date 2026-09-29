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
//! hardware. Caller contract for the unsafe loads (documented, not
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
    pub(crate) const fn from_handler(handler: u64, selector: u16) -> Self {
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
#[allow(dead_code)]
#[must_use]
pub(crate) const fn probe_addr_valid(addr: u64) -> bool {
    addr & 0xFFF == 0 && addr < 0x0000_8000_0000_0000 && addr > 0x1_0000_0000
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
}
