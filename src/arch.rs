//! Architecture and CPU feature contracts.

use crate::{
    validate_platform_label, validate_redaction, DataClass, PlatformError, PlatformResult,
    RedactionState, TraceContext,
};

/// Architecture metadata schema emitted by this crate version.
pub const ARCH_SCHEMA_VERSION: &str = "alani.platform.arch.v1";
/// Maximum architecture descriptor name length.
pub const MAX_ARCH_NAME_LEN: usize = 96;
/// Maximum CPU profile label length.
pub const MAX_CPU_LABEL_LEN: usize = 96;

/// CPU supports an MMU.
pub const CPU_FEATURE_MMU: u64 = 1 << 0;
/// CPU supports floating-point state.
pub const CPU_FEATURE_FPU: u64 = 1 << 1;
/// CPU supports symmetric multiprocessing.
pub const CPU_FEATURE_SMP: u64 = 1 << 2;
/// CPU supports virtualization extensions.
pub const CPU_FEATURE_VIRT: u64 = 1 << 3;
/// CPU supports non-executable page permissions.
pub const CPU_FEATURE_NX: u64 = 1 << 4;
/// CPU supports SSE2 or an equivalent host-mode SIMD baseline.
pub const CPU_FEATURE_SSE2: u64 = 1 << 5;
/// CPU exposes APIC-compatible interrupt delivery.
pub const CPU_FEATURE_APIC: u64 = 1 << 6;
/// CPU exposes an invariant or platform time-stamp counter.
pub const CPU_FEATURE_TSC: u64 = 1 << 7;
/// CPU supports RISC-V Sv39 paging.
pub const CPU_FEATURE_SV39: u64 = 1 << 8;
/// CPU supports RISC-V Sv48 paging.
pub const CPU_FEATURE_SV48: u64 = 1 << 9;
/// CPU supports the RISC-V SBI platform contract.
pub const CPU_FEATURE_RISCV_SBI: u64 = 1 << 10;
/// CPU supports RISC-V AIA interrupt delivery.
pub const CPU_FEATURE_AIA: u64 = 1 << 11;
/// CPU supports atomic memory operations.
pub const CPU_FEATURE_ATOMIC: u64 = 1 << 12;
/// CPU supports instruction-cache fence operations.
pub const CPU_FEATURE_FENCE_I: u64 = 1 << 13;

/// All CPU feature bits known by this crate version.
pub const KNOWN_CPU_FEATURES: u64 = CPU_FEATURE_MMU
    | CPU_FEATURE_FPU
    | CPU_FEATURE_SMP
    | CPU_FEATURE_VIRT
    | CPU_FEATURE_NX
    | CPU_FEATURE_SSE2
    | CPU_FEATURE_APIC
    | CPU_FEATURE_TSC
    | CPU_FEATURE_SV39
    | CPU_FEATURE_SV48
    | CPU_FEATURE_RISCV_SBI
    | CPU_FEATURE_AIA
    | CPU_FEATURE_ATOMIC
    | CPU_FEATURE_FENCE_I;

/// Supported architecture family.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    /// Host-mode simulator profile.
    Host = 0,
    /// x86_64 architecture.
    X86_64 = 1,
    /// RISC-V 64-bit architecture.
    Riscv64 = 2,
}

impl Architecture {
    /// Returns the architecture label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::X86_64 => "x86_64",
            Self::Riscv64 => "riscv64",
        }
    }

    /// Returns default required feature bits for an MVK-capable profile.
    pub const fn required_features(self) -> CpuFeatureSet {
        match self {
            Self::Host => CpuFeatureSet(CPU_FEATURE_MMU | CPU_FEATURE_ATOMIC),
            Self::X86_64 => CpuFeatureSet(
                CPU_FEATURE_MMU
                    | CPU_FEATURE_NX
                    | CPU_FEATURE_SSE2
                    | CPU_FEATURE_APIC
                    | CPU_FEATURE_TSC
                    | CPU_FEATURE_ATOMIC,
            ),
            Self::Riscv64 => CpuFeatureSet(
                CPU_FEATURE_MMU
                    | CPU_FEATURE_SV39
                    | CPU_FEATURE_RISCV_SBI
                    | CPU_FEATURE_ATOMIC
                    | CPU_FEATURE_FENCE_I,
            ),
        }
    }
}

