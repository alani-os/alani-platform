//! Timer descriptor, configuration, and snapshot contracts.

use crate::{
    interrupts::InterruptVector, validate_platform_label, validate_redaction, DataClass,
    PlatformError, PlatformResult, PlatformRights, RedactionState, TraceContext,
};

/// Timer metadata schema emitted by this crate version.
pub const TIMER_SCHEMA_VERSION: &str = "alani.platform.timers.v1";
/// Maximum timer label length.
pub const MAX_TIMER_NAME_LEN: usize = 96;
/// Maximum sane timer frequency represented by this crate version.
pub const MAX_TIMER_FREQUENCY_HZ: u64 = 10_000_000_000;

/// Timer supports monotonic snapshots.
pub const TIMER_CAP_MONOTONIC: u64 = 1 << 0;
/// Timer supports absolute deadlines.
pub const TIMER_CAP_DEADLINE: u64 = 1 << 1;
/// Timer supports periodic mode.
pub const TIMER_CAP_PERIODIC: u64 = 1 << 2;
/// Timer supports watchdog mode.
pub const TIMER_CAP_WATCHDOG: u64 = 1 << 3;
/// Timer is backed by x86 TSC.
pub const TIMER_CAP_TSC: u64 = 1 << 4;
/// Timer is backed by HPET.
pub const TIMER_CAP_HPET: u64 = 1 << 5;
/// Timer is backed by APIC deadline/timer hardware.
pub const TIMER_CAP_APIC: u64 = 1 << 6;
/// Timer is backed by RISC-V SBI timer.
pub const TIMER_CAP_RISCV_SBI: u64 = 1 << 7;

/// All timer capability bits known by this crate version.
pub const TIMER_KNOWN_CAPABILITIES: u64 = TIMER_CAP_MONOTONIC
    | TIMER_CAP_DEADLINE
    | TIMER_CAP_PERIODIC
    | TIMER_CAP_WATCHDOG
    | TIMER_CAP_TSC
    | TIMER_CAP_HPET
    | TIMER_CAP_APIC
    | TIMER_CAP_RISCV_SBI;

/// Timer capability bitmap.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimerCapabilities(pub u64);

impl TimerCapabilities {
    /// Empty capability set.
    pub const NONE: Self = Self(0);
    /// MVK timer baseline.
    pub const MVK_REQUIRED: Self = Self(TIMER_CAP_MONOTONIC | TIMER_CAP_DEADLINE);

    /// Creates capabilities after rejecting reserved bits.
    pub const fn from_bits(bits: u64) -> PlatformResult<Self> {
        if bits & !TIMER_KNOWN_CAPABILITIES != 0 {
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
        if self.0 & !TIMER_KNOWN_CAPABILITIES != 0 {
            Err(PlatformError::ReservedBits)
        } else {
            Ok(())
        }
    }
}

/// Timer hardware kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerKind {
    /// Host-mode simulated timer.
    HostSim = 0,
    /// Monotonic counter.
    Monotonic = 1,
    /// Deadline timer.
    Deadline = 2,
    /// Periodic timer.
    Periodic = 3,
    /// Real-time clock.
    RealTime = 4,
    /// Watchdog timer.
    Watchdog = 5,
}

/// Timer operating mode.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerMode {
    /// Disabled timer.
    Disabled = 0,
    /// One-shot interval.
    OneShot = 1,
    /// Periodic interval.
    Periodic = 2,
    /// Absolute deadline.
    Deadline = 3,
    /// Watchdog mode.
    Watchdog = 4,
}

/// Timer lifecycle state.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerState {
    /// Timer is disabled.
    Disabled = 0,
    /// Timer is configured.
    Configured = 1,
    /// Timer is armed.
    Armed = 2,
    /// Timer expired.
    Expired = 3,
    /// Timer was cancelled.
    Cancelled = 4,
    /// Timer failed.
    Failed = 5,
}

/// Timer resolution.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerResolution {
    /// Nanoseconds per hardware tick.
    pub nanos_per_tick: u64,
}

impl TimerResolution {
    /// Creates timer resolution metadata.
    pub const fn new(nanos_per_tick: u64) -> Self {
        Self { nanos_per_tick }
    }

    /// Validates resolution.
    pub const fn validate(self) -> PlatformResult<()> {
        if self.nanos_per_tick == 0 {
            Err(PlatformError::InvalidTimer)
        } else {
            Ok(())
        }
    }
}

/// Timer descriptor.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerDescriptor<'a> {
    /// Timer label.
    pub name: &'a str,
    /// Schema version.
    pub schema: &'static str,
    /// Timer kind.
    pub kind: TimerKind,
    /// Timer frequency.
    pub frequency_hz: u64,
    /// Timer resolution.
    pub resolution: TimerResolution,
    /// Maximum deadline in nanoseconds.
    pub max_deadline_ns: u64,
    /// Descriptor metadata classification.
    pub data_class: DataClass,
    /// Descriptor metadata redaction state.
    pub redaction: RedactionState,
    /// Trace context.
    pub trace: TraceContext,
}

