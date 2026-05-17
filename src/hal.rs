//! Hardware abstraction layer descriptors and boot sequencing contracts.

use crate::{
    arch::Architecture, validate_alignment, validate_platform_label, validate_redaction, DataClass,
    PlatformError, PlatformResult, PlatformRights, RedactionState, TraceContext,
};

/// HAL metadata schema emitted by this crate version.
pub const HAL_SCHEMA_VERSION: &str = "alani.platform.hal.v1";
/// Maximum MMIO region length represented by this crate version.
pub const MAX_MMIO_REGION_LEN: u64 = 1 << 40;
/// Maximum DMA window length represented by this crate version.
pub const MAX_DMA_WINDOW_LEN: u64 = 1 << 40;
/// Maximum HAL or boot component label length.
pub const MAX_HAL_LABEL_LEN: usize = 96;

/// HAL supports timer hardware.
pub const HAL_CAP_TIMERS: u64 = 1 << 0;
/// HAL supports interrupt controllers.
pub const HAL_CAP_INTERRUPTS: u64 = 1 << 1;
/// HAL supports paging.
pub const HAL_CAP_PAGING: u64 = 1 << 2;
/// HAL supports MMIO metadata.
pub const HAL_CAP_MMIO: u64 = 1 << 3;
/// HAL supports symmetric multiprocessing.
pub const HAL_CAP_SMP: u64 = 1 << 4;
/// HAL supports serial or early console output.
pub const HAL_CAP_SERIAL_CONSOLE: u64 = 1 << 5;
/// HAL supports DMA boundary metadata.
pub const HAL_CAP_DMA: u64 = 1 << 6;
/// HAL supports power control operations.
pub const HAL_CAP_POWER: u64 = 1 << 7;

/// All HAL capability bits known by this crate version.
pub const HAL_KNOWN_CAPABILITIES: u64 = HAL_CAP_TIMERS
    | HAL_CAP_INTERRUPTS
    | HAL_CAP_PAGING
    | HAL_CAP_MMIO
    | HAL_CAP_SMP
    | HAL_CAP_SERIAL_CONSOLE
    | HAL_CAP_DMA
    | HAL_CAP_POWER;

/// HAL capability bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HalCapabilities(pub u64);

impl HalCapabilities {
    /// Empty capability set.
    pub const NONE: Self = Self(0);
    /// MVK-capable HAL baseline.
    pub const MVK_REQUIRED: Self = Self(
        HAL_CAP_TIMERS
            | HAL_CAP_INTERRUPTS
            | HAL_CAP_PAGING
            | HAL_CAP_MMIO
            | HAL_CAP_SERIAL_CONSOLE,
    );

    /// Creates capabilities after rejecting reserved bits.
    pub const fn from_bits(bits: u64) -> PlatformResult<Self> {
        if bits & !HAL_KNOWN_CAPABILITIES != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(Self(bits))
        }
    }

    /// Returns raw capability bits.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns `true` when all required capabilities are present.
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Combines two capability sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Validates reserved bits.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 & !HAL_KNOWN_CAPABILITIES != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(())
        }
    }
}

/// Hardware profile represented by the HAL.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareProfile {
    /// Host-mode simulator.
    HostSim = 0,
    /// QEMU x86_64 profile.
    QemuX86_64 = 1,
    /// QEMU RISC-V 64-bit profile.
    QemuRiscv64 = 2,
    /// Bare-metal x86_64 profile.
    BareMetalX86_64 = 3,
    /// Bare-metal RISC-V 64-bit profile.
    BareMetalRiscv64 = 4,
}

impl HardwareProfile {
    /// Architecture family expected by this hardware profile.
    pub const fn architecture(self) -> Architecture {
        match self {
            Self::HostSim => Architecture::Host,
            Self::QemuX86_64 | Self::BareMetalX86_64 => Architecture::X86_64,
            Self::QemuRiscv64 | Self::BareMetalRiscv64 => Architecture::Riscv64,
        }
    }
}