/// Endianness of architecture-native scalar values.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endianness {
    /// Little-endian native order.
    Little = 0,
    /// Big-endian native order.
    Big = 1,
}

/// Architecture word width.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WordWidth {
    /// 32-bit profile.
    Bits32 = 32,
    /// 64-bit profile.
    Bits64 = 64,
}

/// Privilege level understood by platform contracts.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PrivilegeLevel {
    /// Machine or firmware-level execution.
    Machine = 0,
    /// Hypervisor-level execution.
    Hypervisor = 1,
    /// Supervisor/kernel-level execution.
    Kernel = 2,
    /// User-level execution.
    User = 3,
}

/// Execution mode attached to boot and kernel profiles.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionMode {
    /// Privilege level.
    pub privilege: PrivilegeLevel,
    /// Whether paging is enabled in this mode.
    pub paging_enabled: bool,
    /// Whether interrupts are enabled in this mode.
    pub interrupts_enabled: bool,
}

impl ExecutionMode {
    /// Kernel execution mode with paging and interrupts disabled during early setup.
    pub const EARLY_KERNEL: Self = Self {
        privilege: PrivilegeLevel::Kernel,
        paging_enabled: false,
        interrupts_enabled: false,
    };

    /// Runtime kernel execution mode.
    pub const KERNEL: Self = Self {
        privilege: PrivilegeLevel::Kernel,
        paging_enabled: true,
        interrupts_enabled: true,
    };

    /// User execution mode.
    pub const USER: Self = Self {
        privilege: PrivilegeLevel::User,
        paging_enabled: true,
        interrupts_enabled: true,
    };
}

/// CPU vendor or profile source.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuVendor {
    /// Intel-compatible CPU.
    Intel = 0,
    /// AMD-compatible CPU.
    Amd = 1,
    /// RISC-V CPU.
    Riscv = 2,
    /// SBI-provided RISC-V machine profile.
    Sbi = 3,
    /// Host simulator.
    Simulator = 4,
    /// Unknown vendor.
    Unknown = 5,
}

/// CPU feature bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuFeatureSet(pub u64);

impl CpuFeatureSet {
    /// Empty feature set.
    pub const NONE: Self = Self(0);
    /// Creates a feature set after rejecting reserved bits.
    pub const fn from_bits(bits: u64) -> PlatformResult<Self> {
        if bits & !KNOWN_CPU_FEATURES != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(Self(bits))
        }
    }

    /// Returns raw feature bits.
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Returns `true` when all required features are present.
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    /// Combines two feature sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Validates reserved bits.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 & !KNOWN_CPU_FEATURES != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(())
        }
    }
}

/// Architecture descriptor used by HAL and boot code.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchDescriptor<'a> {
    /// Descriptor label.
    pub name: &'a str,
    /// Architecture family.
    pub architecture: Architecture,
    /// Native endianness.
    pub endianness: Endianness,
    /// Native word width.
    pub word_width: WordWidth,
    /// CPU feature bits expected by the descriptor.
    pub features: CpuFeatureSet,
    /// Schema version.
    pub schema: &'static str,
    /// Descriptor metadata classification.
    pub data_class: DataClass,
    /// Descriptor metadata redaction state.
    pub redaction: RedactionState,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> ArchDescriptor<'a> {
    /// Creates an architecture descriptor with MVK-oriented defaults.
    pub const fn new(name: &'a str, architecture: Architecture) -> Self {
        Self {
            name,
            architecture,
            endianness: Endianness::Little,
            word_width: WordWidth::Bits64,
            features: architecture.required_features(),
            schema: ARCH_SCHEMA_VERSION,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets CPU feature bits.
    pub const fn with_features(mut self, features: CpuFeatureSet) -> Self {
        self.features = features;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates architecture descriptor metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_ARCH_NAME_LEN)?;
        if self.schema != ARCH_SCHEMA_VERSION {
            return Err(PlatformError::UnsupportedArchitecture);
        }
        self.features.validate()?;
        if self.word_width != WordWidth::Bits64 {
            return Err(PlatformError::UnsupportedArchitecture);
        }
        if self.endianness != Endianness::Little {
            return Err(PlatformError::UnsupportedArchitecture);
        }
        if !self
            .features
            .contains(self.architecture.required_features())
        {
            return Err(PlatformError::UnsupportedFeature);
        }
        validate_redaction(self.data_class, self.redaction)?;
        self.trace.validate()
    }
}

