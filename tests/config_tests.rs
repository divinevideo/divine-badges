use divine_badges::config::validate_base_url;

#[test]
fn config_requires_non_empty_api_base_url() {
    let result = validate_base_url("");
    assert!(result.is_err());
}