/// Cache policy for MMIO or memory-like platform regions.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachePolicy {
    /// Uncached access.
    Uncached = 0,
    /// Write-combining access.
    WriteCombining = 1,
    /// Write-through cached access.
    WriteThrough = 2,
    /// Write-back cached access.
    WriteBack = 3,
}

/// Direction of device DMA relative to system memory.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaDirection {
    /// Device reads from memory.
    ToDevice = 0,
    /// Device writes to memory.
    FromDevice = 1,
    /// Device may both read and write memory.
    Bidirectional = 2,
}

/// Deterministic boot phase.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum BootPhase {
    /// Reset or handoff entry.
    Reset = 0,
    /// Early console setup.
    EarlyConsole = 1,
    /// CPU feature validation.
    CpuFeatures = 2,
    /// Memory map parse and preservation.
    MemoryMap = 3,
    /// Page table setup.
    Paging = 4,
    /// Interrupt descriptor or controller setup.
    Interrupts = 5,
    /// Timer setup.
    Timers = 6,
    /// Device discovery or probe.
    Devices = 7,
    /// Runtime spawn.
    Runtime = 8,
    /// Platform ready.
    Ready = 9,
    /// Terminal failure.
    Failed = 10,
}

impl BootPhase {
    /// Returns `true` when `next` is a valid deterministic successor.
    pub const fn can_transition_to(self, next: Self) -> bool {
        if matches!(self, Self::Failed | Self::Ready) {
            return false;
        }
        matches!(next, Self::Failed) || next as u8 == self as u8 + 1
    }

    /// Returns `true` when the phase is terminal.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

/// HAL descriptor used by boot and kernel code.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalDescriptor<'a> {
    /// HAL label.
    pub name: &'a str,
    /// Schema version.
    pub schema: &'static str,
    /// Architecture family.
    pub architecture: Architecture,
    /// Hardware profile.
    pub hardware: HardwareProfile,
    /// HAL capabilities.
    pub capabilities: HalCapabilities,
    /// HAL metadata classification.
    pub data_class: DataClass,
    /// HAL metadata redaction state.
    pub redaction: RedactionState,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> HalDescriptor<'a> {
    /// Creates a HAL descriptor.
    pub const fn new(name: &'a str, hardware: HardwareProfile) -> Self {
        Self {
            name,
            schema: HAL_SCHEMA_VERSION,
            architecture: hardware.architecture(),
            hardware,
            capabilities: HalCapabilities::MVK_REQUIRED,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets capabilities.
    pub const fn with_capabilities(mut self, capabilities: HalCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates HAL metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_HAL_LABEL_LEN)?;
        if self.schema != HAL_SCHEMA_VERSION {
            return Err(PlatformError::InvalidHal);
        }
        if self.architecture != self.hardware.architecture() {
            return Err(PlatformError::UnsupportedArchitecture);
        }
        self.capabilities.validate()?;
        if !self.capabilities.contains(HalCapabilities::MVK_REQUIRED) {
            return Err(PlatformError::UnsupportedFeature);
        }
        validate_redaction(self.data_class, self.redaction)?;
        self.trace.validate()
    }
}

/// Memory-mapped IO region metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MmioRegion<'a> {
    /// Region label.
    pub name: &'a str,
    /// Physical base address.
    pub base: u64,
    /// Region length in bytes.
    pub length: u64,
    /// Cache policy.
    pub cache_policy: CachePolicy,
    /// Whether writes are allowed.
    pub writable: bool,
    /// Whether execution is allowed.
    pub executable: bool,
    /// Region metadata classification.
    pub data_class: DataClass,
    /// Region metadata redaction state.
    pub redaction: RedactionState,
}

