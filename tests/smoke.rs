#[test]
fn repository_identity_is_stable() {
    assert_eq!(alani_platform::repository_name(), "alani-platform");
    assert!(!alani_platform::module_names().is_empty());
}
