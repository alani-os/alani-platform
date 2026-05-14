//! Paging mode, address, page flag, mapping, and page-table plan contracts.

use crate::{
    ranges_overlap, validate_alignment, validate_platform_label, validate_redaction, DataClass,
    PlatformError, PlatformResult, PlatformRights, RedactionState, TraceContext,
};

/// Paging metadata schema emitted by this crate version.
pub const PAGING_SCHEMA_VERSION: &str = "alani.platform.paging.v1";
/// 4 KiB page size.
pub const PAGE_SIZE_4KIB: u64 = 4 * 1024;
/// 2 MiB page size.
pub const PAGE_SIZE_2MIB: u64 = 2 * 1024 * 1024;
/// 1 GiB page size.
pub const PAGE_SIZE_1GIB: u64 = 1024 * 1024 * 1024;
/// Maximum page mapping label length.
pub const MAX_MAPPING_NAME_LEN: usize = 96;

/// Page is present.
pub const PAGE_FLAG_PRESENT: u64 = 1 << 0;
/// Page is writable.
pub const PAGE_FLAG_WRITABLE: u64 = 1 << 1;
/// Page is user accessible.
pub const PAGE_FLAG_USER: u64 = 1 << 2;
/// Page is executable.
pub const PAGE_FLAG_EXECUTABLE: u64 = 1 << 3;
/// Page is global.
pub const PAGE_FLAG_GLOBAL: u64 = 1 << 4;
/// Page disables caching.
pub const PAGE_FLAG_NO_CACHE: u64 = 1 << 5;
/// Page uses write-through caching.
pub const PAGE_FLAG_WRITE_THROUGH: u64 = 1 << 6;
/// Page maps device/MMIO memory.
pub const PAGE_FLAG_DEVICE: u64 = 1 << 7;
/// Page is a guard mapping.
pub const PAGE_FLAG_GUARD: u64 = 1 << 8;
/// Page is copy-on-write.
pub const PAGE_FLAG_COW: u64 = 1 << 9;
/// Page is shared.
pub const PAGE_FLAG_SHARED: u64 = 1 << 10;

/// All page flags known by this crate version.
pub const PAGE_KNOWN_FLAGS: u64 = PAGE_FLAG_PRESENT
    | PAGE_FLAG_WRITABLE
    | PAGE_FLAG_USER
    | PAGE_FLAG_EXECUTABLE
    | PAGE_FLAG_GLOBAL
    | PAGE_FLAG_NO_CACHE
    | PAGE_FLAG_WRITE_THROUGH
    | PAGE_FLAG_DEVICE
    | PAGE_FLAG_GUARD
    | PAGE_FLAG_COW
    | PAGE_FLAG_SHARED;

/// Physical address wrapper.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalAddress(pub u64);

impl PhysicalAddress {
    /// Creates a physical address.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Validates address bit width.
    pub const fn validate(self, address_bits: u8) -> PlatformResult<()> {
        if address_bits == 0 || address_bits > 56 {
            return Err(PlatformError::InvalidAddress);
        }
        if address_bits < 64 && self.0 >= (1u64 << address_bits) {
            return Err(PlatformError::InvalidAddress);
        }
        Ok(())
    }
}

/// Virtual address wrapper.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct VirtualAddress(pub u64);

impl VirtualAddress {
    /// Creates a virtual address.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Validates address bit width.
    pub const fn validate(self, address_bits: u8) -> PlatformResult<()> {
        if address_bits == 0 || address_bits > 57 {
            return Err(PlatformError::InvalidAddress);
        }
        if address_bits < 64 && self.0 >= (1u64 << address_bits) {
            return Err(PlatformError::InvalidAddress);
        }
        Ok(())
    }
}

/// Page size.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageSize {
    /// 4 KiB page.
    Size4KiB = 0,
    /// 2 MiB page.
    Size2MiB = 1,
    /// 1 GiB page.
    Size1GiB = 2,
}

impl PageSize {
    /// Returns size in bytes.
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Size4KiB => PAGE_SIZE_4KIB,
            Self::Size2MiB => PAGE_SIZE_2MIB,
            Self::Size1GiB => PAGE_SIZE_1GIB,
        }
    }
}