impl<'a> MmioRegion<'a> {
    /// Creates an MMIO region.
    pub const fn new(name: &'a str, base: u64, length: u64) -> Self {
        Self {
            name,
            base,
            length,
            cache_policy: CachePolicy::Uncached,
            writable: true,
            executable: false,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
        }
    }

    /// Sets cache policy.
    pub const fn with_cache_policy(mut self, cache_policy: CachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    /// Sets write permission.
    pub const fn writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    /// Sets executable permission.
    pub const fn executable(mut self, executable: bool) -> Self {
        self.executable = executable;
        self
    }

    /// Validates MMIO metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_HAL_LABEL_LEN)?;
        if self.length == 0 || self.length > MAX_MMIO_REGION_LEN {
            return Err(PlatformError::InvalidHal);
        }
        validate_alignment(self.base, 4096)?;
        validate_alignment(self.length, 4096)?;
        if self.executable {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        if matches!(self.cache_policy, CachePolicy::WriteBack) {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        validate_redaction(self.data_class, self.redaction)
    }
}

/// Bounded DMA memory window supplied to platform or device code.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DmaWindow<'a> {
    /// Window label.
    pub name: &'a str,
    /// Physical start address.
    pub physical_start: u64,
    /// Window length in bytes.
    pub length: u64,
    /// Maximum single transfer length in bytes.
    pub max_transfer_len: u64,
    /// DMA direction relative to system memory.
    pub direction: DmaDirection,
    /// Whether the backing memory is pinned for the transfer lifetime.
    pub pinned: bool,
    /// Whether an IOMMU-like mapping constrains the device view.
    pub iommu_mapped: bool,
    /// Window metadata classification.
    pub data_class: DataClass,
    /// Window metadata redaction state.
    pub redaction: RedactionState,
}

impl<'a> DmaWindow<'a> {
    /// Creates a bounded DMA window.
    pub const fn new(
        name: &'a str,
        physical_start: u64,
        length: u64,
        direction: DmaDirection,
    ) -> Self {
        Self {
            name,
            physical_start,
            length,
            max_transfer_len: length,
            direction,
            pinned: true,
            iommu_mapped: false,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
        }
    }

    /// Sets maximum single transfer length.
    pub const fn with_max_transfer_len(mut self, max_transfer_len: u64) -> Self {
        self.max_transfer_len = max_transfer_len;
        self
    }

    /// Marks whether memory is pinned for the transfer lifetime.
    pub const fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Marks whether an IOMMU-like mapping constrains the device view.
    pub const fn iommu_mapped(mut self, iommu_mapped: bool) -> Self {
        self.iommu_mapped = iommu_mapped;
        self
    }

    /// Sets classification and redaction state.
    pub const fn classified(mut self, data_class: DataClass, redaction: RedactionState) -> Self {
        self.data_class = data_class;
        self.redaction = redaction;
        self
    }

    /// Validates DMA metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_HAL_LABEL_LEN)?;
        if self.length == 0
            || self.length > MAX_DMA_WINDOW_LEN
            || self.max_transfer_len == 0
            || self.max_transfer_len > self.length
        {
            return Err(PlatformError::InvalidHal);
        }
        validate_alignment(self.physical_start, 4096)?;
        validate_alignment(self.length, 4096)?;
        if !self.pinned {
            return Err(PlatformError::InvalidState);
        }
        if matches!(self.data_class, DataClass::Secret) && !self.iommu_mapped {
            return Err(PlatformError::UnsafeReviewRequired);
        }
        validate_redaction(self.data_class, self.redaction)
    }
}