/// CPU topology and feature profile.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuProfile<'a> {
    /// Vendor or profile source.
    pub vendor: CpuVendor,
    /// CPU family value supplied by the platform.
    pub family: u16,
    /// CPU model value supplied by the platform.
    pub model: u16,
    /// Number of harts or logical CPUs.
    pub hart_count: u16,
    /// Boot hart/logical CPU identifier.
    pub boot_hart_id: u16,
    /// CPU feature set.
    pub features: CpuFeatureSet,
    /// Human-readable profile label.
    pub label: &'a str,
}

impl<'a> CpuProfile<'a> {
    /// Creates a CPU profile.
    pub const fn new(
        vendor: CpuVendor,
        hart_count: u16,
        boot_hart_id: u16,
        features: CpuFeatureSet,
        label: &'a str,
    ) -> Self {
        Self {
            vendor,
            family: 0,
            model: 0,
            hart_count,
            boot_hart_id,
            features,
            label,
        }
    }

    /// Sets family and model.
    pub const fn with_model(mut self, family: u16, model: u16) -> Self {
        self.family = family;
        self.model = model;
        self
    }

    /// Validates CPU profile metadata.
    pub fn validate(self) -> PlatformResult<()> {
        if self.hart_count == 0 || self.boot_hart_id >= self.hart_count {
            return Err(PlatformError::InvalidCpu);
        }
        self.features.validate()?;
        validate_platform_label(self.label, MAX_CPU_LABEL_LEN)
    }
}

/// Complete architecture profile used by boot and kernel setup.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchitectureProfile<'a> {
    /// Architecture descriptor.
    pub descriptor: ArchDescriptor<'a>,
    /// CPU profile.
    pub cpu: CpuProfile<'a>,
    /// Kernel execution mode.
    pub kernel_mode: ExecutionMode,
    /// User execution mode.
    pub user_mode: ExecutionMode,
    /// Maximum physical address width.
    pub max_physical_address_bits: u8,
    /// Maximum virtual address width.
    pub max_virtual_address_bits: u8,
    /// Cache line size in bytes.
    pub cache_line_bytes: u16,
}

impl<'a> ArchitectureProfile<'a> {
    /// Creates an architecture profile.
    pub const fn new(descriptor: ArchDescriptor<'a>, cpu: CpuProfile<'a>) -> Self {
        Self {
            descriptor,
            cpu,
            kernel_mode: ExecutionMode::KERNEL,
            user_mode: ExecutionMode::USER,
            max_physical_address_bits: 52,
            max_virtual_address_bits: 48,
            cache_line_bytes: 64,
        }
    }

    /// Returns `true` when the profile supports paging.
    pub const fn supports_paging(self) -> bool {
        self.descriptor.features.0 & CPU_FEATURE_MMU != 0
    }

    /// Returns `true` when the profile supports multiple harts/logical CPUs.
    pub const fn supports_smp(self) -> bool {
        self.cpu.hart_count > 1 && self.descriptor.features.0 & CPU_FEATURE_SMP != 0
    }

    /// Validates the architecture profile.
    pub fn validate(self) -> PlatformResult<()> {
        self.descriptor.validate()?;
        self.cpu.validate()?;
        if !self
            .cpu
            .features
            .contains(self.descriptor.architecture.required_features())
        {
            return Err(PlatformError::UnsupportedFeature);
        }
        if !self.descriptor.features.contains(self.cpu.features) {
            return Err(PlatformError::UnsupportedFeature);
        }
        if self.kernel_mode.privilege > PrivilegeLevel::Kernel
            || self.user_mode.privilege != PrivilegeLevel::User
        {
            return Err(PlatformError::UnsupportedArchitecture);
        }
        if self.max_physical_address_bits == 0
            || self.max_physical_address_bits > 56
            || self.max_virtual_address_bits == 0
            || self.max_virtual_address_bits > 57
        {
            return Err(PlatformError::InvalidCpu);
        }
        if self.cache_line_bytes == 0 || !self.cache_line_bytes.is_power_of_two() {
            return Err(PlatformError::InvalidCpu);
        }
        Ok(())
    }
}
