#![cfg_attr(not(feature = "std"), no_std)]

//! Architecture-specific HAL, interrupt, timer, CPU feature, and paging contracts.
//!
//! `alani-platform` owns the platform abstraction boundary used by boot,
//! kernel, device, and simulator code. The API is dependency-free, `no_std`
//! compatible, and explicit about unsafe hardware seams, trace propagation,
//! redaction, authorization, and fail-closed validation.

pub mod arch;
pub mod hal;
pub mod interrupts;
pub mod paging;
pub mod timers;

pub use arch::{
    ArchDescriptor, Architecture, ArchitectureProfile, CpuFeatureSet, CpuProfile, CpuVendor,
    Endianness, ExecutionMode, PrivilegeLevel, WordWidth, ARCH_SCHEMA_VERSION, CPU_FEATURE_AIA,
    CPU_FEATURE_APIC, CPU_FEATURE_ATOMIC, CPU_FEATURE_FENCE_I, CPU_FEATURE_FPU, CPU_FEATURE_MMU,
    CPU_FEATURE_NX, CPU_FEATURE_RISCV_SBI, CPU_FEATURE_SMP, CPU_FEATURE_SSE2, CPU_FEATURE_SV39,
    CPU_FEATURE_SV48, CPU_FEATURE_TSC, CPU_FEATURE_VIRT, KNOWN_CPU_FEATURES, MAX_ARCH_NAME_LEN,
    MAX_CPU_LABEL_LEN,
};
pub use hal::{
    BootPhase, BootStep, CachePolicy, HalCapabilities, HalDescriptor, HardwareProfile, MmioRegion,
    HAL_CAP_DMA, HAL_CAP_INTERRUPTS, HAL_CAP_MMIO, HAL_CAP_PAGING, HAL_CAP_POWER,
    HAL_CAP_SERIAL_CONSOLE, HAL_CAP_SMP, HAL_CAP_TIMERS, HAL_KNOWN_CAPABILITIES,
    HAL_SCHEMA_VERSION, MAX_MMIO_REGION_LEN,
};
pub use interrupts::{
    InterruptBinding, InterruptControllerKind, InterruptDescriptor, InterruptEvent, InterruptFlags,
    InterruptPolarity, InterruptTable, InterruptTrigger, InterruptVector, INTERRUPT_FLAG_DEFERRED,
    INTERRUPT_FLAG_ENABLED, INTERRUPT_FLAG_MASKED, INTERRUPT_FLAG_NMI, INTERRUPT_FLAG_SHARED,
    INTERRUPT_KNOWN_FLAGS, INTERRUPT_SCHEMA_VERSION, MAX_INTERRUPT_NAME_LEN, MAX_INTERRUPT_VECTORS,
};
pub use paging::{
    PageFlags, PageMapping, PageSize, PageTablePlan, PagingDescriptor, PagingMode, PagingPolicy,
    PhysicalAddress, VirtualAddress, PAGE_FLAG_COW, PAGE_FLAG_DEVICE, PAGE_FLAG_EXECUTABLE,
    PAGE_FLAG_GLOBAL, PAGE_FLAG_GUARD, PAGE_FLAG_NO_CACHE, PAGE_FLAG_PRESENT, PAGE_FLAG_SHARED,
    PAGE_FLAG_USER, PAGE_FLAG_WRITABLE, PAGE_FLAG_WRITE_THROUGH, PAGE_KNOWN_FLAGS, PAGE_SIZE_1GIB,
    PAGE_SIZE_2MIB, PAGE_SIZE_4KIB, PAGING_SCHEMA_VERSION,
};
pub use timers::{
    TimerCapabilities, TimerConfig, TimerDescriptor, TimerDevice, TimerKind, TimerMode,
    TimerResolution, TimerSnapshot, TimerState, TIMER_CAP_APIC, TIMER_CAP_DEADLINE, TIMER_CAP_HPET,
    TIMER_CAP_MONOTONIC, TIMER_CAP_PERIODIC, TIMER_CAP_RISCV_SBI, TIMER_CAP_TSC,
    TIMER_CAP_WATCHDOG, TIMER_KNOWN_CAPABILITIES, TIMER_SCHEMA_VERSION,
};

/// Repository name.
pub const REPOSITORY: &str = "alani-platform";
/// Compatibility alias recorded by the repository spec.
pub const ALIAS_ARCH: &str = "alani-arch";
/// Crate version.
pub const VERSION: &str = "0.1.0";
/// Public module names exposed by this crate.
pub const MODULES: &[&str] = &["arch", "hal", "interrupts", "timers", "paging"];

