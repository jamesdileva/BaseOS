#![cfg_attr(not(test), no_std)]

//! S01 boot-info handoff: kernel-neutral boot description passed from UEFI loader to kernel.
//!
//! no_std-clean (core only) so both crates/boot-uefi (UEFI guest) and the
//! future kernel crate can depend on it without pulling in std or allocator.
//! The UEFI guest fills a BootInfo (memory regions plus optional framebuffer)
//! and the kernel validates it via BootInfo::is_valid. Wire-up to real
//! UEFI GetMemoryMap and GOP arrives in later S01 slices; this slice pins the
//! struct contract plus host-side unit tests.
#[cfg(test)]
extern crate std;

use core::fmt;

/// Magic identifying a valid BootInfo (BOOTINFO ASCII, big-endian read).
pub const BOOTINFO_MAGIC: u64 = 0x424F_4F54_494E_464F;
/// Current handoff version; bump on any layout change.
pub const BOOTINFO_VERSION: u16 = 1;
/// Inline capacity: no allocator in early boot, so regions live in the struct.
pub const MAX_MEM_REGIONS: usize = 128;
/// Page size all page_count fields are measured in.
pub const PAGE_SIZE: u64 = 4096;

/// Simplified memory kind for the handoff (UEFI types collapse to these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryKind {
    /// Unusable or unknown; never allocate from these.
    Reserved = 0,
    /// Free RAM the kernel may allocate.
    Usable = 1,
    /// Boot-services reclaimable (free after ExitBootServices).
    Reclaimable = 2,
    /// ACPI tables and NVS the kernel must preserve.
    Acpi = 3,
    /// Firmware and MMIO that must never be touched.
    Firmware = 4,
}

/// One contiguous physical region: start to start plus page_count times PAGE_SIZE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct MemoryRegion {
    /// Physical base address, inclusive.
    pub start: u64,
    /// Length in PAGE_SIZE pages.
    pub page_count: u64,
    /// Region classification.
    pub kind: MemoryKind,
    /// Reserved for future flags; must be zero.
    pub _reserved: u32,
}

/// Optional GOP framebuffer description (zeroed when absent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Framebuffer {
    /// Physical base address of the linear framebuffer.
    pub base: u64,
    /// Total mapping size in bytes.
    pub size: u64,
    /// Visible width in pixels.
    pub width: u32,
    /// Visible height in pixels.
    pub height: u32,
    /// Bytes per scanline.
    pub pitch: u32,
    /// Bits per pixel.
    pub bpp: u32,
}

/// Kernel handoff header: validate with BootInfo::is_valid before use.
///
/// ~3 KB inline map: pass by reference (`&BootInfo`) — never by value —
/// per the perf-first rule (review #19: `Copy` on this size risks
/// stack/memcpy cost on every handoff call).
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct BootInfo {
    /// Must equal BOOTINFO_MAGIC.
    pub magic: u64,
    /// Must equal BOOTINFO_VERSION.
    pub version: u16,
    /// Number of valid entries in regions.
    pub region_count: u16,
    /// Padding to keep regions 8-byte aligned.
    pub _pad: u32,
    /// Inline map; only the first region_count entries are valid.
    pub regions: [MemoryRegion; MAX_MEM_REGIONS],
    /// Non-zero when framebuffer is valid.
    pub has_framebuffer: u32,
    /// Framebuffer description (meaningful only when has_framebuffer is non-zero).
    pub framebuffer: Framebuffer,
}

impl MemoryRegion {
    /// Create a region descriptor.
    pub const fn new(start: u64, page_count: u64, kind: MemoryKind) -> Self {
        Self {
            start,
            page_count,
            kind,
            _reserved: 0,
        }
    }

    /// End address (exclusive); saturates instead of wrapping.
    pub const fn end(&self) -> u64 {
        self.start
            .saturating_add(self.page_count.saturating_mul(PAGE_SIZE))
    }

    /// Size in bytes; saturates instead of wrapping.
    pub const fn size_bytes(&self) -> u64 {
        self.page_count.saturating_mul(PAGE_SIZE)
    }
}

impl Framebuffer {
    /// Empty (absent) framebuffer descriptor.
    pub const fn empty() -> Self {
        Self {
            base: 0,
            size: 0,
            width: 0,
            height: 0,
            pitch: 0,
            bpp: 0,
        }
    }
}

