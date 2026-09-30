use jobs::runtime::{AMAZON_TRUST_SERVICES_ROOTS, amazon_tls_context};

#[test]
fn the_embedded_trust_store_holds_exactly_the_amazon_trust_services_roots() {
    // ARRANGE
    let pem = std::str::from_utf8(AMAZON_TRUST_SERVICES_ROOTS).unwrap();

    // ACT
    let labels: Vec<&str> = pem.lines().filter_map(|line| line.strip_prefix("# ")).collect();

    // ASSERT
    assert_eq!(
        labels,
        [
            "Amazon Root CA 1",
            "Amazon Root CA 2",
            "Amazon Root CA 3",
            "Amazon Root CA 4",
            "Starfield Services Root Certificate Authority - G2"
        ]
    );
    assert_eq!(pem.matches("-----BEGIN CERTIFICATE-----").count(), 5);
}

#[test]
fn the_embedded_roots_form_a_valid_tls_context() {
    // ARRANGE
    let roots = AMAZON_TRUST_SERVICES_ROOTS;

    // ACT
    let context = std::panic::catch_unwind(amazon_tls_context);

    // ASSERT
    assert!(!roots.is_empty());
    assert!(context.is_ok());
}
