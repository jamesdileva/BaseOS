#![cfg_attr(not(test), no_std)]
// SAFETY (unsafe boundary, per `docs/coding-conventions.md`): post-UEFI
// CPU bring-up (`lgdt`/`lidt`/segment reloads), exception entry stubs, and
// COM1 port I/O have no safe equivalent in a `no_std` guest. They live in
// this smallest hardware boundary behind the documented wrappers
// (`load_gdt`, `load_idt`, `bring_up_cpu_and_prove_exception`,
// `exception_handler_rust`). Host `cargo test` never executes guest asm —
// encoding helpers are pure and unit-tested; the asm path is proven
// end-to-end by `tools/qemu/boot-uefi.ps1` (EBS/KERNEL/EXC hard-requires).
#![allow(unsafe_code)]

//! S02 kernel entry + CPU bring-up: receives the S01 [`BootInfo`] handoff,
//! proves the transfer on serial, then (guest-only) crosses
//! `ExitBootServices`, installs a minimal GDT/IDT, and catches an
//! intentional `#DE` so the e2e script can assert a controlled exception.
//!
//! no_std-clean (core only) so the UEFI guest links it today. Host tests
//! cover the serial line contracts and GDT/IDT encoding; the asm bring-up
//! path is gated to `cfg(not(test))` and proven by the OVMF boot script.
#[cfg(test)]
extern crate std;

use baseos_boot_info::{write_str, write_u64_dec, write_u64_hex, BootInfo};

pub mod frame_alloc;
pub mod page_table;

/// Enter the kernel with the boot handoff: validate [`BootInfo`] and emit
/// the one-line entry proof on the caller's byte sink:
/// `KERNEL ok=<0|1> regions=<n> usable=<0x...> fb=<0|1>`.
///
/// `ok` is 1 only when [`BootInfo::is_valid`] passes — a corrupt handoff
/// prints `KERNEL ok=0` and the e2e script fails loudly instead of the
/// guest silently parking. `usable` uses `usable_bytes()` (0 on corrupt
/// header, same documented mask as the BOOTINFO line); `fb` is the gated
/// `framebuffer_opt()` verdict. Never panics; returns the sink's error so
/// the guest can ignore it (COM1 is infallible) while host tests collect
/// bytes.
pub fn kernel_entry<E>(
    boot: &BootInfo,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "KERNEL ok=")?;
    write_u64_dec(emit, u64::from(boot.is_valid()))?;
    write_str(emit, " regions=")?;
    write_u64_dec(emit, u64::from(boot.region_count))?;
    write_str(emit, " usable=")?;
    write_u64_hex(emit, boot.usable_bytes())?;
    write_str(emit, " fb=")?;
    write_u64_dec(emit, u64::from(boot.framebuffer_opt().is_some()))?;
    write_str(emit, "\n")
}

/// COM1 base port (same physical UART the S01 guest programmed; still
/// valid post-`ExitBootServices` because it is plain port I/O).
const COM1_PORT: u16 = 0x3F8;
/// Line-status-register offset; bit 5 (THR empty) gates each write.
const COM1_LSR: u16 = COM1_PORT + 5;
/// Bit 5 of the LSR: transmitter holding register empty.
const LSR_THR_EMPTY: u8 = 0x20;

/// Kernel code selector in the S02 GDT (entry 1; null is entry 0).
const KERNEL_CODE_SEL: u16 = 0x08;
/// Kernel data selector in the S02 GDT (entry 2).
const KERNEL_DATA_SEL: u16 = 0x10;

/// Null descriptor (required GDT slot 0).
const GDT_NULL: u64 = 0;
/// 64-bit kernel code: base 0, limit ignored (L=1), present, executable,
/// readable (`0x00AF9A000000FFFF` — standard long-mode code descriptor).
const GDT_CODE: u64 = 0x00AF_9A00_0000_FFFF;
/// Kernel data: base 0, 4 KiB granularity, present, writable
/// (`0x00CF92000000FFFF` — standard long-mode data descriptor).
const GDT_DATA: u64 = 0x00CF_9200_0000_FFFF;