impl<'a> TimerDescriptor<'a> {
    /// Creates a timer descriptor.
    pub const fn new(name: &'a str, kind: TimerKind, frequency_hz: u64) -> Self {
        Self {
            name,
            schema: TIMER_SCHEMA_VERSION,
            kind,
            frequency_hz,
            resolution: TimerResolution::new(1),
            max_deadline_ns: 60_000_000_000,
            data_class: DataClass::Operational,
            redaction: RedactionState::Operational,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets timer resolution.
    pub const fn with_resolution(mut self, resolution: TimerResolution) -> Self {
        self.resolution = resolution;
        self
    }

    /// Sets maximum deadline.
    pub const fn with_max_deadline_ns(mut self, max_deadline_ns: u64) -> Self {
        self.max_deadline_ns = max_deadline_ns;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates timer descriptor metadata.
    pub fn validate(self) -> PlatformResult<()> {
        validate_platform_label(self.name, MAX_TIMER_NAME_LEN)?;
        if self.schema != TIMER_SCHEMA_VERSION
            || self.frequency_hz == 0
            || self.frequency_hz > MAX_TIMER_FREQUENCY_HZ
            || self.max_deadline_ns == 0
        {
            return Err(PlatformError::InvalidTimer);
        }
        self.resolution.validate()?;
        validate_redaction(self.data_class, self.redaction)?;
        self.trace.validate()
    }
}

/// Timer configuration.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerConfig {
    /// Timer mode.
    pub mode: TimerMode,
    /// Interval in hardware ticks.
    pub interval_ticks: u64,
    /// Absolute deadline in hardware ticks.
    pub deadline_ticks: u64,
    /// Optional interrupt vector used for expiry.
    pub irq_vector: Option<InterruptVector>,
    /// Whether durable audit evidence is required.
    pub requires_audit: bool,
}

impl TimerConfig {
    /// Disabled timer configuration.
    pub const DISABLED: Self = Self {
        mode: TimerMode::Disabled,
        interval_ticks: 0,
        deadline_ticks: 0,
        irq_vector: None,
        requires_audit: false,
    };

    /// Creates a one-shot configuration.
    pub const fn one_shot(interval_ticks: u64, irq_vector: Option<InterruptVector>) -> Self {
        Self {
            mode: TimerMode::OneShot,
            interval_ticks,
            deadline_ticks: 0,
            irq_vector,
            requires_audit: false,
        }
    }

    /// Creates a deadline configuration.
    pub const fn deadline(deadline_ticks: u64, irq_vector: Option<InterruptVector>) -> Self {
        Self {
            mode: TimerMode::Deadline,
            interval_ticks: 0,
            deadline_ticks,
            irq_vector,
            requires_audit: false,
        }
    }

    /// Creates a watchdog configuration.
    pub const fn watchdog(interval_ticks: u64, irq_vector: Option<InterruptVector>) -> Self {
        Self {
            mode: TimerMode::Watchdog,
            interval_ticks,
            deadline_ticks: 0,
            irq_vector,
            requires_audit: true,
        }
    }

    /// Validates timer configuration.
    pub fn validate(self) -> PlatformResult<()> {
        match self.mode {
            TimerMode::Disabled => {
                if self.interval_ticks != 0 || self.deadline_ticks != 0 {
                    return Err(PlatformError::InvalidTimer);
                }
            }
            TimerMode::OneShot | TimerMode::Periodic | TimerMode::Watchdog => {
                if self.interval_ticks == 0 || self.deadline_ticks != 0 {
                    return Err(PlatformError::InvalidTimer);
                }
            }
            TimerMode::Deadline => {
                if self.deadline_ticks == 0 || self.interval_ticks != 0 {
                    return Err(PlatformError::InvalidTimer);
                }
            }
        }
        if let Some(vector) = self.irq_vector {
            vector.validate()?;
        }
        Ok(())
    }
}

/// Timer device contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerDevice<'a> {
    /// Timer descriptor.
    pub descriptor: TimerDescriptor<'a>,
    /// Timer capabilities.
    pub capabilities: TimerCapabilities,
    /// Timer state.
    pub state: TimerState,
    /// Current timer configuration.
    pub config: TimerConfig,
}

impl<'a> TimerDevice<'a> {
    /// Creates a timer device.
    pub const fn new(descriptor: TimerDescriptor<'a>, capabilities: TimerCapabilities) -> Self {
        Self {
            descriptor,
            capabilities,
            state: TimerState::Disabled,
            config: TimerConfig::DISABLED,
        }
    }

    /// Configures the timer after authorization.
    pub fn configure(&mut self, rights: PlatformRights, config: TimerConfig) -> PlatformResult<()> {
        rights.require(PlatformRights::TIMER_CONFIGURE)?;
        if config.requires_audit {
            rights
                .require(PlatformRights::AUDIT)
                .map_err(|_| PlatformError::AuditRequired)?;
        }
        self.descriptor.validate()?;
        self.capabilities.validate()?;
        config.validate()?;
        self.validate_mode_supported(config.mode)?;
        self.config = config;
        self.state = if config.mode == TimerMode::Disabled {
            TimerState::Disabled
        } else {
            TimerState::Configured
        };
        Ok(())
    }