impl BootInfo {
    /// Empty handoff with valid magic and version and zero regions.
    pub const fn new() -> Self {
        Self {
            magic: BOOTINFO_MAGIC,
            version: BOOTINFO_VERSION,
            region_count: 0,
            _pad: 0,
            regions: [MemoryRegion::new(0, 0, MemoryKind::Reserved); MAX_MEM_REGIONS],
            has_framebuffer: 0,
            framebuffer: Framebuffer::empty(),
        }
    }

    /// True when magic, version, and region count are sane.
    pub fn is_valid(&self) -> bool {
        self.magic == BOOTINFO_MAGIC
            && self.version == BOOTINFO_VERSION
            && (self.region_count as usize) <= MAX_MEM_REGIONS
    }

    /// Valid regions slice.
    ///
    /// Returns empty when the header itself is invalid — callers MUST check
    /// [`BootInfo::is_valid`] first: an empty slice alone cannot distinguish
    /// "zero regions" from "corrupt header" (review #19 finding 3).
    pub fn memory_map(&self) -> &[MemoryRegion] {
        if self.is_valid() {
            &self.regions[..self.region_count as usize]
        } else {
            &[]
        }
    }

    /// Append a region; returns false (no mutation) when full.
    pub fn push_region(&mut self, region: MemoryRegion) -> bool {
        let n = self.region_count as usize;
        if n >= MAX_MEM_REGIONS {
            return false;
        }
        self.regions[n] = region;
        self.region_count = (n + 1) as u16;
        true
    }

    /// Total usable bytes across Usable regions (saturating).
    ///
    /// Returns 0 for an invalid header (via [`BootInfo::memory_map`]) exactly
    /// like a valid-but-empty map — callers MUST check [`BootInfo::is_valid`]
    /// first or use [`BootInfo::try_usable_bytes`]; a bare
    /// `usable_bytes() == 0` check cannot distinguish "zero usable RAM" from
    /// "corrupt header" (review #21 residual b, review #23).
    pub fn usable_bytes(&self) -> u64 {
        self.try_usable_bytes().unwrap_or(0)
    }

    /// Total usable bytes, or `None` when the header is invalid.
    ///
    /// Prefer this over [`BootInfo::usable_bytes`] in new code: `Some(0)`
    /// means "valid header, zero usable RAM" while `None` means "corrupt
    /// header" (review #23 — closes the doc-only caveat on `usable_bytes`).
    pub fn try_usable_bytes(&self) -> Option<u64> {
        if !self.is_valid() {
            return None;
        }
        Some(
            self.memory_map()
                .iter()
                .filter(|r| r.kind == MemoryKind::Usable)
                .fold(0u64, |acc, r| acc.saturating_add(r.size_bytes())),
        )
    }

    /// Framebuffer when present and sane.
    ///
    /// A non-zero flag alone is not enough (review #19 finding 2, review #21
    /// residual a): the descriptor must also carry a non-zero base, size,
    /// dimensions, pitch, and bpp, otherwise `None` (absent/invalid) is
    /// returned. A zero base or zero pitch/bpp with nonzero size+dims would
    /// hand the kernel an unmappable descriptor. Geometry must also be
    /// self-consistent (review #23): `pitch` must cover `width * bpp` and
    /// `size` must cover `pitch * height`, else `None`.
    pub fn framebuffer_opt(&self) -> Option<Framebuffer> {
        let fb = self.framebuffer;
        if !(self.is_valid()
            && self.has_framebuffer != 0
            && fb.base != 0
            && fb.size != 0
            && fb.width != 0
            && fb.height != 0
            && fb.pitch != 0
            && fb.bpp != 0)
        {
            return None;
        }
        let bytes_per_pixel = u64::from(fb.bpp).div_ceil(8);
        let min_pitch = u64::from(fb.width).saturating_mul(bytes_per_pixel);
        let min_size = u64::from(fb.pitch).saturating_mul(u64::from(fb.height));
        if u64::from(fb.pitch) < min_pitch || fb.size < min_size {
            return None;
        }
        Some(fb)
    }
}

// Compile-time guards (review #23): BootInfo is Clone-only by perf-first
// rule. Re-adding `Copy` trips E0119 here (verified: !Copy compiles, Copy
// conflicts), so the regression cannot slip in silently.
trait _BootInfoMustNotBeCopy {}
impl _BootInfoMustNotBeCopy for BootInfo {}
impl<T: Copy> _BootInfoMustNotBeCopy for T {}

// Exact layout guard: 16 header + 128*24 regions + 4 flag + 32 fb = 3124
// payload, padded to 8-byte alignment = 3128. Tightens the old `> 2048`
// check so accidental field growth/shrinkage fails fast (bump the literal +
// BOOTINFO_VERSION on intentional change).
const _: () = assert!(core::mem::size_of::<BootInfo>() == 3128);