/// Exception vector for `#DE` (divide error). First IDT slot; no error
/// code pushed by the CPU.
const VECTOR_DE: usize = 0;
/// Exception vector for `#PF` (page fault, Intel SDM Vol 3 §6.15). The CPU
/// pushes a 64-bit error code plus `CR2` holds the faulting address. Live
/// `#PF` injection (sole open #168n4) wires this vector in a later slice;
/// this slice only pins the error-code contract host-side (guest IDT still
/// `#DE`-only, so QEMU behavior is unchanged).
pub const VECTOR_PF: usize = 14;
/// Number of architecturally-defined exception vectors (0..=31) the IDT
/// covers; hardware IRQs (32+) stay masked (`cli`) this slice.
const EXCEPTION_VECTORS: usize = 32;

/// `#PF` error-code decode (Intel SDM Vol 3 Fig 6-8: P/W-U-RSV-I bits 0-4).
/// Pure `core`-only helper so the future guest `#PF` stub and host tests
/// share one contract; guest wiring (IDT gate 14 + `CR2` read + serial line)
/// rides with the live-injection slice. Upper CPU bits (PK/SGX/HLAT, ...)
/// are intentionally NOT decoded this slice — `bits()` preserves the full
/// u64 so nothing is lost; accessors cover only bits 0-4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PfError(u64);

impl PfError {
    /// Rebuild from the CPU-pushed error code (low 5 bits decoded;
    /// full u64 retained via [`PfError::bits`] for future PK/SGX/HLAT).
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Raw bits.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// P=1: protection violation (present page); P=0: non-present page.
    #[must_use]
    pub const fn is_present(self) -> bool {
        self.0 & 1 != 0
    }

    /// W/R=1: faulting access was a write.
    #[must_use]
    pub const fn is_write(self) -> bool {
        self.0 & (1 << 1) != 0
    }

    /// U/S=1: faulting access was from user mode (CPL 3).
    #[must_use]
    pub const fn is_user(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    /// RSVD=1: fault caused by a reserved-bit violation.
    #[must_use]
    pub const fn is_reserved(self) -> bool {
        self.0 & (1 << 3) != 0
    }

    /// I/D=1: faulting access was an instruction fetch.
    #[must_use]
    pub const fn is_instr_fetch(self) -> bool {
        self.0 & (1 << 4) != 0
    }
}

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
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    /// Absent gate (type_attr present-bit clear): a vector that hits this
    /// slot raises `#GP` instead of jumping to garbage.
    const fn missing() -> Self {
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
    fn handler_offset(&self) -> u64 {
        u64::from(self.offset_low)
            | (u64::from(self.offset_mid) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

/// IDT covering the 32 exception vectors; only `#DE` is wired this slice
/// (others stay [`IdtEntry::missing`] and would `#GP` if hit — with `cli`
/// and single-threaded bring-up only `#DE` is expected).
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

/// Write one byte to an ISA I/O port.
///
/// # Safety
/// Caller must pass a valid, writable port for this platform (here: COM1
/// UART registers, already initialized by the S01 guest pre-handoff).
#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags),
    );
}

/// Read one byte from an ISA I/O port.
///
/// # Safety
/// Same contract as [`outb`]: caller passes a readable port (COM1 LSR).
#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags),
    );
    value
}

/// Poll THR-empty, then write one byte to COM1 (post-EBS interrupt-safe
/// path: no firmware, plain port I/O only).
///
/// # Safety
/// COM1 must have been initialized via the S01 guest's `uart_init` before
/// `kernel_entry`; polls a read-only status bit, then writes the register.
unsafe fn com1_write_byte(byte: u8) {
    while inb(COM1_LSR) & LSR_THR_EMPTY == 0 {}
    outb(COM1_PORT, byte);
}

