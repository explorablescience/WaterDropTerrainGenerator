use waterdrop_terrain_generator::core::node;

#[test]
fn every_expected_node_type_is_registered() {
    let labels: Vec<&str> = node::registered_nodes().map(|d| d.label).collect();
    for expected in [
        "Erosion",
        "Flat",
        "Perlin",
        "Load File",
        "Mountain",
        "Export"
    ] {
        assert!(
            labels.contains(&expected),
            "expected '{}' to be registered, got {:?}",
            expected,
            labels
        );
    }
}

#[test]
fn registered_labels_are_unique() {
    let mut labels: Vec<&str> = node::registered_nodes().map(|d| d.label).collect();
    let unique_count = {
        labels.sort_unstable();
        labels.dedup();
        labels.len()
    };
    let total_count = node::registered_nodes().count();
    assert_eq!(
        unique_count, total_count,
        "two node types are registered under the same label"
    );
}

#[test]
fn every_descriptor_factory_builds_a_node_with_that_descriptors_category() {
    for descriptor in node::registered_nodes() {
        let instance = (descriptor.factory)();
        assert_eq!(
            instance.category(),
            descriptor.category,
            "'{}' node's category doesn't match its registry entry",
            descriptor.label
        );
    }
}