impl Default for BootInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MemoryKind::Reserved => "reserved",
            MemoryKind::Usable => "usable",
            MemoryKind::Reclaimable => "reclaimable",
            MemoryKind::Acpi => "acpi",
            MemoryKind::Firmware => "firmware",
        };
        f.write_str(s)
    }
}

/// Write an ASCII string through a byte callback (serial/COM1 writer pattern).
///
/// Same sink style as `baseos-serial-marker::write_marker`: the UEFI guest
/// passes a COM1 byte writer, host tests pass a `Vec` collector.
pub fn write_str<E>(emit: &mut impl FnMut(u8) -> Result<(), E>, s: &str) -> Result<(), E> {
    for &b in s.as_bytes() {
        emit(b)?;
    }
    Ok(())
}

/// Write a `u64` as lowercase `0x...` hex (no leading-zero padding).
pub fn write_u64_hex<E>(emit: &mut impl FnMut(u8) -> Result<(), E>, v: u64) -> Result<(), E> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    write_str(emit, "0x")?;
    if v == 0 {
        return write_str(emit, "0");
    }
    let mut buf = [0u8; 16];
    let mut n = 0usize;
    let mut x = v;
    while x > 0 {
        buf[n] = DIGITS[(x & 0xF) as usize];
        n += 1;
        x >>= 4;
    }
    while n > 0 {
        n -= 1;
        emit(buf[n])?;
    }
    Ok(())
}

/// Write a `u64` as decimal (no padding).
pub fn write_u64_dec<E>(emit: &mut impl FnMut(u8) -> Result<(), E>, v: u64) -> Result<(), E> {
    if v == 0 {
        return write_str(emit, "0");
    }
    let mut buf = [0u8; 20];
    let mut n = 0usize;
    let mut x = v;
    while x > 0 {
        buf[n] = b'0' + (x % 10) as u8;
        n += 1;
        x /= 10;
    }
    while n > 0 {
        n -= 1;
        emit(buf[n])?;
    }
    Ok(())
}

