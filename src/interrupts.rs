//! Interrupt controller, vector, binding, and event contracts.

use crate::{
    validate_platform_label, validate_redaction, DataClass, PlatformError, PlatformResult,
    PlatformRights, RedactionState, TraceContext,
};

/// Interrupt metadata schema emitted by this crate version.
pub const INTERRUPT_SCHEMA_VERSION: &str = "alani.platform.interrupts.v1";
/// Maximum interrupt vectors represented by this crate version.
pub const MAX_INTERRUPT_VECTORS: u16 = 256;
/// Maximum interrupt label length.
pub const MAX_INTERRUPT_NAME_LEN: usize = 96;
/// Maximum external IRQ number represented by this crate version.
pub const MAX_IRQ_NUMBER: u32 = 4095;

/// Binding is enabled.
pub const INTERRUPT_FLAG_ENABLED: u32 = 1 << 0;
/// Binding is masked.
pub const INTERRUPT_FLAG_MASKED: u32 = 1 << 1;
/// Binding may share an IRQ line.
pub const INTERRUPT_FLAG_SHARED: u32 = 1 << 2;
/// Binding defers heavy work to non-interrupt context.
pub const INTERRUPT_FLAG_DEFERRED: u32 = 1 << 3;
/// Binding represents a non-maskable interrupt.
pub const INTERRUPT_FLAG_NMI: u32 = 1 << 4;

/// All interrupt flags known by this crate version.
pub const INTERRUPT_KNOWN_FLAGS: u32 = INTERRUPT_FLAG_ENABLED
    | INTERRUPT_FLAG_MASKED
    | INTERRUPT_FLAG_SHARED
    | INTERRUPT_FLAG_DEFERRED
    | INTERRUPT_FLAG_NMI;

/// Interrupt controller kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptControllerKind {
    /// Host-mode simulated interrupt controller.
    HostSim = 0,
    /// xAPIC local controller.
    XApic = 1,
    /// x2APIC local controller.
    X2Apic = 2,
    /// I/O APIC controller.
    IoApic = 3,
    /// RISC-V PLIC controller.
    RiscvPlic = 4,
    /// RISC-V AIA controller.
    RiscvAia = 5,
}

/// Interrupt trigger mode.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptTrigger {
    /// Edge triggered.
    Edge = 0,
    /// Level triggered.
    Level = 1,
}

/// Interrupt polarity.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptPolarity {
    /// Active high signal.
    ActiveHigh = 0,
    /// Active low signal.
    ActiveLow = 1,
}

/// Interrupt vector wrapper.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct InterruptVector(pub u16);

impl InterruptVector {
    /// Creates an interrupt vector.
    pub const fn new(vector: u16) -> Self {
        Self(vector)
    }

    /// Validates vector range.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 >= MAX_INTERRUPT_VECTORS {
            Err(PlatformError::InvalidInterrupt)
        } else {
            Ok(())
        }
    }
}

/// Interrupt flag bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InterruptFlags(pub u32);

impl InterruptFlags {
    /// No interrupt flags.
    pub const NONE: Self = Self(0);
    /// Enabled interrupt binding.
    pub const ENABLED: Self = Self(INTERRUPT_FLAG_ENABLED);
    /// Deferred interrupt binding.
    pub const DEFERRED: Self = Self(INTERRUPT_FLAG_DEFERRED);

    /// Creates flags after rejecting reserved bits.
    pub const fn from_bits(bits: u32) -> PlatformResult<Self> {
        if bits & !INTERRUPT_KNOWN_FLAGS != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(Self(bits))
        }
    }

    /// Returns raw flag bits.
    pub const fn bits(self) -> u32 {
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

    /// Validates flag invariants.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.0 & !INTERRUPT_KNOWN_FLAGS != 0 {
            return Err(PlatformError::ReservedBits);
        }
        if self.0 & INTERRUPT_FLAG_ENABLED != 0 && self.0 & INTERRUPT_FLAG_MASKED != 0 {
            return Err(PlatformError::InvalidInterrupt);
        }
        if self.0 & INTERRUPT_FLAG_NMI != 0 && self.0 & INTERRUPT_FLAG_SHARED != 0 {
            return Err(PlatformError::InvalidInterrupt);
        }
        Ok(())
    }
}