    /// Arms a configured timer.
    pub fn arm(&mut self, rights: PlatformRights) -> PlatformResult<()> {
        rights.require(PlatformRights::TIMER_CONFIGURE)?;
        if self.state != TimerState::Configured {
            return Err(PlatformError::InvalidState);
        }
        self.state = TimerState::Armed;
        Ok(())
    }

    /// Cancels an armed or configured timer.
    pub fn cancel(&mut self, rights: PlatformRights) -> PlatformResult<()> {
        rights.require(PlatformRights::TIMER_CONFIGURE)?;
        if !matches!(self.state, TimerState::Configured | TimerState::Armed) {
            return Err(PlatformError::InvalidState);
        }
        self.state = TimerState::Cancelled;
        Ok(())
    }

    /// Validates timer device metadata.
    pub fn validate(self) -> PlatformResult<()> {
        self.descriptor.validate()?;
        self.capabilities.validate()?;
        if !self.capabilities.contains(TimerCapabilities::MVK_REQUIRED) {
            return Err(PlatformError::UnsupportedFeature);
        }
        self.config.validate()?;
        self.validate_mode_supported(self.config.mode)
    }

    fn validate_mode_supported(self, mode: TimerMode) -> PlatformResult<()> {
        match mode {
            TimerMode::Disabled => Ok(()),
            TimerMode::OneShot | TimerMode::Deadline => {
                if self.capabilities.0 & TIMER_CAP_DEADLINE == 0 {
                    Err(PlatformError::UnsupportedFeature)
                } else {
                    Ok(())
                }
            }
            TimerMode::Periodic => {
                if self.capabilities.0 & TIMER_CAP_PERIODIC == 0 {
                    Err(PlatformError::UnsupportedFeature)
                } else {
                    Ok(())
                }
            }
            TimerMode::Watchdog => {
                if self.capabilities.0 & TIMER_CAP_WATCHDOG == 0 {
                    Err(PlatformError::UnsupportedFeature)
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Timer diagnostic snapshot.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerSnapshot {
    /// Hardware tick value.
    pub tick: u64,
    /// Monotonic nanoseconds.
    pub monotonic_ns: u64,
    /// Drift in parts per million.
    pub drift_ppm: i32,
    /// Trace context.
    pub trace: TraceContext,
}

impl TimerSnapshot {
    /// Creates a timer snapshot.
    pub const fn new(tick: u64, monotonic_ns: u64) -> Self {
        Self {
            tick,
            monotonic_ns,
            drift_ppm: 0,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets drift.
    pub const fn with_drift_ppm(mut self, drift_ppm: i32) -> Self {
        self.drift_ppm = drift_ppm;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates snapshot metadata.
    pub fn validate(self) -> PlatformResult<()> {
        if self.monotonic_ns == 0 || self.tick == 0 {
            return Err(PlatformError::InvalidTimer);
        }
        if self.drift_ppm < -1_000_000 || self.drift_ppm > 1_000_000 {
            return Err(PlatformError::InvalidTimer);
        }
        self.trace.validate()
    }
}

/// Timer and preemption counters exported for scheduling diagnostics.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerCounters {
    /// Timer expiry events delivered.
    pub expirations: u64,
    /// Expiry events that missed their requested deadline.
    pub missed_deadlines: u64,
    /// Context switches attributed to this timer source.
    pub context_switches: u64,
    /// Preemption events attributed to this timer source.
    pub preemptions: u64,
    /// Trace context.
    pub trace: TraceContext,
}

impl TimerCounters {
    /// Creates timer counters.
    pub const fn new(expirations: u64) -> Self {
        Self {
            expirations,
            missed_deadlines: 0,
            context_switches: 0,
            preemptions: 0,
            trace: TraceContext::EMPTY,
        }
    }

    /// Sets missed deadline count.
    pub const fn with_missed_deadlines(mut self, missed_deadlines: u64) -> Self {
        self.missed_deadlines = missed_deadlines;
        self
    }

    /// Sets context switch count.
    pub const fn with_context_switches(mut self, context_switches: u64) -> Self {
        self.context_switches = context_switches;
        self
    }

    /// Sets preemption count.
    pub const fn with_preemptions(mut self, preemptions: u64) -> Self {
        self.preemptions = preemptions;
        self
    }

    /// Sets trace context.
    pub const fn with_trace(mut self, trace: TraceContext) -> Self {
        self.trace = trace;
        self
    }

    /// Validates timer diagnostic counters.
    pub fn validate(self) -> PlatformResult<()> {
        if self.missed_deadlines > self.expirations || self.preemptions > self.context_switches {
            return Err(PlatformError::InvalidTimer);
        }
        self.trace.validate()
    }
}