/// Emit a two-line `BootInfo` serial summary through a byte callback.
///
/// Line 1: `BOOTINFO valid=<0|1> regions=<n> usable=<0x...>` — `usable` is
/// `usable_bytes()` (0 on corrupt header, same mask as documented there).
/// Line 2: raw framebuffer geometry plus the gated verdict, so a `None`
/// verdict stays diagnosable on serial (review #25 suggestion):
/// `FB present=<0|1> base=<0x...> size=<0x...> <WxH> pitch=<n> bpp=<n>`.
///
/// `no_std`-clean (core only); the UEFI guest reuses this for its COM1
/// debug print once `GetMemoryMap`/GOP fill the struct.
pub fn write_boot_summary<E>(
    info: &BootInfo,
    emit: &mut impl FnMut(u8) -> Result<(), E>,
) -> Result<(), E> {
    write_str(emit, "BOOTINFO valid=")?;
    write_u64_dec(emit, u64::from(info.is_valid()))?;
    write_str(emit, " regions=")?;
    write_u64_dec(emit, u64::from(info.region_count))?;
    write_str(emit, " usable=")?;
    write_u64_hex(emit, info.usable_bytes())?;
    write_str(emit, "\n")?;
    write_str(emit, "FB present=")?;
    write_u64_dec(emit, u64::from(info.framebuffer_opt().is_some()))?;
    write_str(emit, " base=")?;
    write_u64_hex(emit, info.framebuffer.base)?;
    write_str(emit, " size=")?;
    write_u64_hex(emit, info.framebuffer.size)?;
    write_str(emit, " ")?;
    write_u64_dec(emit, u64::from(info.framebuffer.width))?;
    write_str(emit, "x")?;
    write_u64_dec(emit, u64::from(info.framebuffer.height))?;
    write_str(emit, " pitch=")?;
    write_u64_dec(emit, u64::from(info.framebuffer.pitch))?;
    write_str(emit, " bpp=")?;
    write_u64_dec(emit, u64::from(info.framebuffer.bpp))?;
    write_str(emit, "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_handoff_is_valid_and_empty() {
        let info = BootInfo::new();
        assert!(info.is_valid());
        assert_eq!(info.memory_map().len(), 0);
        assert_eq!(info.usable_bytes(), 0);
        assert_eq!(info.framebuffer_opt(), None);
    }

    #[test]
    fn bad_magic_or_version_fails_validation() {
        let mut info = BootInfo::new();
        info.magic = 0;
        assert!(!info.is_valid());
        assert_eq!(info.memory_map().len(), 0);
        let mut info = BootInfo::new();
        info.version = BOOTINFO_VERSION + 1;
        assert!(!info.is_valid());
    }

    #[test]
    fn push_and_sum_usable_ignores_reserved() {
        let mut info = BootInfo::new();
        assert!(info.push_region(MemoryRegion::new(0x1000, 4, MemoryKind::Usable)));
        assert!(info.push_region(MemoryRegion::new(0x5000, 1, MemoryKind::Reserved)));
        assert!(info.push_region(MemoryRegion::new(0x9000, 2, MemoryKind::Usable)));
        assert_eq!(info.memory_map().len(), 3);
        assert_eq!(info.usable_bytes(), 6 * PAGE_SIZE);
    }

    #[test]
    fn full_map_rejects_extra_region() {
        let mut info = BootInfo::new();
        for i in 0..MAX_MEM_REGIONS {
            assert!(info.push_region(MemoryRegion::new(i as u64 * 0x1000, 1, MemoryKind::Usable)));
        }
        assert!(!info.push_region(MemoryRegion::new(0xFFFF_0000, 1, MemoryKind::Usable)));
        assert_eq!(info.memory_map().len(), MAX_MEM_REGIONS);
        assert_eq!(info.usable_bytes(), MAX_MEM_REGIONS as u64 * PAGE_SIZE);
    }

    #[test]
    fn framebuffer_present_only_with_flag() {
        let mut info = BootInfo::new();
        info.framebuffer = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        assert_eq!(info.framebuffer_opt(), None);
        info.has_framebuffer = 1;
        assert_eq!(info.framebuffer_opt(), Some(info.framebuffer));
    }

    #[test]
    fn framebuffer_flag_with_empty_or_degenerate_descriptor_is_none() {
        // Flag set but descriptor empty (review #19 finding 2).
        let mut info = BootInfo::new();
        info.has_framebuffer = 1;
        assert_eq!(info.framebuffer_opt(), None);
        // Size present but zero dimensions still absent.
        info.framebuffer = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 0,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        assert_eq!(info.framebuffer_opt(), None);
        // Fully populated descriptor is present.
        info.framebuffer.width = 1024;
        assert_eq!(info.framebuffer_opt(), Some(info.framebuffer));
    }

    #[test]
    fn framebuffer_zero_base_pitch_or_bpp_is_none() {
        // Review #21 residual (a): size+dims nonzero is not enough — a
        // zero base or zero pitch/bpp would hand the kernel a bad descriptor.
        let mut info = BootInfo::new();
        info.has_framebuffer = 1;
        let good = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        info.framebuffer = good;
        assert_eq!(info.framebuffer_opt(), Some(good));
        info.framebuffer.base = 0;
        assert_eq!(info.framebuffer_opt(), None);
        info.framebuffer = good;
        info.framebuffer.pitch = 0;
        assert_eq!(info.framebuffer_opt(), None);
        info.framebuffer = good;
        info.framebuffer.bpp = 0;
        assert_eq!(info.framebuffer_opt(), None);
    }

    #[test]
    fn corrupt_header_masks_usable_bytes_to_zero() {
        // Review #21 residual (b): invalid header reports 0, same as
        // valid-empty — locks the mask behavior so callers remember to
        // check is_valid() instead of relying on usable_bytes() alone.
        // Review #23 adds try_usable_bytes(): None on corrupt, Some on valid.
        let mut info = BootInfo::new();
        assert!(info.push_region(MemoryRegion::new(0x1000, 4, MemoryKind::Usable)));
        assert_eq!(info.usable_bytes(), 4 * PAGE_SIZE);
        assert_eq!(info.try_usable_bytes(), Some(4 * PAGE_SIZE));
        info.magic = 0;
        assert!(!info.is_valid());
        assert_eq!(info.memory_map().len(), 0);
        assert_eq!(info.usable_bytes(), 0);
        assert_eq!(info.try_usable_bytes(), None);
        // Valid-but-empty stays Some(0): distinct from corrupt None.
        let empty = BootInfo::new();
        assert_eq!(empty.try_usable_bytes(), Some(0));
    }

    #[test]
    fn framebuffer_inconsistent_geometry_is_none() {
        // Review #23: pitch must cover width*bpp, size must cover pitch*height.
        let mut info = BootInfo::new();
        info.has_framebuffer = 1;
        let good = Framebuffer {
            base: 0x8000_0000,
            size: 8 * 1024 * 1024,
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        info.framebuffer = good;
        assert_eq!(info.framebuffer_opt(), Some(good));
        // Undersized: size smaller than pitch*height.
        info.framebuffer.size = 4096 * 100;
        assert_eq!(info.framebuffer_opt(), None);
        // Narrow pitch: pitch smaller than width * bytes_per_pixel.
        info.framebuffer = good;
        info.framebuffer.pitch = 1000;
        assert_eq!(info.framebuffer_opt(), None);
    }

    #[test]
    fn handoff_passes_by_reference_and_clones() {
        // BootInfo is Clone-only (no Copy): ~3 KB must not move by value.
        // Regression guards (review #21 residual c, tightened review #23):
        // exact size_of == 3128 plus a compile-time E0119 conflict guard at
        // module scope (re-adding `Copy` fails the build, verified).
        assert_eq!(core::mem::size_of::<BootInfo>(), 3128);
        fn accepts_ref(info: &BootInfo) -> bool {
            info.is_valid()
        }
        let info = BootInfo::new();
        assert!(accepts_ref(&info));
        let dup = info.clone();
        assert_eq!(dup, info);
    }

    #[test]
    fn region_end_math_saturates() {
        let r = MemoryRegion::new(u64::MAX - 0x1000, 4, MemoryKind::Usable);
        assert_eq!(r.size_bytes(), 4 * PAGE_SIZE);
        assert_eq!(r.end(), u64::MAX);
    }

    fn emit_to_vec(write: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
        let mut out = Vec::new();
        write(&mut out);
        out
    }

    #[test]
    fn hex_and_dec_helpers_emit_expected_text() {
        let hex = emit_to_vec(|out: &mut Vec<u8>| {
            write_u64_hex::<core::convert::Infallible>(
                &mut |b| {
                    out.push(b);
                    Ok(())
                },
                0,
            )
            .unwrap()
        });
        assert_eq!(hex, b"0x0");
        let hex = emit_to_vec(|out: &mut Vec<u8>| {
            write_u64_hex::<core::convert::Infallible>(
                &mut |b| {
                    out.push(b);
                    Ok(())
                },
                0x8000_0000,
            )
            .unwrap()
        });
        assert_eq!(hex, b"0x80000000");
        let hex = emit_to_vec(|out: &mut Vec<u8>| {
            write_u64_hex::<core::convert::Infallible>(
                &mut |b| {
                    out.push(b);
                    Ok(())
                },
                u64::MAX,
            )
            .unwrap()
        });
        assert_eq!(hex, b"0xffffffffffffffff");
        let dec = emit_to_vec(|out: &mut Vec<u8>| {
            write_u64_dec::<core::convert::Infallible>(
                &mut |b| {
                    out.push(b);
                    Ok(())
                },
                0,
            )
            .unwrap()
        });
        assert_eq!(dec, b"0");
        let dec = emit_to_vec(|out: &mut Vec<u8>| {
            write_u64_dec::<core::convert::Infallible>(
                &mut |b| {
                    out.push(b);
                    Ok(())
                },
                1024,
            )
            .unwrap()
        });
        assert_eq!(dec, b"1024");
    }

    #[test]
    fn boot_summary_reports_valid_map_and_fb_geometry() {
        // Valid map + good FB: both verdicts 1 and raw geometry visible.
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
        let mut out = Vec::new();
        write_boot_summary::<core::convert::Infallible>(&info, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        let text = std::string::String::from_utf8(out).unwrap();
        assert!(
            text.contains("BOOTINFO valid=1 regions=1 usable=0x4000"),
            "{text}"
        );
        assert!(
            text.contains("FB present=1 base=0x80000000 size=0x800000 1024x768 pitch=4096 bpp=32"),
            "{text}"
        );
    }

    #[test]
    fn boot_summary_stays_diagnosable_when_fb_gated_to_none() {
        // Review #25: corrupt geometry must still print raw fields so the
        // serial capture shows WHY framebuffer_opt() is None.
        let mut info = BootInfo::new();
        info.has_framebuffer = 1;
        info.framebuffer = Framebuffer {
            base: 0x8000_0000,
            size: 4096 * 100, // undersized vs pitch*height -> None
            width: 1024,
            height: 768,
            pitch: 4096,
            bpp: 32,
        };
        assert_eq!(info.framebuffer_opt(), None);
        let mut out = Vec::new();
        write_boot_summary::<core::convert::Infallible>(&info, &mut |b| {
            out.push(b);
            Ok(())
        })
        .unwrap();
        let text = std::string::String::from_utf8(out).unwrap();
        assert!(text.contains("FB present=0"), "{text}");
        assert!(text.contains("1024x768 pitch=4096 bpp=32"), "{text}");
    }
}
