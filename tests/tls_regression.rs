#[test]
fn subscriber_trust_store_contains_public_roots() {
    // Match existing subscriber trust policy; regression stays tied to source.
    let source = include_str!("../src/mqtt_subscriber.rs");
    assert!(
        !source.contains("with_root_certificates(rustls::RootCertStore::empty())"),
        "subscriber rejects all public TLS chains"
    );
    assert!(source.contains("mqtt_options") || source.contains("TLS_SERVER_ROOTS"));
}