/// Interrupt controller descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptDescriptor<'a> {
    /// Controller label.
    pub name: &'a str,
    /// Schema version.
    pub schema: &'static str,
    /// Controller kind.
    pub controller: InterruptControllerKind,
    /// Supported vector count.
    pub vectors_supported: u16,
    /// Descriptor metadata classification.
    pub data_class: DataClass,
    /// Descriptor metadata redaction state.
    pub redaction: RedactionState,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> InterruptDescriptor<'a> {
    /// Creates an interrupt controller descriptor.
    pub const fn new(name: &'a str, controller: InterruptControllerKind) -> Self {
        Self {
            name,
            schema: INTERRUPT_SCHEMA_VERSION,
            controller,
            vectors_supported: MAX_INTERRUPT_VECTORS,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets supported vector count.
    pub const fn with_vectors_supported(mut self, vectors_supported: u16) -> Self {
        self.vectors_supported = vectors_supported;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates interrupt descriptor metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_INTERRUPT_NAME_LEN)?;
        if self.schema != INTERRUPT_SCHEMA_VERSION
            || self.vectors_supported == 0
            || self.vectors_supported > MAX_INTERRUPT_VECTORS
        {
            return Err(PlatformError::InvalidInterrupt);
        }
        validate_redaction(self.data_class, self.redaction)?;
        self.trace.validate()
    }
}