/// Paging mode.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagingMode {
    /// Paging disabled.
    Disabled = 0,
    /// Host-mode simulated paging.
    HostSimulated = 1,
    /// x86_64 four-level paging.
    X86_64FourLevel = 2,
    /// x86_64 five-level paging.
    X86_64FiveLevel = 3,
    /// RISC-V Sv39 paging.
    RiscvSv39 = 4,
    /// RISC-V Sv48 paging.
    RiscvSv48 = 5,
}

impl PagingMode {
    /// Returns maximum virtual address bits for the mode.
    pub const fn virtual_address_bits(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::HostSimulated | Self::X86_64FourLevel | Self::RiscvSv48 => 48,
            Self::X86_64FiveLevel => 57,
            Self::RiscvSv39 => 39,
        }
    }

    /// Returns `true` when the mode supports paging.
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// Page flag bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageFlags(pub u64);

impl PageFlags {
    /// No flags.
    pub const NONE: Self = Self(0);
    /// Present read-only kernel mapping.
    pub const KERNEL_RO: Self = Self(PAGE_FLAG_PRESENT);
    /// Present read-write kernel mapping.
    pub const KERNEL_RW: Self = Self(PAGE_FLAG_PRESENT | PAGE_FLAG_WRITABLE);
    /// Present executable kernel mapping.
    pub const KERNEL_RX: Self = Self(PAGE_FLAG_PRESENT | PAGE_FLAG_EXECUTABLE);
    /// Present user read-write mapping.
    pub const USER_RW: Self = Self(PAGE_FLAG_PRESENT | PAGE_FLAG_USER | PAGE_FLAG_WRITABLE);
    /// Present device mapping.
    pub const DEVICE: Self = Self(PAGE_FLAG_PRESENT | PAGE_FLAG_DEVICE | PAGE_FLAG_NO_CACHE);
    /// Guard mapping.
    pub const GUARD: Self = Self(PAGE_FLAG_GUARD);

    /// Creates flags after rejecting reserved bits.
    pub const fn from_bits(bits: u64) -> PlatformResult<Self> {
        if bits & !PAGE_KNOWN_FLAGS != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(Self(bits))
        }
    }

    /// Returns raw flag bits.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns `true` when all required flags are present.
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Combines two flag sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Validates reserved bits and intrinsic flag invariants.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 & !PAGE_KNOWN_FLAGS != 0 {
            return Err(PlatformError::ReservedBits);
        }
        if self.0 & PAGE_FLAG_GUARD != 0
            && self.0
                & (PAGE_FLAG_PRESENT | PAGE_FLAG_WRITABLE | PAGE_FLAG_USER | PAGE_FLAG_EXECUTABLE)
                != 0
        {
            return Err(PlatformError::InvalidPaging);
        }
        if self.0 & PAGE_FLAG_DEVICE != 0 && self.0 & PAGE_FLAG_EXECUTABLE != 0 {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        if self.0 & PAGE_FLAG_COW != 0 && self.0 & PAGE_FLAG_WRITABLE != 0 {
            return Err(PlatformError::InvalidPaging);
        }
        Ok(())
    }
}

/// Paging validation policy.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagingPolicy {
    /// Whether writable executable mappings are allowed.
    pub allow_write_execute: bool,
    /// Whether user mappings may map device memory.
    pub allow_user_device: bool,
    /// Whether mappings require explicit guard pages around sensitive memory.
    pub require_guard_pages: bool,
}

impl PagingPolicy {
    /// Default fail-closed paging policy.
    pub const DEFAULT: Self = Self {
        allow_write_execute: false,
        allow_user_device: false,
        require_guard_pages: true,
    };

    /// Host-mode fixture policy.
    pub const HOST_FIXTURE: Self = Self {
        allow_write_execute: false,
        allow_user_device: false,
        require_guard_pages: false,
    };
}

