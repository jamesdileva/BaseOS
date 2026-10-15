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

/// Safe CPU/table boundary: privileged `lgdt`/`lidt` confined to
/// `cpu::load_gdt`/`load_idt`; safe callers use `cpu::gate_for` and
/// `cpu::probe_addr_valid` without an `unsafe` block.
pub mod cpu;
pub mod frame_alloc;
pub mod latency;
pub mod page_table;
pub mod sched;
pub mod sync;
pub mod thread;
pub mod timer;
/// Safe COM1 port-I/O boundary: raw `in`/`out` confined to private
/// `uart::outb`/`inb` bodies; safe callers use `uart::com1_write_byte` and
/// the `write_*` line helpers without an `unsafe` block.
pub mod uart;

/// Re-exported for host tests so they keep one owner for the gate encoding
/// contract (`cpu::IdtEntry` is the canonical path; this alias avoids
/// churning every `super::*` test import). Test-only: guest code names
/// `cpu::` directly.
#[cfg(test)]
pub(crate) use cpu::IdtEntry;
use uart::{write_exc_line, write_pf_line, write_pfcpl_line, write_pfus_line};
#[cfg(not(test))]
use uart::{write_tick_line, write_tr_line};

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
/// Lives here (not in `uart`) so the boundary-shape pin test names one
/// owner; `uart` imports these.
pub(crate) const COM1_PORT: u16 = 0x3F8;
/// Line-status-register offset; bit 5 (THR empty) gates each write.
pub(crate) const COM1_LSR: u16 = COM1_PORT + 5;
/// Bit 5 of the LSR: transmitter holding register empty.
pub(crate) const LSR_THR_EMPTY: u8 = 0x20;

/// Kernel code selector in the S02 GDT (entry 1; null is entry 0).
const KERNEL_CODE_SEL: u16 = 0x08;
/// Kernel data selector in the S02 GDT (entry 2).
const KERNEL_DATA_SEL: u16 = 0x10;
/// User code selector (GDT entry 3, RPL 3): `3 * 8 | 3 = 0x1B`. S09 slice2c
/// pins the descriptor/selector only — no CPL3 entry (`iretq`/TSS/user
/// stacks) yet, so the guest still runs ring 0 on the kernel selectors.
/// Slice2m consumes these in the host-only `iretq` frame builder.
#[allow(dead_code)]
pub(crate) const USER_CODE_SEL: u16 = 0x1B;
/// User data selector (GDT entry 4, RPL 3): `4 * 8 | 3 = 0x23`. Same
/// deferred-entry contract as [`USER_CODE_SEL`].
#[allow(dead_code)]
pub(crate) const USER_DATA_SEL: u16 = 0x23;

/// Null descriptor (required GDT slot 0).
const GDT_NULL: u64 = 0;
/// 64-bit kernel code: base 0, limit ignored (L=1), present, executable,
/// readable (`0x00AF9A000000FFFF` — standard long-mode code descriptor).
const GDT_CODE: u64 = 0x00AF_9A00_0000_FFFF;
/// Kernel data: base 0, 4 KiB granularity, present, writable
/// (`0x00CF92000000FFFF` — standard long-mode data descriptor).
const GDT_DATA: u64 = 0x00CF_9200_0000_FFFF;
/// User 64-bit code (DPL 3): kernel code encoding with the DPL field
/// (bits 45-46) raised `0 -> 3` (`0x9A -> 0xFA` access byte), so ring-3
/// callers can execute it once a CPL3 entry path exists. Pinned host-side
/// this slice; the guest GDT grows to 5 entries but still enters ring 0.
const GDT_USER_CODE: u64 = 0x00AF_FA00_0000_FFFF;
/// User data (DPL 3): kernel data encoding with DPL raised `0 -> 3`
/// (`0x92 -> 0xF2` access byte), present + writable. Same deferred-entry
/// contract as [`GDT_USER_CODE`].
const GDT_USER_DATA: u64 = 0x00CF_F200_0000_FFFF;
/// TSS selector (GDT entry 5, RPL 0): `5 * 8 = 0x28`. S09 slice2g wires the
/// 7-slot table and loads `TR` with this selector; slice2h wires `RSP0` to
/// the 16 KiB ring-0 stack top before `ltr` (no CPL3 entry yet).
pub(crate) const TSS_SEL: u16 = 0x28;
/// Available 64-bit TSS descriptor type (Intel SDM Vol 3 §7.2.3: `0x9`).
/// Busy (`0xB`) is what the CPU writes on first `ltr`/task-switch; the host
/// pin asserts the initial image is available, never busy.
pub(crate) const TSS_TYPE_AVAIL: u64 = 0x9;

/// Low 64 bits of a 64-bit available-TSS system descriptor (Intel SDM
/// Vol 3 Fig 7-4): limit 15:0, base 23:0 split, type `0x9`, S=0, DPL=0,
/// P=1, limit 19:16, AVL=0, L=0, DB=0, G=0 (byte-granular), base 31:24.
/// Consumed by the TSS-wiring slice (`cpu::load_gdt` grows the GDT to 7
/// slots with this encoding).
/// `limit` is the byte limit (`size - 1`, e.g. 103 for a 104-byte TSS).
pub(crate) const fn tss_desc_low(base: u64, limit: u32) -> u64 {
    let limit = (limit as u64) & 0xF_FFFF;
    let b = base & 0xFFFF_FFFF;
    (limit & 0xFFFF)
        | ((b & 0xFFFF) << 16)
        | (((b >> 16) & 0xFF) << 32)
        | (TSS_TYPE_AVAIL << 40)
        | (1 << 47)
        | (((limit >> 16) & 0xF) << 48)
        | (((b >> 24) & 0xFF) << 56)
}

/// High 64 bits of a 64-bit TSS descriptor: base 63:32, upper reserved 0.
/// Companion to [`tss_desc_low`]; consumed by `cpu::load_gdt`.
pub(crate) const fn tss_desc_high(base: u64) -> u64 {
    base >> 32
}

