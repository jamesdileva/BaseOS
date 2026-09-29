# Unsafe inventory — S05 slice 1 baseline @bfc4790, slice 2 + slice 3 deltas noted

Scope: exhaustive over `crates/` for `unsafe`, `asm!`/`global_asm!`,
raw-pointer ops (`read_unaligned`, `from_raw_parts`, `addr_of[_mut]`,
`transmute`, raw deref). Method: the two `Select-String` sweeps recorded
in the S05 slice-1 worklog row (guest + test hits listed, no sampling).

Result: `unsafe` blocks are concentrated in exactly 2 files —
`crates/boot-uefi/src/main.rs` (guest boundary) and
`crates/kernel/src/lib.rs` + `crates/kernel/src/uart.rs` (slice 2 split)
+ `crates/kernel/src/cpu.rs` (slice 3 split: GDT/IDT tables + loads;
CPU/exception boundary)
`crates/kernel/src/page_table.rs`, `crates/kernel/src/frame_alloc.rs`,
`crates/boot-info`, `crates/serial-marker`, `crates/version` contain
**zero** `unsafe` blocks (only doc prose and the safe
`fill_from_raw_descriptors(&[u8])` parser; `page_table.rs` mentions of
`unsafe` are doc-prose lines, no `unsafe` block). Line numbers below are
@bfc4790 unless marked (slice 2).