impl Default for PagingPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Paging descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagingDescriptor<'a> {
    /// Paging profile label.
    pub name: &'a str,
    /// Schema version.
    pub schema: &'static str,
    /// Paging mode.
    pub mode: PagingMode,
    /// Default page size.
    pub default_page_size: PageSize,
    /// Maximum physical address bits.
    pub physical_address_bits: u8,
    /// Descriptor metadata classification.
    pub data_class: DataClass,
    /// Descriptor metadata redaction state.
    pub redaction: RedactionState,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> PagingDescriptor<'a> {
    /// Creates a paging descriptor.
    pub const fn new(name: &'a str, mode: PagingMode) -> Self {
        Self {
            name,
            schema: PAGING_SCHEMA_VERSION,
            mode,
            default_page_size: PageSize::Size4KiB,
            physical_address_bits: 52,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets physical address width.
    pub const fn with_physical_address_bits(mut self, physical_address_bits: u8) -> Self {
        self.physical_address_bits = physical_address_bits;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates descriptor metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_MAPPING_NAME_LEN)?;
        if self.schema != PAGING_SCHEMA_VERSION || !self.mode.is_enabled() {
            return Err(PlatformError::InvalidPaging);
        }
        if self.physical_address_bits == 0 || self.physical_address_bits > 56 {
            return Err(PlatformError::InvalidAddress);
        }
        validate_redaction(self.data_class, self.redaction)?;
        self.trace.validate()
    }
}

/// Page mapping metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageMapping<'a> {
    /// Mapping label.
    pub name: &'a str,
    /// Virtual start address.
    pub virtual_start: VirtualAddress,
    /// Physical start address.
    pub physical_start: PhysicalAddress,
    /// Mapping length in bytes.
    pub length: u64,
    /// Page size.
    pub page_size: PageSize,
    /// Page flags.
    pub flags: PageFlags,
    /// Mapping metadata classification.
    pub data_class: DataClass,
    /// Mapping metadata redaction state.
    pub redaction: RedactionState,
}

impl<'a> PageMapping<'a> {
    /// Creates a page mapping.
    pub const fn new(
        name: &'a str,
        virtual_start: VirtualAddress,
        physical_start: PhysicalAddress,
        length: u64,
        flags: PageFlags,
    ) -> Self {
        Self {
            name,
            virtual_start,
            physical_start,
            length,
            page_size: PageSize::Size4KiB,
            flags,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
        }
    }

    /// Sets page size.
    pub const fn with_page_size(mut self, page_size: PageSize) -> Self {
        self.page_size = page_size;
        self
    }

    /// Sets classification and redaction state.
    pub const fn classified(mut self, data_class: DataClass, redaction: RedactionState) -> Self {
        self.data_class = data_class;
        self.redaction = redaction;
        self
    }