/// Single boot sequencing step.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootStep<'a> {
    /// Boot phase.
    pub phase: BootPhase,
    /// Component label.
    pub component: &'a str,
    /// Whether the phase completed.
    pub completed: bool,
    /// Whether durable audit evidence is required.
    pub requires_audit: bool,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> BootStep<'a> {
    /// Creates a boot step.
    pub const fn new(phase: BootPhase, component: &'a str) -> Self {
        Self {
            phase,
            component,
            completed: false,
            requires_audit: matches!(
                phase,
                BootPhase::CpuFeatures
                    | BootPhase::MemoryMap
                    | BootPhase::Paging
                    | BootPhase::Interrupts
                    | BootPhase::Timers
            ),
            trace: TraceContext::EMPTY,
        }
    }

    /// Marks the step completed or incomplete.
    pub const fn completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates boot step metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.component, MAX_HAL_LABEL_LEN)?;
        self.trace.validate()
    }
}

/// Fixed-capacity deterministic boot plan or boot log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootPlan<'a, const N: usize> {
    steps: [Option<BootStep<'a>>; N],
    len: usize,
    sealed: bool,
}

impl<'a, const N: usize> BootPlan<'a, N> {
    /// Creates an empty boot plan.
    pub const fn new() -> Self {
        Self {
            steps: [None; N],
            len: 0,
            sealed: false,
        }
    }

    /// Returns plan capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns step count.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no steps are present.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` when the plan has been sealed.
    pub const fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Returns boot step slots.
    pub const fn steps(&self) -> &[Option<BootStep<'a>>; N] {
        &self.steps
    }

    /// Appends a completed boot step after authorization and ordering checks.
    pub fn push(&mut self, rights: PlatformRights, step: BootStep<'a>) -> PlatformResult<()> {
        rights.require(PlatformRights::CONFIGURE)?;
        if self.sealed {
            return Err(PlatformError::ReadOnly);
        }
        if step.requires_audit {
            rights
                .require(PlatformRights::AUDIT)
                .map_err(|_| PlatformError::AuditRequired)?;
        }
        step.validate()?;
        if !step.completed {
            return Err(PlatformError::InvalidState);
        }
        if self.len >= N {
            return Err(PlatformError::CapacityExceeded);
        }
        if self.len == 0 {
            if step.phase != BootPhase::Reset {
                return Err(PlatformError::InvalidState);
            }
        } else {
            let previous = self.steps[self.len - 1].ok_or(PlatformError::Internal)?;
            if !previous.phase.can_transition_to(step.phase) {
                return Err(PlatformError::InvalidState);
            }
        }
        self.steps[self.len] = Some(step);
        self.len += 1;
        Ok(())
    }

    /// Seals the boot plan after validating a terminal phase.
    pub fn seal(&mut self, rights: PlatformRights) -> PlatformResult<()> {
        rights.require(PlatformRights::CONFIGURE)?;
        self.validate()?;
        let last = self.steps[self.len - 1].ok_or(PlatformError::MissingField)?;
        if !last.phase.is_terminal() {
            return Err(PlatformError::InvalidState);
        }
        self.sealed = true;
        Ok(())
    }

    /// Validates boot-plan ordering and step metadata.
    pub fn validate(&self) -> PlatformResult<()> {
        if self.len == 0 || self.len > N {
            return Err(PlatformError::MissingField);
        }
        let mut count = 0;
        let mut previous_phase: Option<BootPhase> = None;
        let mut index = 0;
        while index < N {
            if let Some(step) = self.steps[index] {
                step.validate()?;
                if !step.completed {
                    return Err(PlatformError::InvalidState);
                }
                if count == 0 {
                    if step.phase != BootPhase::Reset {
                        return Err(PlatformError::InvalidState);
                    }
                } else {
                    let previous = previous_phase.ok_or(PlatformError::Internal)?;
                    if !previous.can_transition_to(step.phase) {
                        return Err(PlatformError::InvalidState);
                    }
                }
                previous_phase = Some(step.phase);
                count += 1;
            }
            index += 1;
        }
        if count != self.len {
            return Err(PlatformError::InvalidHal);
        }
        Ok(())
    }
}

impl<'a, const N: usize> Default for BootPlan<'a, N> {
    fn default() -> Self {
        Self::new()
    }
}