/// Feature bit for architecture descriptors and CPU feature metadata.
pub const PLATFORM_FEATURE_ARCH: u64 = 1 << 0;
/// Feature bit for HAL descriptors and boot sequencing.
pub const PLATFORM_FEATURE_HAL: u64 = 1 << 1;
/// Feature bit for interrupt controller contracts.
pub const PLATFORM_FEATURE_INTERRUPTS: u64 = 1 << 2;
/// Feature bit for timer contracts.
pub const PLATFORM_FEATURE_TIMERS: u64 = 1 << 3;
/// Feature bit for paging and page-table primitives.
pub const PLATFORM_FEATURE_PAGING: u64 = 1 << 4;
/// Feature bit for MMIO region metadata.
pub const PLATFORM_FEATURE_MMIO: u64 = 1 << 5;
/// Feature bit for trace-context propagation.
pub const PLATFORM_FEATURE_TRACE_CONTEXT: u64 = 1 << 6;
/// Feature bit for host-mode simulated platform records.
pub const PLATFORM_FEATURE_HOST_SIMULATION: u64 = 1 << 7;

/// All platform feature bits known by this crate version.
pub const PLATFORM_KNOWN_FEATURES: u64 = PLATFORM_FEATURE_ARCH
    | PLATFORM_FEATURE_HAL
    | PLATFORM_FEATURE_INTERRUPTS
    | PLATFORM_FEATURE_TIMERS
    | PLATFORM_FEATURE_PAGING
    | PLATFORM_FEATURE_MMIO
    | PLATFORM_FEATURE_TRACE_CONTEXT
    | PLATFORM_FEATURE_HOST_SIMULATION;

/// Caller may read platform metadata.
pub const PLATFORM_RIGHT_READ: u64 = 1 << 0;
/// Caller may configure platform components.
pub const PLATFORM_RIGHT_CONFIGURE: u64 = 1 << 1;
/// Caller may bind or unbind interrupts.
pub const PLATFORM_RIGHT_INTERRUPT_BIND: u64 = 1 << 2;
/// Caller may configure timers.
pub const PLATFORM_RIGHT_TIMER_CONFIGURE: u64 = 1 << 3;
/// Caller may create or remove page mappings.
pub const PLATFORM_RIGHT_MAP_MEMORY: u64 = 1 << 4;
/// Caller may control CPU or privileged execution state.
pub const PLATFORM_RIGHT_CPU_CONTROL: u64 = 1 << 5;
/// Caller may touch MMIO metadata.
pub const PLATFORM_RIGHT_MMIO: u64 = 1 << 6;
/// Caller may emit or preserve audit evidence.
pub const PLATFORM_RIGHT_AUDIT: u64 = 1 << 7;
/// Caller has administrative platform authority.
pub const PLATFORM_RIGHT_ADMIN: u64 = 1 << 8;

/// All platform rights known by this crate version.
pub const PLATFORM_KNOWN_RIGHTS: u64 = PLATFORM_RIGHT_READ
    | PLATFORM_RIGHT_CONFIGURE
    | PLATFORM_RIGHT_INTERRUPT_BIND
    | PLATFORM_RIGHT_TIMER_CONFIGURE
    | PLATFORM_RIGHT_MAP_MEMORY
    | PLATFORM_RIGHT_CPU_CONTROL
    | PLATFORM_RIGHT_MMIO
    | PLATFORM_RIGHT_AUDIT
    | PLATFORM_RIGHT_ADMIN;

/// Trace flag indicating the event was sampled.
pub const TRACE_FLAG_SAMPLED: u32 = 1 << 0;
/// Trace flag indicating debug metadata may be attached by a trusted sink.
pub const TRACE_FLAG_DEBUG: u32 = 1 << 1;
/// Trace flag indicating a hardware-facing boundary was crossed.
pub const TRACE_FLAG_HARDWARE_BOUNDARY: u32 = 1 << 2;
/// Trace flag indicating audit evidence must be preserved.
pub const TRACE_FLAG_AUDIT_REQUIRED: u32 = 1 << 3;

/// Trace flags known by this crate version.
pub const TRACE_KNOWN_FLAGS: u32 = TRACE_FLAG_SAMPLED
    | TRACE_FLAG_DEBUG
    | TRACE_FLAG_HARDWARE_BOUNDARY
    | TRACE_FLAG_AUDIT_REQUIRED;

