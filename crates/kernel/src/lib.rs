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
#[cfg(not(test))]
use uart::write_tick_line;
use uart::{write_exc_line, write_pf_line};

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

/// Rust body of the `#PF` stub: prove the fault was *caught* (serial
/// line) then chain into an intentional `#DE` so the single e2e boot proves
/// BOTH vectors (`PF ok=1 ...` then `EXC ok=1 vector=0`) before parking.
///
/// Never returns; `vector` is 14, `code` is the raw CPU-pushed error code
/// ([`PfError`] decodes bits 0-4), `cr2` is the faulting address. The chain
/// (`xor eax,eax; div eax`) re-faults through the `#DE` gate whose handler
/// parks — no `iretq` ever executes, so the CPU-pushed `#PF` error code is
/// never popped (REVIEW #229 risk 2: safe exactly because nothing returns;
/// a future returning handler MUST `add rsp,8` before `iretq` or the return
/// frame misaligns).
#[no_mangle]
pub extern "C" fn exception_handler_pf_rust(vector: u64, code: u64, cr2: u64) -> ! {
    // Port I/O is a safe wrapper now (`uart::write_pf_line`); only the
    // fault-chain `div` + `hlt` park need `unsafe`. Same COM1-exclusive
    // contract as `exception_handler_rust`.
    write_pf_line(true, vector, code, cr2);
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
/// reads the code from `[RSP]` with NO pop, and no `iretq` ever runs (both
/// handlers park via `hlt`; the `#PF` body chains via a fresh `#DE` fault,
/// not a return). Hence the error-code slot is abandoned, not misaligned
/// (REVIEW #229 risk 2 closed by construction; documented for any future
/// returning handler: `add rsp,8` before `iretq`).
/// Microsoft x64 ABI: `RCX` = vector (14), `RDX` = error code (from
/// `[RSP]`), `R8` = `CR2` (faulting address), 32-byte shadow space, `RSP`
/// 16-byte aligned before `call`. Host tests assert these instructions.
macro_rules! pf_isr_stub_asm {
    () => {
        r#"
    .global baseos_isr_pf
    baseos_isr_pf:
        mov rdx, [rsp]
        mov rax, cr2
        mov r8, rax
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
/// `TICK ok=1 ticks=<n>`, then volatile-read [`PF_PROBE_ADDR`] (unmapped
/// guard → not-present `#PF`).
/// The `#PF` gate jumps to [`exception_handler_pf_rust`], which emits
/// `PF ok=1 vector=14 code=... addr=...` and chains into an intentional
/// `#DE` so `EXC ok=1 vector=0` still fires before parking — never returns.
///
/// The tick wait is bounded (200 `hlt` spins ≈ 2 s budget at 100 Hz): a
/// missing tick emits `TICK ok=0 ticks=0` and the e2e hard-require fails
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
/// untouched.
///
/// # Safety
/// Must run post-`ExitBootServices` on the boot CPU with interrupts
/// disabled (`cli` already executed by the guest after a successful EBS)
/// and COM1 initialized. Host `cargo test` builds compile this out.
#[cfg(not(test))]
pub unsafe fn bring_up_cpu_and_prove_exception() -> ! {
    cpu::load_gdt();
    cpu::load_idt();
    timer::remap_pic();
    if timer::pit_program_writes().is_none() {
        // Fail-closed before `sti` (REVIEW #458 residue 1): no PIT program
        // means no tick can arrive — report loudly and skip the IRQ window
        // entirely (the `#PF` probe below still fires, so e2e fails on the
        // missing `TICK ok=1`, never on a hang). `TIMER_PROGRAM` is `Some`
        // by construction, so this arm is dead-code in practice.
        write_tick_line(false, 0);
    } else {
        timer::program_pit();
        timer::reset_total_drained();
        cpu::sti();
        // Multi-tick wait (S07 slice26; slice28 cumulative; slice33 `>=`
        // helper): `hlt` until `QUANTUM_TICKS + 1` total ticks are observed
        // (one full quantum plus one, so the task-context live drain expires
        // and owes exactly one resched). Cumulative (`pending + drained_total`)
        // instead of pending-only (S07 slice28; S07 slice66: IRQ records
        // without draining, so pending carries the window until the task
        // batch moves it to drained_total — either order sums the same). The
        // target check routes through `timer::cumulative_wait_ready` (`>=`,
        // never `==`): the storm fallback is non-monotonic by construction
        // (REVIEW #501 risk 2), so exact equality could miss its wakeup after
        // a step-back. Cumulative (not pending-only) since S07 slice28; S07
        // slice66 keeps it: IRQ records without draining, so pending carries
        // the window and drained_total carries the task batch — the sum is
        // the proof either way. Const-driven: a quantum change moves the target; the
        // ps1 `ticks=[1-9]` gate accepts any positive count.
        let wait_target = crate::sched::QUANTUM_TICKS + 1;
        let mut spins = 0u32;
        loop {
            if timer::cumulative_wait_ready(timer::cumulative_snapshot(), wait_target) {
                break;
            }
            if spins >= 200 {
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
    }
    // Live #PF (#168n6, sole functional open): touch the unmapped guard.
    // If either IDT gate is wrong this triple-faults and e2e fails loudly
    // (missing PF/EXC lines). Volatile so the read is never optimized out.
    core::arch::asm!(
        "mov rax, [{addr}]",
        addr = in(reg) PF_PROBE_ADDR as *const u64,
        options(nostack),
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
        // (`R8`), the vector into `RCX`, then the standard Win64
        // align + shadow-space + call sequence.
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
        let tick: fn(bool, u64) = uart::write_tick_line;
        let dec: fn(u64) = uart::write_dec_u64;
        let hex: fn(u64) = uart::write_hex_u64;
        let _ = (init, write_byte, exc, pf, tick, dec, hex);
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
            "| A9 |", "| D1 |", "| D2 |", "| D3 |", "| D4 |", "| D5 |", "| D6 |", "| D7 |",
            "| D8 |", "| D9 |", "| D10 |", "| D11 |",
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
