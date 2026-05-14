//! Hardware abstraction layer descriptors and boot sequencing contracts.

use crate::{
    arch::Architecture, validate_alignment, validate_platform_label, validate_redaction, DataClass,
    PlatformError, PlatformResult, RedactionState, TraceContext,
};

/// HAL metadata schema emitted by this crate version.
pub const HAL_SCHEMA_VERSION: &str = "alani.platform.hal.v1";
/// Maximum MMIO region length represented by this skeleton.
pub const MAX_MMIO_REGION_LEN: u64 = 1 << 40;
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