/// Result alias for platform validation and host-mode operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Error taxonomy for platform, HAL, interrupt, timer, and paging contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformError {
    /// A required field was empty or omitted.
    MissingField,
    /// A bounded field exceeded its documented maximum length.
    FieldTooLong,
    /// A label contained a disallowed character.
    InvalidLabel,
    /// Unknown feature, capability, flag, or rights bits were supplied.
    ReservedBits,
    /// Architecture metadata failed validation or is unsupported.
    UnsupportedArchitecture,
    /// CPU feature metadata failed validation or is unsupported.
    UnsupportedFeature,
    /// CPU profile metadata failed validation.
    InvalidCpu,
    /// HAL metadata failed validation.
    InvalidHal,
    /// Interrupt controller, vector, binding, or table metadata failed validation.
    InvalidInterrupt,
    /// Timer metadata failed validation.
    InvalidTimer,
    /// Paging mode, mapping, flag, or table metadata failed validation.
    InvalidPaging,
    /// Address or address range was malformed.
    InvalidAddress,
    /// An address, range, or region was not aligned as required.
    Unaligned,
    /// Ranges overlap where exclusivity is required.
    Overlap,
    /// Caller lacks required platform authority.
    AccessDenied,
    /// Operation attempted to mutate a read-only target.
    ReadOnly,
    /// Fixed-capacity collection is full.
    CapacityExceeded,
    /// State machine transition is not allowed.
    InvalidState,
    /// Trace context was malformed.
    InvalidTrace,
    /// Redaction state is incompatible with data classification.
    InvalidRedaction,
    /// Audit evidence is required but not authorized.
    AuditRequired,
    /// Operation crosses an explicit unsafe-review boundary.
    UnsafeReviewRequired,
    /// Internal invariant failed.
    Internal,
}

impl PlatformError {
    /// Stable reason label for diagnostics and tests.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::FieldTooLong => "field_too_long",
            Self::InvalidLabel => "invalid_label",
            Self::ReservedBits => "reserved_bits",
            Self::UnsupportedArchitecture => "unsupported_architecture",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::InvalidCpu => "invalid_cpu",
            Self::InvalidHal => "invalid_hal",
            Self::InvalidInterrupt => "invalid_interrupt",
            Self::InvalidTimer => "invalid_timer",
            Self::InvalidPaging => "invalid_paging",
            Self::InvalidAddress => "invalid_address",
            Self::Unaligned => "unaligned",
            Self::Overlap => "overlap",
            Self::AccessDenied => "access_denied",
            Self::ReadOnly => "read_only",
            Self::CapacityExceeded => "capacity_exceeded",
            Self::InvalidState => "invalid_state",
            Self::InvalidTrace => "invalid_trace",
            Self::InvalidRedaction => "invalid_redaction",
            Self::AuditRequired => "audit_required",
            Self::UnsafeReviewRequired => "unsafe_review_required",
            Self::Internal => "internal",
        }
    }

    /// Returns `true` when this error represents a fail-closed platform boundary.
    pub const fn is_security_relevant(self) -> bool {
        matches!(
            self,
            Self::ReservedBits
                | Self::UnsupportedArchitecture
                | Self::UnsupportedFeature
                | Self::InvalidAddress
                | Self::Unaligned
                | Self::Overlap
                | Self::AccessDenied
                | Self::ReadOnly
                | Self::InvalidRedaction
                | Self::AuditRequired
                | Self::UnsafeReviewRequired
        )
    }
}

/// Data sensitivity classification for platform metadata.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DataClass {
    /// Public metadata.
    Public = 0,
    /// Operational metadata suitable for trusted operators.
    Operational = 1,
    /// Sensitive metadata requiring redaction before broad export.
    Sensitive = 2,
    /// Secret metadata or hardware state that must not be exported raw.
    Secret = 3,
}

impl DataClass {
    /// Returns `true` when data with this class must be redacted before export.
    pub const fn requires_redaction(self) -> bool {
        matches!(self, Self::Sensitive | Self::Secret)
    }
}

/// Redaction state applied to platform metadata.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedactionState {
    /// Public fields only.
    Public = 0,
    /// Operational metadata only.
    Operational = 1,
    /// Sensitive fields were redacted.
    SensitiveRedacted = 2,
    /// Secret fields were redacted.
    SecretRedacted = 3,
    /// Sensitive fields are present and must not be exported broadly.
    UnredactedSensitive = 4,
}

/// Stable trace context copied from observability/syscall layers when present.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TraceContext {
    /// Trace identifier shared across component boundaries.
    pub trace_id: u64,
    /// Current span identifier.
    pub span_id: u64,
    /// Parent span identifier.
    pub parent_span_id: u64,
    /// Trace flags.
    pub flags: u32,
}