Re-baseline policy (REVIEW #236 nit 2): line numbers drift whenever
wrappers move code, so they are NOT the gate — the host shape-pin tests
(`s05_unsafe_inventory_pins_*`, now with per-row `| A1 |`…`| D10 |`
anchors) are. Each slice that moves an inventoried site notes the new
path here and re-baselines only that site's lines; full line re-baseline
rides with the slice that closes S05. Slice 3 moved D1/D2 (+ tables) from
`kernel/lib.rs` to `kernel/cpu.rs` with zero guest behavior change
(guest `bootx64.efi` byte-identical 85504B); safe `gate_for` /
`probe_addr_valid` added alongside.

## A. Port I/O — COM1 UART (no safe equivalent on a `no_std` guest)

| # | Site | Why unsafe | Invariant / caller contract | Proof |
|---|---|---|---|---|
| A1 | boot-uefi `outb` (151, `asm! out dx,al`) | `out` touches hardware directly | Caller passes a COM1 UART register; QEMU `pc` target always has COM1 | QEMU serial bytes |
| A2 | boot-uefi `inb` (165, `asm! in al,dx`) | `in` reads hardware directly | Caller passes a readable port (COM1 LSR) | THR-empty poll never hangs on QEMU |
| A3 | boot-uefi `uart_init` (181) | Programs UART divisor/line/FIFO regs | Call once before any byte; single-threaded boot | Marker FOUND on every boot |
| A4 | boot-uefi `com1_write_byte` (196) | LSR poll + TX write | `uart_init` ran; exclusive owner (pre-EBS boot thread) | All serial lines |
| A5 | boot-uefi `com1_emit` (203, safe wrapper, `unsafe` block 206) | Bridges infallible sink style to A4 | Same as A4, by construction at `efi_main` | `write_marker`/`write_boot_summary` output |
| A6 | kernel `uart::outb/uart::inb` (private unsafe bodies) + safe `uart::com1_write_byte` (slice 2 @this-slice, was lib.rs 248/262/279 @bfc4790) | Post-EBS copy of A1/A2/A4 (firmware gone, same UART) | S01 `uart_init` ran pre-handoff; post-EBS COM1 is plain port I/O; `unsafe` confined to `outb`/`inb` bodies, callers safe | `EXC`/`PF` lines post-EBS |
| A7 | kernel `uart::write_exc_line`, `uart::write_pf_line`, `uart::write_dec_u64`, `uart::write_hex_u64` (safe wrappers, slice 2 @this-slice, was lib.rs 290/365/390/411 @bfc4790) | Interrupt-context writers, cannot use caller sink | COM1 initialized; single-threaded exception context owns UART; host pin `uart_wrappers_are_safe_without_unsafe_block` proves `fn` (not `unsafe fn`) type | `EXC`/`PF` e2e gates |

## B. UEFI table / slot loads — raw offsets + `read_unaligned` + `transmute`

| # | Site | Why unsafe | Invariant / caller contract | Proof |
|---|---|---|---|---|
| B1 | `load_boot_services` (248): null check, signature `read_unaligned`, +96 slot `read_unaligned` | Raw firmware pointer deref | Live `EFI_SYSTEM_TABLE` or null; signature `0x5453595320494249` gates every offset read; null BS fails closed | 5-case host gate test (null/bad-sig/null-BS/good/null-slot) |
| B2 | `load_get_memory_map` (272, +56), `load_allocate_pool` (293, +64), `load_locate_protocol` (315, +320), `load_exit_boot_services` (221, +232) | `transmute` of a raw slot to `extern "efiapi"` fn | Via B1 gate; null slot fails closed (never transmute null) | Same 5-case pattern per loader; offset-collision asserts (+232 vs +56/+320) |
| B3 | `fill_framebuffer` GOP derefs (561-572): `Mode`/`Info`/`SizeOfInfo`/`FB base`/`FB size` + `from_raw_parts(info_ptr, 36)` | Firmware-owned pointers, sized slice build | `LocateProtocol` ok + non-null `gop`/`Mode`/`Info` + `size_of_info >= 36`; parse via `fill_from_gop_mode` (fail-closed) | Host locate-probe tests; `FB present=1` e2e |
| B4 | `efi_main` (935, `extern "efiapi"`, `#[allow(not_unsafe_ptr_arg_deref)]` 933, `unsafe` block 939) | Firmware entry ABI; raw `(image_handle, system_table)` | Single-threaded boot; init-then-write order; sinks infallible | Whole `boot-uefi.ps1` chain |

## C. Map buffers — static + pool + raw slices

| # | Site | Why unsafe | Invariant / caller contract | Proof |
|---|---|---|---|---|
| C1 | `fill_boot_info` (365): `addr_of_mut!(MEMMAP_BUF)` (378), `from_raw_parts(buf, map_size)` (420) | Static scratch + slice over firmware-filled bytes | Exclusive owner (boot thread pre-EBS); `desc_size >= 40`, `map_size <= cap`; one `buf` pointer threads the dance (reads exactly the buffer that produced `map_key`); pool buffer retained, never freed pre-EBS | BTS-dance + structural + raw-error host tests; `MEMMAP` budget e2e |
| C2 | `refresh_map_key` (436): `&mut *addr_of_mut!(MEMMAP_BUF)` (440) | Same static, key-only refresh | Pre-EBS; single-shot (no dance): outgrown map fails closed, EBS reports original stale status | Outgrown-static host test; refresh-failure test |
| C3 | `phys_to_table_ptr` (626): `&*(phys as *const [u64; 512])` | Identity-map deref of firmware table frame | `table_base_in_boot_map` gate (aligned, non-zero, no overflow, whole frame in one `BootInfo` region); caller passes only walker bases | Bounds-gate host test; `MissingTable` e2e via `PAGING` |

## D. CPU registers / tables / traps (kernel + guest probes)

| # | Site | Why unsafe | Invariant / caller contract | Proof |
|---|---|---|---|---|
| D1 | kernel `cpu::load_gdt` (slice 3 @this-slice, was lib.rs 494 @bfc4790): `addr_of!(GDT)`, `lgdt`, far-return CS reload | Privileged descriptor load + segment switch | Post-EBS boot CPU; static outlives use; selectors `0x08`/`0x10` | Survival into `#DE` e2e |
| D2 | kernel `cpu::load_idt` (slice 3 @this-slice, was lib.rs 528 @bfc4790): `addr_of_mut!(IDT)` gate writes, `addr_of!(IDT)`, `lidt` — gates built via safe [`gate_for`] (pins `KERNEL_CODE_SEL`; slice 4: `IdtEntry::from_handler` is private to `cpu`, so the pin is compiler-enforced) | Privileged table load; raw static mutation | `cli` in effect; gates target `baseos_isr_div0`/`baseos_isr_pf` with `KERNEL_CODE_SEL` | `PF`+`EXC` lines e2e |
| D3 | `global_asm!` stubs (481/483) + `extern "C"` fwd decls (544) | Hand-written entry code (Win64 ABI: vector in `RCX`, shadow space, align) | Host ABI-pin tests share the macro source | Stub-ABI tests; live fire e2e |
| D4 | `exception_handler_rust` body (327) / `exception_handler_pf_rust` body (349): `hlt`, chained `div eax` (352) | `hlt`/fault-chain are CPU instructions | COM1-exclusive exception context; never returns so the pushed `#PF` error code needs no pop (`add rsp,8` owed only if a future handler returns) | `PF ok=1` then `EXC ok=1` chained e2e |
| D5 | `bring_up_cpu_and_prove_exception` guest (560): `load_gdt/load_idt` + volatile probe read (566) + `hlt` (573) | Crosses to the exception world; touches unmapped guard | Post-EBS, `cli`, COM1 ready; probe = `PF_PROBE_ADDR` canonical guard | `PF`+`EXC` e2e |
| D6 | `bring_up_cpu_and_prove_exception` host stub (584, `cfg(test)`) | Same signature without hardware | Spin-only; never touches hardware | Never invoked by tests |
| D7 | kernel `cpu::read_cr3` (slice 6 @this-slice, was boot-uefi 609): `mov {}, cr3` | Privileged register read (valid at CPL0) | Post-EBS boot CPU, single-threaded boot path | `PAGING` e2e |
| D8 | kernel `cpu::read_efer` (slice 6 @this-slice, was boot-uefi 835): `rdmsr` on `EFER_MSR` | `rdmsr` is privileged | CPL0 guest; `ECX` = `EFER_MSR`, NXE = bit 11 | `RMBZ nxen=1` e2e |
| D9 | guest probes `probe_paging`, `probe_live_flags[_for]`, `probe_rmbz` via `kernel::cpu::read_cr3/read_efer` for reads + `kernel::cpu::cr3_pml4_base/efer_nxe_enabled` for decode (slice 7: read+decode co-located, call-site pairing unified) | Read `CR3`/`EFER` + deref firmware tables | Post-EBS boot thread; identity-map lookup via C3 | `PAGING`/`PERMAUDIT[x]`/`RMBZ` e2e |
| D10 | guest `panic` handler (1045, `hlt` 1046) + EBS-fail park (1028, `hlt`) | Must never return into firmware | Guest-only (`cfg(not(test))`) | Fail-loud e2e (missing lines = FAIL) |

## E. Test-only `unsafe` (host `#[cfg(test)]`, fake tables — sound by fixture)

`load_*` 5-case gates, `fill_boot_info`/`fill_framebuffer`/
`exit_boot_services` scripted stands (incl. `from_raw_parts_mut` buffer
setup at 1682), `MEMMAP_BUF` identity asserts (1667/1818). Static-touching
tests serialize via `MEMMAP_TEST_LOCK`. These call the same unsafe fns as
the guest but with heap `Vec` fakes, so no firmware/hardware is touched.

## Shape pins (host tests)

- `boot-uefi`: `s05_unsafe_inventory_pins_boundary_shape` — this doc lists
  every §A–E site; boundary constants pinned
  (96/56/64/320/232, GOP 24/8/16/24/32, `GOP_INFO_MIN` 36, `MEMMAP_BUF_LEN`
  16384, `GET_MAP_MAX_ATTEMPTS` 4, `GET_MAP_POOL_SLACK` 4096,
  `STRUCTURAL_FAIL == usize::MAX`).
- `kernel`: `s05_unsafe_inventory_pins_cpu_boundary_shape` — this doc lists
  every §A/D site; `KERNEL_CODE_SEL` 0x08 / `KERNEL_DATA_SEL` 0x10,
  `VECTOR_DE` 0 / `VECTOR_PF` 14 / `EXCEPTION_VECTORS` 32,
  `PF_PROBE_ADDR` 0x500000000000, COM1 `0x3F8`/`+5`/`0x20`.

Follow-ups (not this slice): boot-uefi UART unify with
`kernel::uart` (pre-EBS `uart_init` stays guest-side); per-site invariant
docs beyond this table. Done: port-I/O safe wrappers (slice 2), CPU
gate/probe safe wrappers `gate_for` / `probe_addr_valid` (slice 3),
selector-pin enforcement + agreement test (slice 4), single-source probe
guard + pure CR3/EFER decode wrappers `cr3_pml4_base` / `efer_nxe_enabled`
(slice 5, host-only — live reads stay D7/D8), live register reads
`read_cr3` / `read_efer` moved into `cpu` (slice 6 — probes
call `kernel::cpu::read_*`), read+decode call-site unification onto
`cpu::cr3_pml4_base` / `cpu::efer_nxe_enabled` (slice 7, this slice —
REVIEW #246 wiring gap closed; UART unify next).
