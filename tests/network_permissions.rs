use gamepkg::manifest::Manifest;

#[test]
fn network_boolean_and_legacy_modes_round_trip_as_a_boolean_without_endpoint_grants() {
    let original = include_str!("../examples/minimal/manifest.toml");
    for (value, enabled) in [
        ("false", false),
        ("true", true),
        ("\"none\"", false),
        ("\"https\"", true),
        ("\"multiplayer\"", true),
        ("\"all\"", true),
    ] {
        let text = original.replace("network = \"none\"", &format!("network = {value}\nnetwork_hosts = [\"old.invalid:1\"]\nnetwork_listen = [\"127.0.0.1:2\"]\nnetwork_allow_local = false"));
        let manifest = Manifest::parse(text.as_bytes()).unwrap();
        assert_eq!(manifest.permissions.network, enabled);
        let encoded = toml::to_string(&manifest).unwrap();
        assert!(encoded.contains(&format!("network = {enabled}")));
        for legacy in ["network_hosts", "network_listen", "network_allow_local"] {
            assert!(!encoded.contains(legacy));
        }
        assert_eq!(
            Manifest::parse(encoded.as_bytes())
                .unwrap()
                .permissions
                .network,
            enabled
        );
    }
}

#[test]
fn ambiguous_network_values_are_rejected() {
    let original = include_str!("../examples/minimal/manifest.toml");
    for value in ["1", "0", "\"true\"", "\"enabled\"", "[]", "{}"] {
        assert!(
            Manifest::parse(
                original
                    .replace("network = \"none\"", &format!("network = {value}"))
                    .as_bytes()
            )
            .is_err(),
            "{value}"
        );
    }
}