impl TraceContext {
    /// Empty trace context used when no trace is available.
    pub const EMPTY: Self = Self {
        trace_id: 0,
        span_id: 0,
        parent_span_id: 0,
        flags: 0,
    };

    /// Creates a root trace context.
    pub const fn root(trace_id: u64, span_id: u64) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id: 0,
            flags: TRACE_FLAG_SAMPLED,
        }
    }

    /// Creates a child trace context preserving trace flags.
    pub const fn child(self, span_id: u64) -> Self {
        Self {
            trace_id: self.trace_id,
            span_id,
            parent_span_id: self.span_id,
            flags: self.flags,
        }
    }

    /// Sets trace flags.
    pub const fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// Returns `true` when both trace and span identifiers are present.
    pub const fn is_present(self) -> bool {
        self.trace_id != 0 && self.span_id != 0
    }

    /// Validates trace metadata.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.flags & !TRACE_KNOWN_FLAGS != 0 {
            return Err(PlatformError::ReservedBits);
        }
        if self.trace_id == 0 && self.span_id == 0 && self.parent_span_id == 0 {
            return Ok(());
        }
        if self.trace_id == 0 || self.span_id == 0 {
            return Err(PlatformError::InvalidTrace);
        }
        if self.parent_span_id != 0 && self.parent_span_id == self.span_id {
            return Err(PlatformError::InvalidTrace);
        }
        Ok(())
    }
}

/// Platform authority bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformRights(pub u64);

impl PlatformRights {
    /// No authority.
    pub const NONE: Self = Self(0);
    /// Read platform metadata.
    pub const READ: Self = Self(PLATFORM_RIGHT_READ);
    /// Configure platform metadata.
    pub const CONFIGURE: Self = Self(PLATFORM_RIGHT_CONFIGURE);
    /// Bind or unbind interrupts.
    pub const INTERRUPT_BIND: Self = Self(PLATFORM_RIGHT_INTERRUPT_BIND);
    /// Configure timers.
    pub const TIMER_CONFIGURE: Self = Self(PLATFORM_RIGHT_TIMER_CONFIGURE);
    /// Create or remove page mappings.
    pub const MAP_MEMORY: Self = Self(PLATFORM_RIGHT_MAP_MEMORY);
    /// Control CPU or privileged execution state.
    pub const CPU_CONTROL: Self = Self(PLATFORM_RIGHT_CPU_CONTROL);
    /// Touch MMIO metadata.
    pub const MMIO: Self = Self(PLATFORM_RIGHT_MMIO);
    /// Emit or preserve audit evidence.
    pub const AUDIT: Self = Self(PLATFORM_RIGHT_AUDIT);
    /// Administrative platform authority.
    pub const ADMIN: Self = Self(PLATFORM_RIGHT_ADMIN);
    /// Full authority for host-mode administrative tests.
    pub const ADMINISTRATOR: Self = Self(PLATFORM_KNOWN_RIGHTS);

    /// Creates rights from raw bits after rejecting unknown bits.
    pub const fn from_bits(bits: u64) -> PlatformResult<Self> {
        if bits & !PLATFORM_KNOWN_RIGHTS != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(Self(bits))
        }
    }

    /// Returns raw rights bits.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns `true` when all required rights are present.
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Combines two rights sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Validates reserved bits.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 & !PLATFORM_KNOWN_RIGHTS != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(())
        }
    }

    /// Fails closed when required rights are absent.
    pub const fn require(self, required: Self) -> PlatformResult<()> {
        if self.0 & !PLATFORM_KNOWN_RIGHTS != 0 || required.0 & !PLATFORM_KNOWN_RIGHTS != 0 {
            return Err(PlatformError::ReservedBits);
        }
        if self.contains(required) {
            Ok(())
        } else {
            Err(PlatformError::AccessDenied)
        }
    }
}

/// Implementation maturity marker for generated repository metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentStatus {
    /// API is present as a draft skeleton.
    Draft,
    /// API is implemented enough for host-mode experimentation.
    Experimental,
    /// API is compatible and stable.
    Stable,
}

/// Stable component identity record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentInfo {
    /// Repository name.
    pub repository: &'static str,
    /// Crate version.
    pub version: &'static str,
    /// Current implementation status.
    pub status: ComponentStatus,
}

/// Returns stable component identity metadata.
pub const fn component_info() -> ComponentInfo {
    ComponentInfo {
        repository: REPOSITORY,
        version: VERSION,
        status: ComponentStatus::Experimental,
    }
}

/// Returns the repository name.
pub const fn repository_name() -> &'static str {
    REPOSITORY
}

/// Returns public module names.
pub fn module_names() -> &'static [&'static str] {
    MODULES
}