/// Interrupt binding metadata.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptBinding<'a> {
    /// Interrupt vector.
    pub vector: InterruptVector,
    /// External IRQ number.
    pub irq: u32,
    /// Binding label.
    pub name: &'a str,
    /// Trigger mode.
    pub trigger: InterruptTrigger,
    /// Polarity.
    pub polarity: InterruptPolarity,
    /// Priority, where larger values are more urgent.
    pub priority: u8,
    /// Interrupt flags.
    pub flags: InterruptFlags,
    /// Target CPU or hart.
    pub target_cpu: u16,
    /// Whether durable audit evidence is required.
    pub requires_audit: bool,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> InterruptBinding<'a> {
    /// Creates an interrupt binding.
    pub const fn new(vector: InterruptVector, irq: u32, name: &'a str) -> Self {
        Self {
            vector,
            irq,
            name,
            trigger: InterruptTrigger::Edge,
            polarity: InterruptPolarity::ActiveHigh,
            priority: 0,
            flags: InterruptFlags::ENABLED,
            target_cpu: 0,
            requires_audit: true,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets flags.
    pub const fn with_flags(mut self, flags: InterruptFlags) -> Self {
        self.flags = flags;
        self
    }

    /// Sets target CPU.
    pub const fn with_target_cpu(mut self, target_cpu: u16) -> Self {
        self.target_cpu = target_cpu;
        self
    }

    /// Sets trigger and polarity.
    pub const fn with_signal(
        mut self,
        trigger: InterruptTrigger,
        polarity: InterruptPolarity,
    ) -> Self {
        self.trigger = trigger;
        self.polarity = polarity;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates interrupt binding metadata.
    pub fn validate(self) -> PlatformResult<()> {
        self.vector.validate()?;
        if self.irq > MAX_IRQ_NUMBER {
            return Err(PlatformError::InvalidInterrupt);
        }
        validate_platform_label(self.name, MAX_INTERRUPT_NAME_LEN)?;
        self.flags.validate()?;
        if self.requires_audit && !self.flags.contains(InterruptFlags::DEFERRED) {
            return Err(PlatformError::AuditRequired);
        }
        self.trace.validate()
    }
}

/// Interrupt event captured by a top half.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptEvent {
    /// Interrupt vector.
    pub vector: InterruptVector,
    /// External IRQ number.
    pub irq: u32,
    /// Monotonic event counter.
    pub counter: u64,
    /// Whether hardware was acknowledged.
    pub acknowledged: bool,
    /// Whether work was deferred to non-interrupt context.
    pub deferred: bool,
    /// Trace context.
    pub trace: TraceContext,
}

impl InterruptEvent {
    /// Creates an interrupt event.
    pub const fn new(vector: InterruptVector, irq: u32, counter: u64) -> Self {
        Self {
            vector,
            irq,
            counter,
            acknowledged: false,
            deferred: true,
            trace: TraceContext::EMPTY,
        }
    }

    /// Marks the event acknowledged.
    pub const fn acknowledged(mut self, acknowledged: bool) -> Self {
        self.acknowledged = acknowledged;
        self
    }

    /// Marks whether non-trivial work was deferred out of interrupt context.
    pub const fn deferred(mut self, deferred: bool) -> Self {
        self.deferred = deferred;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates interrupt event metadata.
    pub fn validate(self) -> PlatformResult<()> {
        self.vector.validate()?;
        if self.irq > MAX_IRQ_NUMBER || self.counter == 0 {
            return Err(PlatformError::InvalidInterrupt);
        }
        if !self.acknowledged || !self.deferred {
            return Err(PlatformError::InvalidState);
        }
        self.trace.validate()
    }
}

/// Fixed-capacity interrupt binding table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptTable<'a, const N: usize> {
    /// Controller descriptor.
    pub descriptor: InterruptDescriptor<'a>,
    entries: [Option<InterruptBinding<'a>>; N],
    len: usize,
    read_only: bool,
}

impl<'a, const N: usize> InterruptTable<'a, N> {
    /// Creates an empty interrupt table.
    pub const fn new(descriptor: InterruptDescriptor<'a>) -> Self {
        Self {
            descriptor,
            entries: [None; N],
            len: 0,
            read_only: false,
        }
    }

    /// Marks the table read-only or mutable.
    pub const fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Returns table capacity.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Returns binding count.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when no bindings are present.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns binding slots.
    pub const fn entries(&self) -> &[Option<InterruptBinding<'a>>; N] {
        &self.entries
    }

    /// Binds an interrupt after authorization and duplicate checks.
    pub fn bind(
        &mut self,
        rights: PlatformRights,
        binding: InterruptBinding<'a>,
    ) -> PlatformResult<()> {
        rights.require(PlatformRights::INTERRUPT_BIND)?;
        if self.read_only {
            return Err(PlatformError::ReadOnly);
        }
        if binding.requires_audit {
            rights
                .require(PlatformRights::AUDIT)
                .map_err(|_| PlatformError::AuditRequired)?;
        }
        self.descriptor.validate()?;
        binding.validate()?;
        if binding.vector.0 >= self.descriptor.vectors_supported {
            return Err(PlatformError::InvalidInterrupt);
        }
        if self.len >= N {
            return Err(PlatformError::CapacityExceeded);
        }
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                if entry.vector == binding.vector || entry.irq == binding.irq {
                    return Err(PlatformError::InvalidInterrupt);
                }
            }
            index += 1;
        }
        self.entries[self.len] = Some(binding);
        self.len += 1;
        Ok(())
    }

    /// Finds a binding by vector.
    pub fn find(&self, vector: InterruptVector) -> PlatformResult<InterruptBinding<'a>> {
        vector.validate()?;
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                if entry.vector == vector {
                    return Ok(entry);
                }
            }
            index += 1;
        }
        Err(PlatformError::InvalidInterrupt)
    }

    /// Unbinds a vector.
    pub fn unbind(
        &mut self,
        rights: PlatformRights,
        vector: InterruptVector,
    ) -> PlatformResult<InterruptBinding<'a>> {
        rights.require(PlatformRights::INTERRUPT_BIND)?;
        if self.read_only {
            return Err(PlatformError::ReadOnly);
        }
        vector.validate()?;
        let mut remove_index = None;
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                if entry.vector == vector {
                    remove_index = Some(index);
                    break;
                }
            }
            index += 1;
        }
        let remove_index = remove_index.ok_or(PlatformError::InvalidInterrupt)?;
        let removed = self.entries[remove_index].ok_or(PlatformError::Internal)?;
        self.entries[remove_index] = None;
        self.len -= 1;
        self.compact();
        Ok(removed)
    }

    /// Validates table invariants.
    pub fn validate(&self) -> PlatformResult<()> {
        self.descriptor.validate()?;
        if self.len > N {
            return Err(PlatformError::InvalidInterrupt);
        }
        let mut count = 0;
        let mut index = 0;
        while index < N {
            if let Some(entry) = self.entries[index] {
                entry.validate()?;
                count += 1;
                let mut other = index + 1;
                while other < N {
                    if let Some(other_entry) = self.entries[other] {
                        if entry.vector == other_entry.vector || entry.irq == other_entry.irq {
                            return Err(PlatformError::InvalidInterrupt);
                        }
                    }
                    other += 1;
                }
            }
            index += 1;
        }
        if count != self.len {
            return Err(PlatformError::InvalidInterrupt);
        }
        Ok(())
    }

    fn compact(&mut self) {
        let mut write = 0;
        let mut read = 0;
        while read < N {
            if let Some(entry) = self.entries[read] {
                if write != read {
                    self.entries[write] = Some(entry);
                    self.entries[read] = None;
                }
                write += 1;
            }
            read += 1;
        }
    }
}
