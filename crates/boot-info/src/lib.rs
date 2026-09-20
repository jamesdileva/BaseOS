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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// Valid regions slice; empty when the header itself is invalid.
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
    pub fn usable_bytes(&self) -> u64 {
        self.memory_map()
            .iter()
            .filter(|r| r.kind == MemoryKind::Usable)
            .fold(0u64, |acc, r| acc.saturating_add(r.size_bytes()))
    }

    /// Framebuffer when present.
    pub fn framebuffer_opt(&self) -> Option<Framebuffer> {
        if self.is_valid() && self.has_framebuffer != 0 {
            Some(self.framebuffer)
        } else {
            None
        }
    }
}

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
    fn region_end_math_saturates() {
        let r = MemoryRegion::new(u64::MAX - 0x1000, 4, MemoryKind::Usable);
        assert_eq!(r.size_bytes(), 4 * PAGE_SIZE);
        assert_eq!(r.end(), u64::MAX);
    }
}