/// Emit `EXC ok=<0|1> vector=<n>\n` directly on COM1 (interrupt context
/// cannot use the caller-supplied `kernel_entry` sink).
///
/// # Safety
/// Requires COM1 initialized (S01 `uart_init` ran before handoff); single-
/// threaded exception context owns the UART exclusively.
unsafe fn write_exc_line(ok: bool, vector: u64) {
    for b in b"EXC ok=" {
        com1_write_byte(*b);
    }
    com1_write_byte(if ok { b'1' } else { b'0' });
    for b in b" vector=" {
        com1_write_byte(*b);
    }
    let mut buf = [0u8; 20];
    let mut n = 0;
    let mut v = vector;
    if v == 0 {
        buf[0] = b'0';
        n = 1;
    } else {
        while v > 0 {
            buf[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
    }
    for i in (0..n).rev() {
        com1_write_byte(buf[i]);
    }
    com1_write_byte(b'\n');
}

/// Rust body of the `#DE` stub: prove the exception was *caught* (serial
/// line) then park — a controlled failure, not a triple-fault reset.
///
/// Never returns; `vector` is the architecturally-defined vector the stub
/// passed (0 for `#DE`).
#[no_mangle]
extern "C" fn exception_handler_rust(vector: u64) -> ! {
    // SAFETY: COM1 was initialized pre-handoff; exception context is
    // single-threaded and exclusive on the UART. `ok=1` means the IDT gate
    // delivered control here — the e2e hard-require for S02 slice 2.
    unsafe {
        write_exc_line(true, vector);
        loop {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Shared `#DE` ISR stub source (AT&T). `x86_64-unknown-uefi` uses the
/// Microsoft x64 ABI: the first `extern "C"` argument is in `RCX` (not
/// `RDI`), the call site must reserve 32 bytes of shadow space, and `RSP`
/// must be 16-byte aligned before `call`. Host tests assert these
/// instructions so the ABI contract cannot silently regress.
macro_rules! de_isr_stub_asm {
    () => {
        r#"
    .global baseos_isr_div0
    baseos_isr_div0:
        xor ecx, ecx
        and rsp, -16
        sub rsp, 32
        call exception_handler_rust
    "#
    };
}

// Guest-only asm: never linked into host `cargo test` builds (the host
// harness links std and must not emit `lidt`/`lgdt`/`div` traps).
#[cfg(not(test))]
core::arch::global_asm!(de_isr_stub_asm!());

/// Load the S02 GDT and reload visible segment registers + CS.
///
/// # Safety
/// Guest-only post-`ExitBootServices` path: `GDT` is a stable static, the
/// far-return reloads CS to [`KERNEL_CODE_SEL`], and data selectors point
/// at [`KERNEL_DATA_SEL`]. Host builds are a no-op (no `lgdt` on the test
/// harness); the OVMF e2e path proves the descriptors by surviving past
/// load into the intentional `#DE`.
#[cfg(not(test))]
unsafe fn load_gdt() {
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

/// Populate `#DE` (vector 0) and `lidt` the exception table.
///
/// # Safety
/// Same guest-only contract as [`load_gdt`]: `IDT` static must outlive the
/// CPU, gate targets [`baseos_isr_div0`] (code segment
/// [`KERNEL_CODE_SEL`]), and `cli` is already in effect so unhandled
/// vectors cannot fire this slice. Host builds are a no-op; e2e proves the
/// gate by catching the intentional `#DE`.
#[cfg(not(test))]
unsafe fn load_idt() {
    let handler = baseos_isr_div0 as *const () as u64;
    let entry = IdtEntry::from_handler(handler, KERNEL_CODE_SEL);
    (*core::ptr::addr_of_mut!(IDT)).entries[VECTOR_DE] = entry;
    let ptr = DescriptorTablePointer {
        limit: (core::mem::size_of::<Idt>() - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
}

// Forward-declared asm stub (defined in `global_asm!` above).
#[cfg(not(test))]
extern "C" {
    fn baseos_isr_div0();
}

/// Guest-only CPU bring-up + controlled `#DE` proof: load GDT, load IDT,
/// execute `xor eax,eax; xor edx,edx; div eax` (divide-by-zero). The `#DE`
/// gate jumps to [`exception_handler_rust`], which emits
/// `EXC ok=1 vector=0` and parks — never returns.
///
/// # Safety
/// Must run post-`ExitBootServices` on the boot CPU with interrupts
/// disabled (`cli` already executed by the guest after a successful EBS)
/// and COM1 initialized. Host `cargo test` builds compile this out.
#[cfg(not(test))]
pub unsafe fn bring_up_cpu_and_prove_exception() -> ! {
    load_gdt();
    load_idt();
    // Intentional #DE: EAX=0 divisor. If the IDT gate is wrong this
    // triple-faults and the e2e script fails loudly (missing EXC line).
    core::arch::asm!("xor eax, eax", "xor edx, edx", "div eax", options(nostack),);
    // Unreachable when #DE fires; park if the trap somehow returns.
    loop {
        core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

/// Host-test stub mirroring the guest signature so link/call sites stay
/// symmetric; never invoked by tests (guest-only proof is e2e).
///
/// # Safety
/// Host `cargo test` only: spins forever and never touches hardware. The
/// real guest body (`cfg(not(test))`) carries the full post-EBS contract.
#[cfg(test)]
pub unsafe fn bring_up_cpu_and_prove_exception() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baseos_boot_info::{Framebuffer, MemoryKind, MemoryRegion};
    use std::vec::Vec;

    fn run(boot: &BootInfo) -> std::string::String {
        let mut out = Vec::new();
        kernel_entry::<core::convert::Infallible>(boot, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        std::string::String::from_utf8(out).unwrap()
    }

    #[test]
    fn valid_handoff_reports_ok_with_map_and_fb() {
        let mut info = BootInfo::new();
        assert!(info.push_region(MemoryRegion::new(0x1000, 4, MemoryKind::Usable)));
        info.has_framebuffer = 1;
        info.framebuffer = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        let text = run(&info);
        assert_eq!(text, "KERNEL ok=1 regions=1 usable=0x4000 fb=1\n");
    }

    #[test]
    fn corrupt_handoff_fails_loudly_with_ok_zero() {
        // A bad magic must print ok=0 (e2e fails loudly), regions still raw,
        // usable masked to 0, fb gated to 0.
        let mut info = BootInfo::new();
        assert!(info.push_region(MemoryRegion::new(0x1000, 4, MemoryKind::Usable)));
        info.has_framebuffer = 1;
        info.framebuffer = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        info.magic = 0;
        let text = run(&info);
        assert_eq!(text, "KERNEL ok=0 regions=1 usable=0x0 fb=0\n");
    }

    #[test]
    fn valid_empty_handoff_reports_ok_zero_counts() {
        let text = run(&BootInfo::new());
        assert_eq!(text, "KERNEL ok=1 regions=0 usable=0x0 fb=0\n");
    }

    #[test]
    fn gdt_descriptors_match_long_mode_layout() {
        // Slot contract the guest `lgdt` + far-return reload depends on:
        // null, present executable code (L=1), present writable data.
        assert_eq!(GDT_NULL, 0);
        assert_eq!(GDT_CODE >> 43 & 1, 1, "code descriptor must be executable");
        assert_eq!(GDT_CODE >> 47 & 1, 1, "code descriptor must be present");
        assert_eq!(
            GDT_CODE >> 53 & 1,
            1,
            "code descriptor must set L (long mode)"
        );
        assert_eq!(GDT_DATA >> 44 & 1, 1, "data descriptor S bit (code/data)");
        assert_eq!(GDT_DATA >> 47 & 1, 1, "data descriptor must be present");
        assert_eq!(KERNEL_CODE_SEL, 0x08);
        assert_eq!(KERNEL_DATA_SEL, 0x10);
    }

    #[test]
    fn idt_de_gate_round_trips_handler_address() {
        // Encoding the guest `lidt` consumes: split offset fields must
        // reassemble to the stub address; type_attr is a present interrupt
        // gate; selector is the kernel code segment.
        let handler = 0x0000_0000_DEAD_BEEF_u64;
        let e = IdtEntry::from_handler(handler, KERNEL_CODE_SEL);
        assert_eq!(e.handler_offset(), handler);
        // Copy packed fields to locals before assert_eq! (E0793: no refs
        // into #[repr(packed)] storage).
        let type_attr = e.type_attr;
        let selector = e.selector;
        let ist = e.ist;
        let reserved = e.reserved;
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
        // Missing entry must not claim present (unhandled vectors #GP).
        let m = IdtEntry::missing();
        let missing_type_attr = m.type_attr;
        assert_eq!(
            missing_type_attr & 0x80,
            0,
            "missing gate present-bit clear"
        );
        assert_eq!(m.handler_offset(), 0);
        assert_eq!(EXCEPTION_VECTORS, 32);
        assert_eq!(VECTOR_DE, 0);
    }

    #[test]
    fn de_isr_stub_passes_vector_per_win64_abi() {
        // `x86_64-unknown-uefi` `extern "C"` is Microsoft x64: first
        // argument in RCX, 32-byte shadow space, 16-byte stack alignment
        // before `call`. Guest and host share `de_isr_stub_asm!` — assert
        // the contract here so a stub edit cannot silently regress it
        // (wrong register produced `EXC ok=1 vector=266751014` on QEMU).
        let stub = de_isr_stub_asm!();
        assert!(
            stub.contains("xor ecx, ecx"),
            "vector must be zeroed in RCX (Win64 first arg), not RDI: {stub}"
        );
        assert!(
            stub.contains("and rsp, -16"),
            "RSP must be 16-byte aligned before call: {stub}"
        );
        assert!(
            stub.contains("sub rsp, 32"),
            "32-byte shadow space required by Win64 ABI: {stub}"
        );
        assert!(
            stub.contains("call exception_handler_rust"),
            "stub must transfer to the Rust body: {stub}"
        );
        assert!(
            !stub.contains("rdi"),
            "RDI is not the Win64 argument register: {stub}"
        );
    }

    #[test]
    fn pf_error_decode_matches_sdm_bits() {
        // #168n4 scoping: pin the P/W-U/RSV/I contract before the guest
        // wires IDT vector 14 (error code + CR2). Bit layout Intel SDM
        // Vol 3 Fig 6-8.
        assert_eq!(VECTOR_PF, 14);
        let not_present_read = PfError::from_bits(0b00000);
        assert!(!not_present_read.is_present());
        assert!(!not_present_read.is_write());
        assert!(!not_present_read.is_user());
        assert!(!not_present_read.is_reserved());
        assert!(!not_present_read.is_instr_fetch());
        let prot_write_user = PfError::from_bits(0b00111);
        assert!(prot_write_user.is_present());
        assert!(prot_write_user.is_write());
        assert!(prot_write_user.is_user());
        assert!(!prot_write_user.is_reserved());
        assert!(!prot_write_user.is_instr_fetch());
        let rsvd_instr = PfError::from_bits(0b11000);
        assert!(!rsvd_instr.is_present());
        assert!(!rsvd_instr.is_write());
        assert!(!rsvd_instr.is_user());
        assert!(rsvd_instr.is_reserved());
        assert!(rsvd_instr.is_instr_fetch());
        assert_eq!(PfError::from_bits(0b10110).bits(), 0b10110);
        // Upper bits preserved raw but ignored by the bit 0-4 accessors
        // (REVIEW #225 gap 2: intentionally truncated this slice).
        let upper = PfError::from_bits(u64::MAX);
        assert_eq!(upper.bits(), u64::MAX);
        assert!(upper.is_present());
        assert!(upper.is_write());
        assert!(upper.is_user());
        assert!(upper.is_reserved());
        assert!(upper.is_instr_fetch());
        let upper_only = PfError::from_bits(!0x1F);
        assert!(!upper_only.is_present());
        assert!(!upper_only.is_write());
        assert!(!upper_only.is_user());
        assert!(!upper_only.is_reserved());
        assert!(!upper_only.is_instr_fetch());
    }
}