/// Compact root view of the platform crate contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCatalog {
    /// Repository name.
    pub repository: &'static str,
    /// Compatibility alias.
    pub alias: &'static str,
    /// Crate version.
    pub version: &'static str,
    /// Feature bitmap.
    pub features: u64,
    /// Rights bitmap recognized by this crate version.
    pub rights: u64,
    /// Architecture schema version.
    pub arch_schema: &'static str,
    /// HAL schema version.
    pub hal_schema: &'static str,
    /// Interrupt schema version.
    pub interrupt_schema: &'static str,
    /// Timer schema version.
    pub timer_schema: &'static str,
    /// Paging schema version.
    pub paging_schema: &'static str,
}

impl PlatformCatalog {
    /// Current platform catalog.
    pub const CURRENT: Self = Self {
        repository: REPOSITORY,
        alias: ALIAS_ARCH,
        version: VERSION,
        features: PLATFORM_KNOWN_FEATURES,
        rights: PLATFORM_KNOWN_RIGHTS,
        arch_schema: ARCH_SCHEMA_VERSION,
        hal_schema: HAL_SCHEMA_VERSION,
        interrupt_schema: INTERRUPT_SCHEMA_VERSION,
        timer_schema: TIMER_SCHEMA_VERSION,
        paging_schema: PAGING_SCHEMA_VERSION,
    };

    /// Validates catalog metadata.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.repository.is_empty()
            || self.alias.is_empty()
            || self.version.is_empty()
            || self.arch_schema.is_empty()
            || self.hal_schema.is_empty()
            || self.interrupt_schema.is_empty()
            || self.timer_schema.is_empty()
            || self.paging_schema.is_empty()
        {
            return Err(PlatformError::MissingField);
        }
        if self.features & !PLATFORM_KNOWN_FEATURES != 0
            || self.rights & !PLATFORM_KNOWN_RIGHTS != 0
        {
            return Err(PlatformError::ReservedBits);
        }
        Ok(())
    }
}

/// Current platform catalog.
pub const PLATFORM_CATALOG: PlatformCatalog = PlatformCatalog::CURRENT;

/// Returns the current platform catalog.
pub const fn platform_catalog() -> PlatformCatalog {
    PlatformCatalog::CURRENT
}

/// Validates redaction state for a data class.
pub const fn validate_redaction(
    data_class: DataClass,
    redaction: RedactionState,
) -> PlatformResult<()> {
    match data_class {
        DataClass::Public => {
            if matches!(redaction, RedactionState::Public) {
                Ok(())
            } else {
                Err(PlatformError::InvalidRedaction)
            }
        }
        DataClass::Operational => {
            if matches!(redaction, RedactionState::Operational) {
                Ok(())
            } else {
                Err(PlatformError::InvalidRedaction)
            }
        }
        DataClass::Sensitive => {
            if matches!(
                redaction,
                RedactionState::SensitiveRedacted | RedactionState::SecretRedacted
            ) {
                Ok(())
            } else {
                Err(PlatformError::InvalidRedaction)
            }
        }
        DataClass::Secret => {
            if matches!(redaction, RedactionState::SecretRedacted) {
                Ok(())
            } else {
                Err(PlatformError::InvalidRedaction)
            }
        }
    }
}

/// Validates a stable platform label.
pub fn validate_platform_label(label: &str, max_len: usize) -> PlatformResult<()> {
    if label.is_empty() {
        return Err(PlatformError::MissingField);
    }
    if label.len() > max_len {
        return Err(PlatformError::FieldTooLong);
    }
    if !label.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.' | b'/' | b'@')
    }) {
        return Err(PlatformError::InvalidLabel);
    }
    Ok(())
}

/// Validates that `value` is aligned to `alignment`.
pub const fn validate_alignment(value: u64, alignment: u64) -> PlatformResult<()> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(PlatformError::InvalidAddress);
    }
    if value & (alignment - 1) != 0 {
        return Err(PlatformError::Unaligned);
    }
    Ok(())
}

/// Returns `true` when two half-open ranges overlap.
pub const fn ranges_overlap(
    a_start: u64,
    a_len: u64,
    b_start: u64,
    b_len: u64,
) -> PlatformResult<bool> {
    let Some(a_end) = a_start.checked_add(a_len) else {
        return Err(PlatformError::InvalidAddress);
    };
    let Some(b_end) = b_start.checked_add(b_len) else {
        return Err(PlatformError::InvalidAddress);
    };
    if a_len == 0 || b_len == 0 {
        return Err(PlatformError::InvalidAddress);
    }
    Ok(a_start < b_end && b_start < a_end)
}
