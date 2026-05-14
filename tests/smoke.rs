use alani_platform::{
    ArchDescriptor, Architecture, ArchitectureProfile, BootPhase, CpuFeatureSet, CpuProfile,
    CpuVendor, DataClass, HalDescriptor, HardwareProfile, InterruptBinding,
    InterruptControllerKind, InterruptDescriptor, InterruptFlags, InterruptTable, InterruptVector,
    MmioRegion, PageFlags, PageMapping, PageSize, PageTablePlan, PagingDescriptor, PagingMode,
    PagingPolicy, PhysicalAddress, PlatformError, PlatformRights, RedactionState,
    TimerCapabilities, TimerConfig, TimerDescriptor, TimerDevice, TimerKind, TimerMode,
    TraceContext, VirtualAddress, CPU_FEATURE_ATOMIC, CPU_FEATURE_MMU, PAGE_FLAG_EXECUTABLE,
    PAGE_FLAG_USER, TIMER_CAP_DEADLINE, TIMER_CAP_MONOTONIC, TIMER_CAP_WATCHDOG,
};

#[test]
fn repository_identity_and_catalog_are_stable() {
    assert_eq!(alani_platform::repository_name(), "alani-platform");
    assert_eq!(
        alani_platform::module_names(),
        &["arch", "hal", "interrupts", "timers", "paging"]
    );
    assert_eq!(alani_platform::ALIAS_ARCH, "alani-arch");
    assert!(alani_platform::platform_catalog().validate().is_ok());
}

#[test]
fn architecture_profiles_validate_features_and_trace_context() {
    let features = CpuFeatureSet(CPU_FEATURE_MMU | CPU_FEATURE_ATOMIC);
    let descriptor = ArchDescriptor::new("host", Architecture::Host).with_features(features);
    let cpu = CpuProfile::new(CpuVendor::Simulator, 2, 0, features, "host-cpu");
    let profile = ArchitectureProfile::new(descriptor, cpu);

    assert!(profile.validate().is_ok());
    assert!(profile.supports_paging());
    assert!(!profile.supports_smp());

    assert_eq!(
        CpuFeatureSet::from_bits(1 << 63),
        Err(PlatformError::ReservedBits)
    );
    assert_eq!(
        TraceContext::EMPTY.with_flags(1 << 31).validate(),
        Err(PlatformError::ReservedBits)
    );
}

#[test]
fn hal_boot_phase_and_mmio_metadata_fail_closed() {
    let hal = HalDescriptor::new("host-hal", HardwareProfile::HostSim);
    assert!(hal.validate().is_ok());
    assert!(BootPhase::Reset.can_transition_to(BootPhase::EarlyConsole));
    assert!(!BootPhase::Reset.can_transition_to(BootPhase::Paging));
    assert!(BootPhase::Ready.is_terminal());

    let mmio = MmioRegion::new("uart", 0x1000, 0x1000);
    assert!(mmio.validate().is_ok());

    let executable_mmio = mmio.executable(true);
    assert_eq!(
        executable_mmio.validate(),
        Err(PlatformError::UnsafeReviewRequired)
    );

    let unaligned_mmio = MmioRegion::new("bad-uart", 0x1001, 0x1000);
    assert_eq!(unaligned_mmio.validate(), Err(PlatformError::Unaligned));
}

#[test]
fn interrupt_table_enforces_rights_audit_and_duplicates() {
    let descriptor = InterruptDescriptor::new("host-ic", InterruptControllerKind::HostSim);
    let mut table = InterruptTable::<4>::new(descriptor);
    let binding = InterruptBinding::new(InterruptVector::new(48), 1, "timer")
        .with_flags(InterruptFlags::ENABLED.union(InterruptFlags::DEFERRED));

    assert_eq!(
        table.bind(PlatformRights::INTERRUPT_BIND, binding),
        Err(PlatformError::AuditRequired)
    );

    let rights = PlatformRights::INTERRUPT_BIND.union(PlatformRights::AUDIT);
    table.bind(rights, binding).unwrap();
    assert_eq!(table.len(), 1);
    assert_eq!(
        table.bind(rights, binding),
        Err(PlatformError::InvalidInterrupt)
    );
    assert!(table.find(InterruptVector::new(48)).is_ok());
    assert!(table.validate().is_ok());
}

