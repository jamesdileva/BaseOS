#![cfg_attr(not(test), no_std)]

//! S02 kernel entry: receives the S01 [`BootInfo`] handoff and proves the
//! transfer on serial before the guest parks.
//!
//! no_std-clean (core only) so the UEFI guest links it today and a real
//! post-`ExitBootServices` kernel image can reuse it unchanged. This slice
//! is the handoff proof (`KERNEL ok=<0|1>` line asserted by
//! `tools/qemu/boot-uefi.ps1`); CPU init / GDT / IDT ride later S02 slices.
#[cfg(test)]
extern crate std;

use baseos_boot_info::{write_str, write_u64_dec, write_u64_hex, BootInfo};

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
}