/// Exception vector for `#DE` (divide error). First IDT slot; no error
/// code pushed by the CPU.
const VECTOR_DE: usize = 0;
/// Exception vector for `#PF` (page fault, Intel SDM Vol 3 §6.15). The CPU
/// pushes a 64-bit error code plus `CR2` holds the faulting address. Live
/// `#PF` injection (sole open #168n4) wires this vector in a later slice;
/// this slice only pins the error-code contract host-side (guest IDT still
/// `#DE`-only, so QEMU behavior is unchanged).
pub const VECTOR_PF: usize = 14;
/// Live-`#PF` probe address for the S04 `#168n6` slice: a canonical low-half
/// address far above any firmware-mapped RAM (`0x0000_5000_0000_0000` =
/// 80 TiB) so a volatile read deterministically faults not-present
/// (supervisor read, `P=0`) on OVMF. 4 KiB-aligned, below the
/// non-canonical hole (`0x0000800000000000`), above the framebuffer
/// (`0x80000000`) and low RAM — host-tested below.
pub const PF_PROBE_ADDR: u64 = 0x0000_5000_0000_0000;
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

/// CPU/table boundary moved to [`cpu`] (S05 slice 3): `Gdt`/`GDT`,
/// `DescriptorTablePointer`, `IdtEntry`, `Idt`/`IDT`, and the privileged
/// `load_gdt`/`load_idt` bodies live there; `bring_up_cpu_and_prove_exception`
/// calls them. See `cpu.rs`.
///
/// Port-I/O boundary moved to [`uart`] (S05 slice 2): raw `in`/`out`
/// (`outb`/`inb`) are private unsafe bodies there; `com1_write_byte` and the
/// `write_*` line helpers are safe wrappers. See `uart.rs`.
///
/// Rust body of the `#DE` stub: prove the exception was *caught* (serial
/// line) then park — a controlled failure, not a triple-fault reset.
///
/// Never returns; `vector` is the architecturally-defined vector the stub
/// passed (0 for `#DE`).
#[no_mangle]
extern "C" fn exception_handler_rust(vector: u64) -> ! {
    // Port I/O is a safe wrapper now (`uart::write_exc_line`); only the
    // `hlt` park needs `unsafe`. COM1 was initialized pre-handoff and the
    // single-threaded exception context owns the UART. `ok=1` means the IDT
    // gate delivered control here — the e2e hard-require for S02 slice 2.
    write_exc_line(true, vector);
    unsafe {
        loop {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Fail-closed live-`#PF` U/S verdict (S09 slice2b, REVIEW #622 risk 3).
///
/// The S09 `USER` line is classifier-only (`classify_access` `Err` returns),
/// not a live `#PF` with a CPU-pushed U/S bit. This is the smallest live
/// proof: the supervisor-read-not-present probe at [`PF_PROBE_ADDR`] must
/// arrive with error code `0x0` (`P=0` not-present, `W/R=0` read, `U/S=0`
/// supervisor) at the expected fault address. Returns `(ok, us, w, p)` where
/// `us`/`w`/`p` are the decoded bits and `ok` is true only when
/// `us == false && w == false && p == false && cr2 == PF_PROBE_ADDR` — any
/// `U/S=1`, write, present, or off-probe fault prints `ok=0` fail-closed
/// instead of a silent pass.
///
/// Pure / `core`-only so host tests and the guest `#PF` handler share the
/// exact rule; the guest maps `ok` to `PFUS ok=1` (hard-required by
/// `boot-uefi.ps1`). CPL3 `U=1` entry (user stacks, DPL3 GDT/TSS, `iretq` to
/// ring 3) stays deferred: this slice pins the supervisor half of the U/S
/// path live, the user half stays classifier-only until that slice.
#[must_use]
pub fn pf_us_verdict(code: u64, cr2: u64) -> (bool, bool, bool, bool) {
    let err = PfError::from_bits(code);
    let (us, w, p) = (err.is_user(), err.is_write(), err.is_present());
    let ok = !us && !w && !p && cr2 == PF_PROBE_ADDR;
    (ok, us, w, p)
}

/// Fail-closed live-`#PF` CPL verdict (S09 slice2cf, #30 remainder, REVIEW
/// #799: the supervisor-only [`pf_us_verdict`] pins `U/S=0`; the live CPL3
/// entry slice must prove vector 14 + `U/S=1` + `CPL=3` from one fault).
///
/// Returns `(ok, us, cpl)` where `us` is the decoded U/S bit (error-code
/// bit 2) and `cpl` is `cs & 3` (code-segment RPL — the faulting privilege
/// level the CPU pushed on the `#PF` frame). `ok` is true only when
/// `vector == VECTOR_PF && us && cpl == 3` — a supervisor fault, a non-`#PF`
/// vector, or a CPL0 delivery prints `ok=0` fail-closed instead of a
/// silent pass.
///
/// Hardware-IDT note: `#PF` is never software-invoked (`INT n`), so the
/// CPU delivers CPL3 faults through the existing DPL0 `gate_for` gate —
/// DPL gates only `INT`-invoked vectors (Linux precedent: a DPL0 `#PF`
/// gate handles user-space faults). No IDT change is needed for the live
/// slice; the guest `iretq` entry + stub CS capture + `PFCPL` serial line
/// ride next, with `boot-uefi.ps1` hard-requiring
/// `PFCPL ok=1 us=1 cpl=3`.
///
/// Pure / `core`-only so host tests and the future guest `#PF` handler
/// share the exact rule.
#[must_use]
pub fn pf_cpl_verdict(vector: u64, code: u64, cs: u16) -> (bool, bool, u64) {
    let us = PfError::from_bits(code).is_user();
    let cpl = u64::from(cs & 3);
    let ok = vector as usize == VECTOR_PF && us && cpl == 3;
    (ok, us, cpl)
}

/// Live-`#PF` branch predicate: true when the faulting CS ran at CPL3.
///
/// Single-sources the S09 slice2cg exactly-one-line branch (REVIEW #801
/// risk 4: one fault emits `PFCPL` xor `PFUS`, never both). The guest
/// [`exception_handler_pf_rust`] branches on this; host tests pin it so a
/// flipped comparison (`!= 3`, `== 0`) fails loudly.
///
/// REVIEW #803 residual (1): the CPL0/`PFUS` arm is now host-only — the
/// live guest always takes the CPL3/`PFCPL` arm, so a `pf_us_verdict`
/// mutant hides live. Owed: periodic forced-CPL0-probe boot proving the
/// `PFUS` arm still fires.
#[must_use]
pub fn pf_branch_is_cpl3(cs_raw: u64) -> bool {
    (cs_raw as u16) & 3 == 3
}

/// Rust body of the `#PF` stub: prove the fault was *caught* (serial
/// line) then chain into an intentional `#DE` so the single e2e boot proves
/// BOTH vectors (`PF ok=1 ...` then `EXC ok=1 vector=0`) before parking.
///
/// Never returns; `vector` is 14, `code` is the raw CPU-pushed error code
/// ([`PfError`] decodes bits 0-4), `cr2` is the faulting address, `cs_raw`
/// is the raw 64-bit pushed CS slot (low 16 bits are the faulting
/// code-segment selector; `cs & 3` is the faulting CPL). The chain
/// (`xor eax,eax; div eax`) re-faults through the `#DE` gate whose handler
/// parks — no `iretq` ever executes, so the CPU-pushed `#PF` error code is
/// never popped (REVIEW #229 risk 2: safe exactly because nothing returns;
/// a future returning handler MUST `add rsp,8` before `iretq` or the return
/// frame misaligns).
///
/// S09 slice2cg (live CPL3, #30): one fault emits `PF` plus exactly ONE
/// specific line — CPL3 faults (`cs & 3 == 3`, see [`pf_branch_is_cpl3`])
/// emit `PFCPL` via [`pf_cpl_verdict`], CPL0 faults emit `PFUS` via
/// [`pf_us_verdict`] (REVIEW #801 risk 4: never both lines for one fault).
/// REVIEW #803 residual (2): the `iretq` user stack is still the unmapped
/// `USER_STACK` (`USTACK ok=0` perms-refused) — one-way only, safe because
/// the S-bit supervisor fetch faults before any stack use. True CPL3 entry
/// (executing user code) needs the owed ident-pin mapped stack.
#[no_mangle]
pub extern "C" fn exception_handler_pf_rust(vector: u64, code: u64, cr2: u64, cs_raw: u64) -> ! {
    // Port I/O is a safe wrapper now (`uart::write_pf_line`); only the
    // fault-chain `div` + `hlt` park need `unsafe`. Same COM1-exclusive
    // contract as `exception_handler_rust`.
    write_pf_line(true, vector, code, cr2);
    // S09 slice2cg branch (REVIEW #801 risk 4): exactly one specific line
    // per fault. CPL3 delivery proves the live user path (`PFCPL ok=1
    // us=1 cpl=3`); a CPL0 delivery keeps the supervisor `PFUS` proof.
    let cs = cs_raw as u16;
    if pf_branch_is_cpl3(cs_raw) {
        let (ok, us, cpl) = pf_cpl_verdict(vector, code, cs);
        write_pfcpl_line(ok, us, cpl);
    } else {
        // S09 slice2b live-U/S proof: decode the CPU-pushed error code and emit
        // the fail-closed PFUS line before chaining (a U=1/W=1/P=1/off-probe
        // fault prints `PFUS ok=0` and fails the e2e hard-require loudly).
        let (us_ok, us, w, p) = pf_us_verdict(code, cr2);
        write_pfus_line(us_ok, us, w, p);
    }
    unsafe {
        // Chain: intentional #DE so e2e keeps its EXC hard-require.
        core::arch::asm!("xor eax, eax", "xor edx, edx", "div eax", options(nostack),);
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

/// Shared `#PF` ISR stub source. `#PF` pushes a 64-bit error code, so the
/// fault frame on entry is `[error code][RIP][CS][RFLAGS...]` — the stub
/// reads the code from `[RSP]` and the faulting CS from `[RSP+16]` with NO
/// pop, and no `iretq` ever runs (both handlers park via `hlt`; the `#PF`
/// body chains via a fresh `#DE` fault, not a return). Hence the error-code
/// slot is abandoned, not misaligned (REVIEW #229 risk 2 closed by
/// construction; documented for any future returning handler:
/// `add rsp,8` before `iretq`).
/// The `[RSP+16]` CS offset holds for BOTH the CPL0 frame and the CPL3->0
/// frame (a privilege change pushes extra SS/RSP/RFLAGS *above* CS, so CS
/// stays at +16): S09 slice2cg captures it into the Win64 fourth arg (`R9`)
/// so the Rust body can verdict `cpl = cs & 3` live.
/// Microsoft x64 ABI: `RCX` = vector (14), `RDX` = error code (from
/// `[RSP]`), `R8` = `CR2` (faulting address), `R9` = pushed CS slot,
/// 32-byte shadow space, `RSP` 16-byte aligned before `call`. Host tests
/// assert these instructions.
macro_rules! pf_isr_stub_asm {
    () => {
        r#"
    .global baseos_isr_pf
    baseos_isr_pf:
        mov rdx, [rsp]
        mov rax, cr2
        mov r8, rax
        mov r9, [rsp+16]
        mov ecx, 14
        and rsp, -16
        sub rsp, 32
        call exception_handler_pf_rust
    "#
    };
}

// Guest-only asm: never linked into host `cargo test` builds (the host
// harness links std and must not emit `lidt`/`lgdt`/`div` traps).
#[cfg(not(test))]
core::arch::global_asm!(de_isr_stub_asm!());
#[cfg(not(test))]
core::arch::global_asm!(pf_isr_stub_asm!());

/// Guest-only CPU bring-up + timer multi-tick + scheduler quantum feed +
/// live `#PF` proof (S07 slice15; slice19: IRQ-owned static scheduler;
/// S07 slice26: multi-tick task-context live poll): load
/// GDT, load IDT (now covering `#DE` + `#PF` + timer IRQ0 vector 32), remap
/// the PIC, arm the PIT at 100 Hz
/// (fail-closed: `pit_program_writes() == None` skips `program` + `sti` and
/// emits `TICK ok=0 ticks=0` before the `#PF` probe — REVIEW #458 residue
/// 1), `sti`, wait for `QUANTUM_TICKS + 1` IRQ0 ticks (multi-tick periodic
/// proof: one full quantum plus one, so the bulk drain expires and owes
/// exactly one resched), feed the drained HW ticks into
/// the IRQ-owned static demo scheduler quantum via `timer::poll_timer_dispatch`
/// (S07 slice 17 combined drain-then-dispatch, slice 18 delegating directly
/// to `Scheduler::drain_then_dispatch_gated` so the wrapper cannot drift,
/// slice 19 static-only over `TIMER_TICKS` + `STATIC_SCHED` so no
/// stack-local table remains for the future live IRQ handler;
/// S07 slice25 live helper with the IRQ-owned streak static, S07 slice66
/// record-then-defer flip live (`timer_irq_rust` records one tick and
/// returns with no in-IRQ dispatch; the task-context batch below carries the
/// full wait window, TICK reports the gated cumulative total), emit
/// `TICK ok=1 ticks=<n>`, then `iretq` to CPL3 at a supervisor code RIP
/// (S09 slice2cg live-CPL3 entry: CPL3 fetch of the S-bit page faults
/// U/S=1, P=1 with no PTE surgery).
/// The `#PF` gate jumps to [`exception_handler_pf_rust`], which emits
/// `PF ok=1 vector=14 code=... addr=...` plus `PFCPL ok=1 us=1 cpl=3`
/// (stub-captured pushed CS) and chains into an intentional
/// `#DE` so `EXC ok=1 vector=0` still fires before parking — never returns.
///
/// The tick wait is bounded ([`timer::STI_MAX_SPINS`] `hlt` spins ≈ 2 s budget
/// at 100 Hz): a missing tick emits `TICK ok=0 ticks=0` and the e2e hard-require fails
/// loudly instead of hanging. The drain runs gated (`cli` + `IrqGuard` +
/// `poll_timer_dispatch`), closing the REVIEW #442 R2 static-mut drain in
/// the guest. The quantum feed arms a two-thread demo scheduler
/// (metadata-only descriptors, no stack switch) with one gated dispatch,
/// drains HW ticks into it (bulk accounting, at most one resched owed),
/// and dispatches ONCE on expiry — the same
/// `drain_then_dispatch_gated` shape the future PIT handler uses (S07
/// slice 17: drain-then-pick in one helper so the backlog credits the
/// pre-dispatch current, never the new one).
/// The timer IRQ body only ticks (no serial), so extra ticks during the
/// later `#PF`/`#DE` park cannot interleave the proof lines.
/// The `TICK` line shape is unchanged (`ok=<0|1> ticks=<n>`, S07 slice30: `n`
/// is the gated cumulative total), so the e2e `ticks=[1-9]` hard-require is
/// untouched. S07 slice68 adds the `IRQSTACK ok=1 task=<0x...> irq=<0x...>`
/// two-stack proof after it (task `RSP` vs IRQ-inner capture on the
/// dedicated stack; hard-required by e2e).
///
/// # Safety
/// Must run post-`ExitBootServices` on the boot CPU with interrupts
/// disabled (`cli` already executed by the guest after a successful EBS)
/// and COM1 initialized. Host `cargo test` builds compile this out.
#[cfg(not(test))]
pub unsafe fn bring_up_cpu_and_prove_exception() -> ! {
    cpu::load_gdt();
    cpu::load_idt();
    // S09 slice2k (#30 remainder): live TR proof — `str` readback must equal
    // `TSS_SEL` right after `ltr`; a skipped/faulted `ltr` prints ok=0 loudly.
    {
        let tr = cpu::read_tr();
        write_tr_line(tr == TSS_SEL, tr);
    }
    timer::remap_pic();
    if timer::pit_program_writes().is_none() {
        // Fail-closed before `sti` (REVIEW #458 residue 1): no PIT program
        // means no tick can arrive — report loudly and skip the IRQ window
        // entirely (the `#PF` probe below still fires, so e2e fails on the
        // missing `TICK ok=1`, never on a hang). `TIMER_PROGRAM` is `Some`
        // by construction, so this arm is dead-code in practice.
        write_tick_line(false, 0);
        uart::write_sti_line(
            false,
            0,
            0,
            0,
            timer::STI_WORK_BATCH,
            timer::STI_WORK_TARGET,
        );
    } else {
        timer::program_pit();
        timer::reset_total_drained();
        cpu::sti();
        // Multi-tick wait (S07 slice26; slice28 cumulative; slice33 `>=`
        // helper; S08 slice2 preemption probe): busy-work batch + `hlt`
        // until the JOINT `sti_window_ready` gate clears — `QUANTUM_TICKS + 1`
        // total ticks (one full quantum plus one) AND `STI_WORK_TARGET`
        // workload iterations. The work batch is integer-only (audit pin:
        // no float/vector on the IRQ path) and `black_box`ed so the
        // optimizer keeps it; the wakeup routes through
        // `timer::sti_window_ready` (both halves via named `>=` helpers,
        // never `==`), so a tick-only `hlt` spin or a work-only spin
        // refuses. Const-driven: a quantum change moves the tick target; the
        // ps1 `ticks=[1-9]` gate accepts any positive count.
        let wait_target = crate::sched::QUANTUM_TICKS + 1;
        let work_target = timer::STI_WORK_TARGET;
        let mut work: u64 = 0;
        let mut spins = 0u32;
        loop {
            let mut i = 0u64;
            while i < timer::STI_WORK_BATCH {
                work = work.saturating_add(1);
                i += 1;
                core::hint::spin_loop();
            }
            work = core::hint::black_box(work);
            if timer::sti_window_ready(timer::cumulative_snapshot(), work, wait_target, work_target)
            {
                break;
            }
            if spins >= timer::STI_MAX_SPINS {
                break;
            }
            spins += 1;
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
        // Re-mask before the drain so the `IrqGuard` proof holds and the
        // `#PF` probe below runs deterministically.
        cpu::cli();
        let gate = cpu::IrqGuard::enter();
        // Scheduler quantum feed (S07 slice15; slice19 static): arm the
        // IRQ-owned static demo table (descriptors are metadata-only here —
        // no stack switch, just the quantum counter the HW ticks drain
        // into), then drain via the static scheduler path and reschedule
        // ONCE on expiry.
        crate::sched::sched_init_demo_static(&gate);
        // S07 slice 17: combined drain-then-dispatch (credits to the
        // pre-dispatch current before the pick, so the backlog can never
        // misattribute to the new current). Dispatch result ignored: the
        // demo table always has a Ready target when expiry fires.
        // S07 slice66 record-then-defer flip: `timer_irq_rust` records one
        // tick per IRQ and returns (no in-IRQ drain), so this task-context
        // batch carries the full wait window — the proof line reports the
        // CUMULATIVE total via the gated snapshot (pending + drained_total
        // under `cli` + gate), never the drained-only remainder. This also
        // wires `cumulative_snapshot_gated` into the guest proof (REVIEW
        // #495 tail 3: the gated path is the one used under `cli`).
        let (_drained, _dispatched) = timer::poll_timer_dispatch(&gate);
        let total = timer::cumulative_snapshot_gated(&gate);
        write_tick_line(total > 0, total);
        // S08 slice2 preemption proof: the joint gate over the gated
        // cumulative total + the busy-work counter. `TICK` stays the
        // tick-half proof; this line wires `sti_window_ready` into the
        // guest (closes the slice-1 dead-code gap) and e2e hard-requires
        // `STI ok=1` with positive ticks AND work. S08 slice3 trace: `spins`
        // is the used `hlt` count, so a parameter change (quantum, batch,
        // target) shows up as a comparable before/after wait cost. S08
        // slice4 knob report (REVIEW #605 note 1): `batch`/`target` ride on
        // the same line because `spins` couples to work when work-bound.
        uart::write_sti_line(
            timer::sti_window_ready(total, work, wait_target, work_target),
            total,
            work,
            u64::from(spins),
            timer::STI_WORK_BATCH,
            work_target,
        );
    }
    // S07 slice68 (nit 3 RSP-switch proof): two-stack line — task-context
    // `RSP` vs the last IRQ0 inner capture (recorded while running on the
    // dedicated IRQ stack). `cli` in effect on both arms (the `sti` arm
    // re-masked before the drain; the fail-closed arm never executed `sti`),
    // so the snapshot is exact. `ok=1` proves the tick ran on the IRQ stack
    // while dispatch stayed on the task stack (record-then-defer, #21).
    {
        let task_rsp = cpu::read_rsp();
        // SAFETY: post-EBS guest, `cli` in effect (see above);
        // single-threaded boot owns both statics.
        let (base, inner) = unsafe { timer::irq_stack_snapshot() };
        let top = timer::irq_stack_top_for(base);
        let ok = inner != 0
            && timer::irq_stack_contains(base, inner)
            && inner != task_rsp
            && !timer::irq_stack_contains(base, task_rsp)
            && base & 0xF == 0
            && top & 0xF == 0;
        uart::write_irqstack_line(ok, task_rsp, inner, base, timer::IRQ_STACK_LEN as u64);
    }
    // Live #PF via CPL3 fetch-fault entry (S09 slice2cg, #30, REVIEW
    // #799/#801 cheapest disconfirm): `iretq` to CPL3 (CS 0x1B / SS 0x23,
    // RFLAGS 0x202, user-stack top) at a supervisor code RIP (the `772`
    // label below, present supervisor `.text`). CPL3 fetch of the S-bit
    // page faults with U/S=1, P=1 before executing a single user
    // instruction — no PTE surgery needed. The `#PF` gate jumps to
    // `exception_handler_pf_rust`, whose stub captures the pushed CS into
    // `R9` and emits `PFCPL ok=1 us=1 cpl=3` before chaining into `#DE`.
    // If the GDT/TSS/IDT wiring is wrong this triple-faults and e2e fails
    // loudly (missing PFCPL/EXC lines). One-way: the handler never
    // returns, so no CPL0 resume is needed.
    // SAFETY: post-EBS guest, `cli` re-masked above; `TSS.rsp0` wired in
    // `load_gdt` absorbs the CPL3->0 push; selectors/RFLAGS pin the
    // host-tested `iretq_frame_for` shape (CS 0x1B, SS 0x23, 0x202).
    // S09 slice2cg fix (disassembly-proven): `rax` MUST be plain
    // `out`, never `lateout` — the `lea` writes `rax` BEFORE the
    // `push {ss}` reads its input, so `lateout` lets the compiler stash
    // `ss` in `rax` and the `lea` clobbers it (pushed SS became the RIP
    // with garbage RPL → silent triple-fault; found via objdump).
    let user_rsp = cpu::user_stack_top_for(cpu::user_stack_base());
    core::arch::asm!(
        "lea rax, [rip + 772f]",
        "push {ss}",
        "push {rsp}",
        "push {rflags}",
        "push {cs}",
        "push rax",
        "iretq",
        "772:",
        ss = in(reg) u64::from(USER_DATA_SEL),
        rsp = in(reg) user_rsp,
        rflags = in(reg) cpu::USER_RFLAGS,
        cs = in(reg) u64::from(USER_CODE_SEL),
        out("rax") _,
    );
    // Unreachable when #PF fires; park if the trap somehow returns.
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
        // S09 slice2c: user DPL3 descriptors — same shape as the kernel
        // pair with DPL 0 -> 3 (bits 45-46), present, code L=1 +
        // executable, data writable; selectors carry RPL 3.
        assert_eq!(GDT_USER_CODE >> 43 & 1, 1, "user code must be executable");
        assert_eq!(GDT_USER_CODE >> 45 & 0b11, 3, "user code DPL must be 3");
        assert_eq!(GDT_USER_CODE >> 47 & 1, 1, "user code must be present");
        assert_eq!(GDT_USER_CODE >> 53 & 1, 1, "user code must set L");
        // REVIEW #632 nits (slice2d): pin S=1 + DB=0 + G=1 so a
        // system-descriptor (S=0) or DB/L-conflict typo fails host-side.
        assert_eq!(
            GDT_USER_CODE >> 44 & 1,
            1,
            "user code S must be 1 (code/data)"
        );
        assert_eq!(
            GDT_USER_CODE >> 54 & 1,
            0,
            "user code DB must be 0 with L=1"
        );
        assert_eq!(GDT_USER_CODE >> 55 & 1, 1, "user code G must be 1");
        assert_eq!(GDT_USER_DATA >> 41 & 1, 1, "user data must be writable");
        assert_eq!(
            GDT_USER_DATA >> 43 & 1,
            0,
            "user data must not be executable"
        );
        assert_eq!(GDT_USER_DATA >> 45 & 0b11, 3, "user data DPL must be 3");
        assert_eq!(GDT_USER_DATA >> 47 & 1, 1, "user data must be present");
        // REVIEW #632 nits (slice2d): pin S/G/DB/L shape for the data pair.
        assert_eq!(
            GDT_USER_DATA >> 44 & 1,
            1,
            "user data S must be 1 (code/data)"
        );
        assert_eq!(GDT_USER_DATA >> 55 & 1, 1, "user data G must be 1");
        assert_eq!(GDT_USER_DATA >> 54 & 1, 1, "user data DB must be 1");
        assert_eq!(GDT_USER_DATA >> 53 & 1, 0, "user data L must be 0");
        assert_eq!(USER_CODE_SEL, 0x1B);
        assert_eq!(USER_DATA_SEL, 0x23);
        assert_eq!(USER_CODE_SEL >> 3, 3, "user code GDT index 3");
        assert_eq!(USER_DATA_SEL >> 3, 4, "user data GDT index 4");
        assert_eq!(USER_CODE_SEL & 0b11, 3, "user code RPL 3");
        assert_eq!(USER_DATA_SEL & 0b11, 3, "user data RPL 3");
    }

    #[test]
    fn tss_descriptor_pins_available_system_shape() {
        // S09 slice2e (#30 remainder, REVIEW #634 risk 1): pin the 16-byte
        // available-TSS encoding host-side before any GDT growth / `ltr`.
        // System descriptor (S=0, type 0x9, P=1, DPL=0, L=DB=0, G=0) with
        // base/limit round-trip; busy (0xB) must never be the initial image.
        assert_eq!(TSS_TYPE_AVAIL, 0x9);
        assert_eq!(TSS_SEL, 0x28);
        assert_eq!(TSS_SEL >> 3, 5, "TSS GDT index 5");
        assert_eq!(TSS_SEL & 0b11, 0, "TSS RPL 0");
        let base = 0x0000_ABCD_1234_5000_u64;
        let limit = 103_u32;
        let lo = tss_desc_low(base, limit);
        let hi = tss_desc_high(base);
        assert_eq!(lo >> 40 & 0xF, 0x9, "type must be available-TSS");
        assert_eq!(lo >> 44 & 1, 0, "S must be 0 (system)");
        assert_eq!(lo >> 45 & 0b11, 0, "TSS DPL must be 0");
        assert_eq!(lo >> 47 & 1, 1, "TSS must be present");
        assert_eq!(lo >> 52 & 1, 0, "TSS AVL must be 0");
        assert_eq!(lo >> 53 & 1, 0, "TSS L must be 0");
        assert_eq!(lo >> 54 & 1, 0, "TSS DB must be 0");
        assert_eq!(lo >> 55 & 1, 0, "TSS G must be 0 (byte limit)");
        assert_ne!(lo >> 40 & 0xF, 0xB, "initial image never busy");
        let limit_lo = lo & 0xFFFF;
        let limit_hi = (lo >> 48) & 0xF;
        assert_eq!(limit_lo | (limit_hi << 16), u64::from(limit));
        let base_lo =
            ((lo >> 16) & 0xFFFF) | (((lo >> 32) & 0xFF) << 16) | (((lo >> 56) & 0xFF) << 24);
        assert_eq!(base_lo, base & 0xFFFF_FFFF);
        assert_eq!(hi, base >> 32);
        // Zero base/limit edge stays well-formed (null-table probe shape).
        let lo0 = tss_desc_low(0, 103);
        assert_eq!(lo0 >> 40 & 0xF, 0x9);
        assert_eq!(tss_desc_high(0), 0);
    }

    #[test]
    fn idt_de_gate_round_trips_handler_address() {
        // Encoding the guest `lidt` consumes: split offset fields must
        // reassemble to the stub address; type_attr is a present interrupt
        // gate; selector is the kernel code segment. Built via `cpu::gate_for`
        // (REVIEW #240 R1: `IdtEntry::from_handler` is private to `cpu`, so
        // the selector pin cannot be bypassed in-crate).
        let handler = 0x0000_0000_DEAD_BEEF_u64;
        let e = cpu::gate_for(handler);
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
    fn pf_isr_stub_passes_vector_code_cr2_per_win64_abi() {
        // `#PF` pushes an error code: the stub reads it from `[RSP]`
        // (no pop — the handler parks), loads `CR2` into the third arg
        // (`R8`), the pushed CS slot from `[RSP+16]` into the fourth arg
        // (`R9`; S09 slice2cg live-CPL3 capture — the +16 offset holds
        // for both the CPL0 frame and the CPL3->0 frame, whose extra
        // SS/RSP/RFLAGS sit *above* CS), the vector into `RCX`, then the
        // standard Win64 align + shadow-space + call sequence.
        let stub = pf_isr_stub_asm!();
        assert!(
            stub.contains("mov rdx, [rsp]"),
            "error code must come from [RSP] without popping: {stub}"
        );
        assert!(
            stub.contains("mov rax, cr2"),
            "fault address must be read from CR2: {stub}"
        );
        assert!(
            stub.contains("mov r8, rax"),
            "CR2 must reach the third Win64 arg (R8): {stub}"
        );
        assert!(
            stub.contains("mov r9, [rsp+16]"),
            "pushed CS must reach the fourth Win64 arg (R9) from [RSP+16]: {stub}"
        );
        assert!(
            stub.contains("mov ecx, 14"),
            "vector 14 must be in RCX (Win64 first arg): {stub}"
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
            stub.contains("call exception_handler_pf_rust"),
            "stub must transfer to the PF Rust body: {stub}"
        );
        assert_eq!(VECTOR_PF, 14);
    }

    #[test]
    fn idt_pf_gate_round_trips_handler_address() {
        // Same encoding contract as the `#DE` gate, for vector 14: split
        // offset fields reassemble, present interrupt gate, kernel code
        // selector. Guest `load_idt` wires both gates; e2e proves `#PF`
        // live (probe touch) chained into `#DE`. Via `cpu::gate_for`
        // (REVIEW #240 R1: `from_handler` is private, pin enforced).
        let handler = 0x0000_0000_CAFE_F00D_u64;
        let e = cpu::gate_for(handler);
        assert_eq!(e.handler_offset(), handler);
        let type_attr = e.type_attr;
        let selector = e.selector;
        let ist = e.ist;
        let reserved = e.reserved;
        assert_eq!(type_attr, 0x8E, "present interrupt gate DPL0");
        assert_eq!(selector, KERNEL_CODE_SEL);
        assert_eq!(ist, 0);
        assert_eq!(reserved, 0);
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

    #[test]
    fn pf_us_verdict_pins_supervisor_read_not_present_live_shape() {
        // S09 slice2b (REVIEW #622 risk 3): the live probe is a supervisor
        // read of unmapped PF_PROBE_ADDR, so the CPU pushes code 0x0 at the
        // probe address. Any U/S=1, write, present, or off-probe fault must
        // fail closed (ok=0) while still reporting the decoded bits.
        assert_eq!(
            pf_us_verdict(0x0, PF_PROBE_ADDR),
            (true, false, false, false)
        );
        // U/S=1 (CPL3 fault shape): decoded us=1, verdict refuses.
        assert_eq!(
            pf_us_verdict(0b100, PF_PROBE_ADDR),
            (false, true, false, false)
        );
        // Write fault: decoded w=1, verdict refuses.
        assert_eq!(
            pf_us_verdict(0b010, PF_PROBE_ADDR),
            (false, false, true, false)
        );
        // Present (protection-violation) fault: decoded p=1, refuses.
        assert_eq!(
            pf_us_verdict(0b001, PF_PROBE_ADDR),
            (false, false, false, true)
        );
        // Right code, wrong address (stale CR2): refuses.
        assert_eq!(
            pf_us_verdict(0x0, PF_PROBE_ADDR + 0x1000),
            (false, false, false, false)
        );
        // Upper CPU bits (PK/SGX/HLAT) never flip the bit 0-2 verdict.
        assert_eq!(
            pf_us_verdict(!0x1Fu64, PF_PROBE_ADDR),
            (true, false, false, false)
        );
        // REVIEW #807 nit (slice2ci follow-up): the four refuse arms above
        // already kill a constant-(true,false,false,false) mutant at the
        // first refuse assert_eq -- no separate assert_ne pin is carried
        // (zero added discriminating power, documentation only).
    }

    #[test]
    fn pf_cpl_verdict_pins_vector_us_cpl_shape() {
        // S09 slice2cf (REVIEW #799): the live CPL3 entry must arrive as
        // vector 14 + U/S=1 + CPL 3 (`iretq` with CS 0x1B/0x23, whose RPL
        // the CPU pushes on the `#PF` frame). Any other shape — supervisor
        // U/S=0, non-`#PF` vector, CPL0 CS — refuses fail-closed while
        // still reporting the decoded bits.
        assert_eq!(pf_cpl_verdict(14, 0b100, 0x1B), (true, true, 3));
        assert_eq!(pf_cpl_verdict(14, 0b111, 0x23), (true, true, 3));
        // Supervisor fault at CPL0 (today's live probe shape): refuses.
        assert_eq!(pf_cpl_verdict(14, 0x0, 0x08), (false, false, 0));
        // U/S=1 but wrong vector (not `#PF`): refuses.
        assert_eq!(pf_cpl_verdict(0, 0b100, 0x1B), (false, true, 3));
        // Vector 14 + U/S=1 but CPL0 CS (kernel selector): refuses.
        assert_eq!(pf_cpl_verdict(14, 0b100, 0x08), (false, true, 0));
        // Upper error-code bits (PK/SGX/HLAT) never flip the U/S verdict;
        // high CS bits (TI/LDT) never leak into CPL.
        assert_eq!(pf_cpl_verdict(14, u64::MAX, 0x1B), (true, true, 3));
        assert_eq!(pf_cpl_verdict(14, !0x1F, 0x1B), (false, false, 3));
        assert_eq!(pf_cpl_verdict(14, 0b100, 0xFFFB), (true, true, 3));
        // REVIEW #801 nit (TI coverage): 0xFFFB tests high bits but not
        // TI=1/RPL=0 (0x0C = index 1, TI=1, RPL=0) — `& 3` still isolates
        // CPL 0, so a TI-set CPL0 CS refuses even with U/S=1.
        assert_eq!(pf_cpl_verdict(14, 0b100, 0x0C), (false, true, 0));
        // Mutant-kill pin per slice2ce precedent: a regressed verdict
        // returning constant (true, true, 3) — value-identical to the
        // live-CPL3 accept — MUST fail the three refuse arms below, so the
        // suite carries discriminating power on the refuse path.
        let live_accept = (true, true, 3);
        assert_ne!(pf_cpl_verdict(0, 0b100, 0x1B), live_accept);
        assert_ne!(pf_cpl_verdict(14, 0b000, 0x1B), live_accept);
        assert_ne!(pf_cpl_verdict(14, 0b100, 0x08), live_accept);
    }

    #[test]
    fn pf_branch_routes_exactly_one_line() {
        // S09 slice2ch (REVIEW #801 risk 4 + #803 risk 1): the guest
        // `#PF` body emits PFCPL xor PFUS per fault — this pins the
        // routing predicate the guest branches on, so a flipped comparison
        // (`!= 3`, `== 0`) fails here even though live QEMU only ever
        // exercises the CPL3 arm.
        for cs in [0x1Bu64, 0x23, 0xFFFB, 0x1B | (0xABCD << 16)] {
            assert!(pf_branch_is_cpl3(cs), "CPL3 CS {cs:#x} must route PFCPL");
        }
        for cs in [0x08u64, 0x10, 0x00, 0x0C, 0x28] {
            assert!(!pf_branch_is_cpl3(cs), "CPL0 CS {cs:#x} must route PFUS");
        }
        // Cross-domain check: a CPL3-routed fault can accept the PFCPL
        // verdict while the supervisor PFUS verdict refuses its U=1 shape,
        // and vice versa — the two arms never co-accept one fault.
        let (cpl_ok, _, _) = pf_cpl_verdict(14, 0b100, 0x1B);
        let (us_ok, _, _, _) = pf_us_verdict(0b100, PF_PROBE_ADDR);
        assert!(cpl_ok, "live CPL3 shape must accept PFCPL");
        assert!(!us_ok, "same U=1 shape must refuse PFUS");
        let (cpl_refuse, _, _) = pf_cpl_verdict(14, 0x0, 0x08);
        let (us_accept, _, _, _) = pf_us_verdict(0x0, PF_PROBE_ADDR);
        assert!(!cpl_refuse, "supervisor shape must refuse PFCPL");
        assert!(us_accept, "supervisor shape must accept PFUS");
    }
    #[test]
    fn pf_probe_addr_is_canonical_unmapped_guard() {
        // #168n6 live-#PF guard: canonical low-half, 4 KiB-aligned, far
        // above firmware RAM/FB (0x80000000) and below the non-canonical
        // hole (0x0000800000000000) so OVMF faults not-present (P=0).
        // Single-source (REVIEW #242 nit): the raw predicate lives ONLY in
        // `cpu::probe_addr_valid` — this test asserts via that wrapper on
        // one table (no duplicated `& 0xFFF` / range asserts), so the
        // contract cannot drift across copies. `black_box` keeps the probe
        // assert runtime (not const-evaluable) so `clippy -D warnings`
        // (`assertions_on_constants`) stays clean.
        assert_eq!(PF_PROBE_ADDR, 0x0000_5000_0000_0000);
        let probe = core::hint::black_box(PF_PROBE_ADDR);
        assert!(
            cpu::probe_addr_valid(probe),
            "probe must pass the guard wrapper"
        );
        for (addr, expected) in [
            (PF_PROBE_ADDR + 0x1000, true),
            (PF_PROBE_ADDR + 1, false),
            (0x0000_8000_0000_0000_u64, false),
            (0x8000_0000_u64, false),
            (0_u64, false),
        ] {
            assert_eq!(cpu::probe_addr_valid(addr), expected, "guard({addr:#x})");
        }
    }

    #[test]
    fn probe_addr_valid_agrees_with_pf_probe_guard() {
        // REVIEW #240 R2 agreement pin, single-sourced per #242: the probe
        // constant passes the wrapper and the wrapper pins the same table
        // as the guard test above (raw predicate lives only in `cpu`).
        assert_eq!(PF_PROBE_ADDR, 0x0000_5000_0000_0000);
        assert!(cpu::probe_addr_valid(PF_PROBE_ADDR));
        assert!(cpu::probe_addr_valid(PF_PROBE_ADDR + 0x1000));
        assert!(!cpu::probe_addr_valid(PF_PROBE_ADDR + 1));
    }

    #[test]
    fn uart_wrappers_are_safe_without_unsafe_block() {
        // S05 slice 2 (slice 8: +`uart_init` unified from boot-uefi): the
        // kernel port-I/O boundary is safe wrappers over private unsafe
        // `outb`/`inb` bodies. Binding to the plain `fn` type proves
        // safeness at compile time — an `unsafe fn` would not coerce —
        // without executing port I/O on the host (which would fault
        // outside the guest).
        let init: fn() = uart::uart_init;
        let write_byte: fn(u8) = uart::com1_write_byte;
        let exc: fn(bool, u64) = uart::write_exc_line;
        let pf: fn(bool, u64, u64, u64) = uart::write_pf_line;
        let pfus: fn(bool, bool, bool, bool) = uart::write_pfus_line;
        let pfcpl: fn(bool, bool, u64) = uart::write_pfcpl_line;
        let tr: fn(bool, u16) = uart::write_tr_line;
        let tick: fn(bool, u64) = uart::write_tick_line;
        let sti: fn(bool, u64, u64, u64, u64, u64) = uart::write_sti_line;
        let irqstack: fn(bool, u64, u64, u64, u64) = uart::write_irqstack_line;
        let dec: fn(u64) = uart::write_dec_u64;
        let hex: fn(u64) = uart::write_hex_u64;
        let _ = (
            init, write_byte, exc, pf, pfus, pfcpl, tr, tick, sti, irqstack, dec, hex,
        );
    }

    #[test]
    fn s05_unsafe_inventory_pins_cpu_boundary_shape() {
        // S05 slice 1: `docs/unsafe-inventory.md` sections A/D list every
        // kernel unsafe site. This test pins the doc against the CPU
        // boundary shape so a dropped site or a changed constant fails here.
        // Slice 2 hardening (REVIEW #236 nit 1): substring site names alone
        // would still pass if a table row were deleted while the name
        // survived elsewhere, so every §A/§D row anchor is pinned too.
        const DOC: &str = include_str!("../../../docs/unsafe-inventory.md");
        for site in [
            "outb",
            "inb",
            "com1_write_byte",
            "write_exc_line",
            "write_pf_line",
            "write_tick_line",
            "write_sti_line",
            "write_irqstack_line",
            "read_rsp",
            "tick_on_irq_stack",
            "irq_stack_snapshot",
            "IRQ_STACK",
            "timer_irq_rust",
            "remap_pic",
            "program_pit",
            "pending_snapshot",
            "drain_into_scheduler",
            "poll_timer_dispatch",
            "load_gdt",
            "load_idt",
            "sti",
            "bring_up_cpu_and_prove_exception",
            "baseos_isr_div0",
            "baseos_isr_pf",
        ] {
            assert!(DOC.contains(site), "inventory must document {site}");
        }
        for row in [
            "| A1 |", "| A2 |", "| A3 |", "| A4 |", "| A5 |", "| A6 |", "| A7 |", "| A8 |",
            "| A9 |", "| A10 |", "| A11 |", "| D1 |", "| D2 |", "| D3 |", "| D4 |", "| D5 |",
            "| D6 |", "| D7 |", "| D8 |", "| D9 |", "| D10 |", "| D11 |", "| D12 |",
        ] {
            assert!(DOC.contains(row), "inventory must keep row {row}");
        }
        assert_eq!(KERNEL_CODE_SEL, 0x08);
        assert_eq!(KERNEL_DATA_SEL, 0x10);
        assert_eq!(VECTOR_DE, 0);
        assert_eq!(VECTOR_PF, 14);
        assert_eq!(EXCEPTION_VECTORS, 32);
        assert_eq!(PF_PROBE_ADDR, 0x0000_5000_0000_0000);
        assert_eq!(COM1_PORT, 0x3F8);
        assert_eq!(COM1_LSR, COM1_PORT + 5);
        assert_eq!(LSR_THR_EMPTY, 0x20);
    }
}