    /// Validates mapping metadata under descriptor and policy constraints.
    pub fn validate(
        self,
        descriptor: PagingDescriptor<'_>,
        policy: PagingPolicy,
    ) -> PlatformResult<()> {
        descriptor.validate()?;
        validate_platform_label(self.name, MAX_MAPPING_NAME_LEN)?;
        if self.length == 0 {
            return Err(PlatformError::InvalidAddress);
        }
        let page_bytes = self.page_size.bytes();
        validate_alignment(self.virtual_start.0, page_bytes)?;
        validate_alignment(self.physical_start.0, page_bytes)?;
        validate_alignment(self.length, page_bytes)?;
        self.virtual_start
            .validate(descriptor.mode.virtual_address_bits())?;
        self.physical_start
            .validate(descriptor.physical_address_bits)?;
        self.flags.validate()?;
        if self
            .flags
            .contains(PageFlags(PAGE_FLAG_WRITABLE | PAGE_FLAG_EXECUTABLE))
            && !policy.allow_write_execute
        {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        if self
            .flags
            .contains(PageFlags(PAGE_FLAG_USER | PAGE_FLAG_DEVICE))
            && !policy.allow_user_device
        {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        if matches!(self.data_class, DataClass::Sensitive | DataClass::Secret)
            && self.flags.contains(PageFlags(PAGE_FLAG_USER))
        {
            return Err(PlatformError::InvalidPaging);
        }
        validate_redaction(self.data_class, self.redaction)
    }

    /// Returns `true` when this mapping overlaps another virtual range.
    pub const fn overlaps_virtual(self, other: Self) -> PlatformResult<bool> {
        ranges_overlap(
            self.virtual_start.0,
            self.length,
            other.virtual_start.0,
            other.length,
        )
    }
}

/// Fixed-capacity page-table plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageTablePlan<'a, const N: usize> {
    /// Paging descriptor.
    pub descriptor: PagingDescriptor<'a>,
    /// Paging validation policy.
    pub policy: PagingPolicy,
    mappings: [Option<PageMapping<'a>>; N],
    len: usize,
    sealed: bool,
}

impl<'a, const N: usize> PageTablePlan<'a, N> {
    /// Creates an empty page-table plan.
    pub const fn new(descriptor: PagingDescriptor<'a>, policy: PagingPolicy) -> Self {
        Self {
            descriptor,
            policy,
            mappings: [None; N],
            len: 0,
            sealed: false,
        }
    }

    /// Returns plan capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns mapping count.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no mappings are present.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` when plan has been sealed.
    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Returns mapping slots.
    pub const fn mappings(&self) -> &[Option<PageMapping<'a>>; N] {
        &self.mappings
    }

    /// Adds a mapping after authorization, validation, and overlap checks.
    pub fn map(&mut self, rights: PlatformRights, mapping: PageMapping<'a>) -> PlatformResult<()> {
        rights.require(PlatformRights::MAP_MEMORY)?;
        if self.sealed {
            return Err(PlatformError::ReadOnly);
        }
        self.descriptor.validate()?;
        mapping.validate(self.descriptor, self.policy)?;
        if self.len >= N {
            return Err(PlatformError::CapacityExceeded);
        }
        let mut index = 0;
        while index < N {
            if let Some(existing) = self.mappings[index] {
                if mapping.overlaps_virtual(existing)? {
                    return Err(PlatformError::Overlap);
                }
            }
            index += 1;
        }
        self.mappings[self.len] = Some(mapping);
        self.len += 1;
        Ok(())
    }

    /// Removes a mapping by virtual start address.
    pub fn unmap(
        &mut self,
        rights: PlatformRights,
        virtual_start: VirtualAddress,
    ) -> PlatformResult<PageMapping<'a>> {
        rights.require(PlatformRights::MAP_MEMORY)?;
        if self.sealed {
            return Err(PlatformError::ReadOnly);
        }
        virtual_start.validate(self.descriptor.mode.virtual_address_bits())?;
        let mut remove_index = None;
        let mut index = 0;
        while index < N {
            if let Some(mapping) = self.mappings[index] {
                if mapping.virtual_start == virtual_start {
                    remove_index = Some(index);
                    break;
                }
            }
            index += 1;
        }
        let remove_index = remove_index.ok_or(PlatformError::InvalidPaging)?;
        let removed = self.mappings[remove_index].ok_or(PlatformError::Internal)?;
        self.mappings[remove_index] = None;
        self.len -= 1;
        self.compact();
        Ok(removed)
    }

    /// Seals the page-table plan against mutation.
    pub fn seal(&mut self, rights: PlatformRights) -> PlatformResult<()> {
        rights.require(PlatformRights::ADMIN)?;
        self.validate()?;
        self.sealed = true;
        Ok(())
    }

    /// Validates plan invariants.
    pub fn validate(&self) -> PlatformResult<()> {
        self.descriptor.validate()?;
        if self.len > N {
            return Err(PlatformError::InvalidPaging);
        }
        let mut count = 0;
        let mut index = 0;
        while index < N {
            if let Some(mapping) = self.mappings[index] {
                mapping.validate(self.descriptor, self.policy)?;
                count += 1;
                let mut other = index + 1;
                while other < N {
                    if let Some(other_mapping) = self.mappings[other] {
                        if mapping.overlaps_virtual(other_mapping)? {
                            return Err(PlatformError::Overlap);
                        }
                    }
                    other += 1;
                }
            }
            index += 1;
        }
        if count != self.len {
            return Err(PlatformError::InvalidPaging);
        }
        Ok(())
    }

    fn compact(&mut self) {
        let mut write = 0;
        let mut read = 0;
        while read < N {
            if let Some(mapping) = self.mappings[read] {
                if write != read {
                    self.mappings[write] = Some(mapping);
                    self.mappings[read] = None;
                }
                write += 1;
            }
            read += 1;
        }
    }
}