#[test]
fn timer_device_checks_frequency_mode_and_audit() {
    let descriptor = TimerDescriptor::new("lapic", TimerKind::Deadline, 1_000_000);
    let caps = TimerCapabilities(TIMER_CAP_MONOTONIC | TIMER_CAP_DEADLINE);
    let mut timer = TimerDevice::new(descriptor, caps);
    let config = TimerConfig::deadline(100, Some(InterruptVector::new(48)));

    timer
        .configure(PlatformRights::TIMER_CONFIGURE, config)
        .unwrap();
    timer.arm(PlatformRights::TIMER_CONFIGURE).unwrap();
    assert_eq!(timer.state, alani_platform::TimerState::Armed);

    let bad_descriptor = TimerDescriptor::new("bad-timer", TimerKind::Monotonic, 0);
    assert_eq!(bad_descriptor.validate(), Err(PlatformError::InvalidTimer));

    let mut watchdog = TimerDevice::new(
        TimerDescriptor::new("watchdog", TimerKind::Watchdog, 1_000),
        TimerCapabilities(TIMER_CAP_MONOTONIC | TIMER_CAP_DEADLINE | TIMER_CAP_WATCHDOG),
    );
    assert_eq!(
        watchdog.configure(
            PlatformRights::TIMER_CONFIGURE,
            TimerConfig::watchdog(100, None)
        ),
        Err(PlatformError::AuditRequired)
    );

    let unsupported = TimerConfig {
        mode: TimerMode::Periodic,
        interval_ticks: 100,
        deadline_ticks: 0,
        irq_vector: None,
        requires_audit: false,
    };
    assert_eq!(
        timer.configure(PlatformRights::TIMER_CONFIGURE, unsupported),
        Err(PlatformError::UnsupportedFeature)
    );
}

#[test]
fn paging_plan_rejects_overlap_wx_user_device_and_sealed_mutation() {
    let descriptor = PagingDescriptor::new("host-paging", PagingMode::HostSimulated);
    let mut plan = PageTablePlan::<4>::new(descriptor, PagingPolicy::HOST_FIXTURE);
    let rights = PlatformRights::MAP_MEMORY.union(PlatformRights::ADMIN);

    let text = PageMapping::new(
        "kernel-text",
        VirtualAddress::new(0x1000),
        PhysicalAddress::new(0x2000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::KERNEL_RX,
    );
    plan.map(rights, text).unwrap();

    let overlapping = PageMapping::new(
        "overlap",
        VirtualAddress::new(0x1000),
        PhysicalAddress::new(0x3000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::KERNEL_RO,
    );
    assert_eq!(plan.map(rights, overlapping), Err(PlatformError::Overlap));

    let writable_executable = PageMapping::new(
        "wx",
        VirtualAddress::new(0x4000),
        PhysicalAddress::new(0x4000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::KERNEL_RW.union(PageFlags(PAGE_FLAG_EXECUTABLE)),
    );
    assert_eq!(
        writable_executable.validate(descriptor, PagingPolicy::HOST_FIXTURE),
        Err(PlatformError::UnsafeReviewRequired)
    );

    let user_device = PageMapping::new(
        "user-device",
        VirtualAddress::new(0x8000),
        PhysicalAddress::new(0x8000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::DEVICE.union(PageFlags(PAGE_FLAG_USER)),
    );
    assert_eq!(
        user_device.validate(descriptor, PagingPolicy::HOST_FIXTURE),
        Err(PlatformError::UnsafeReviewRequired)
    );

    let sensitive_user = PageMapping::new(
        "sensitive",
        VirtualAddress::new(0xc000),
        PhysicalAddress::new(0xc000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::USER_RW,
    )
    .classified(DataClass::Sensitive, RedactionState::SensitiveRedacted)
    .with_page_size(PageSize::Size4KiB);
    assert_eq!(
        sensitive_user.validate(descriptor, PagingPolicy::HOST_FIXTURE),
        Err(PlatformError::InvalidPaging)
    );

    plan.seal(rights).unwrap();
    let data = PageMapping::new(
        "kernel-data",
        VirtualAddress::new(0x10_0000),
        PhysicalAddress::new(0x10_0000),
        alani_platform::PAGE_SIZE_4KIB,
        PageFlags::KERNEL_RW,
    );
    assert_eq!(plan.map(rights, data), Err(PlatformError::ReadOnly));
}
