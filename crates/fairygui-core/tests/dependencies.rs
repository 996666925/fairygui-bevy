//! Real exported component records with patched cross-package references.
use fairygui_core::{
    async_build::AsyncBuilder,
    object::ObjectStore,
    package::{FuiPackage, LoadContext, PackageRegistry},
    widget::{ControlData, create_from_package, create_from_package_with_registry},
};
use std::sync::Arc;

const HOST: &[u8] = include_bytes!("fixtures/PopupHost.fui");
const THEME: &[u8] = include_bytes!("fixtures/PopupTheme.fui");

fn packages() -> (Arc<FuiPackage>, Arc<FuiPackage>, PackageRegistry) {
    let host = Arc::new(FuiPackage::parse("Basics", HOST).unwrap());
    let theme = Arc::new(FuiPackage::parse("Basics", THEME).unwrap());
    let mut registry = PackageRegistry::new();
    registry.add(host.clone(), "host");
    registry.add(theme.clone(), "theme");
    (host, theme, registry)
}

#[test]
fn foreign_component_keeps_its_resource_owner_while_instance_data_uses_the_parent_table() {
    let (host, theme, registry) = packages();
    let mut store = ObjectStore::new();
    let root = create_from_package_with_registry(
        &mut store,
        &theme,
        theme.item_index_by_name("Demo_Popup").unwrap(),
        &LoadContext::none(),
        &registry,
    )
    .unwrap();
    let foreign = store.child_by_name(root, "n0").unwrap();
    let local = store.child_by_name(root, "n1").unwrap();
    assert_eq!(
        store.object(root).unwrap().source_package_id(),
        Some(theme.id())
    );
    assert_eq!(
        store.object(foreign).unwrap().source_package_id(),
        Some(host.id())
    );
    assert_eq!(
        store.object(local).unwrap().source_package_id(),
        Some(theme.id())
    );
    let title = store.child_by_name(foreign, "title").unwrap();
    assert_eq!(
        store.object(title).unwrap().source_package_id(),
        Some(host.id())
    );
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Popup Menu",
        "instance caption comes from the theme package string table"
    );
    assert_eq!(
        store.object(foreign).unwrap().width(),
        158.0,
        "instance size overrides the host template"
    );
    store.set_parent(foreign, None);
    assert_eq!(
        store.object(foreign).unwrap().source_package_id(),
        Some(host.id())
    );
}

#[test]
fn absent_foreign_package_uses_native_default_type_and_never_an_equal_local_item_id() {
    let (_, theme, _) = packages();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &theme,
        theme.item_index_by_name("Demo_Popup").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let child = store.child_by_name(root, "n0").unwrap();
    let child = store.object(child).unwrap();
    assert!(
        matches!(child.control(), ControlData::Component(_)),
        "serialized type is an ordinary component, not the referenced button extension"
    );
    assert_eq!(child.package_item_id(), None);
    assert!(child.children().is_empty());
    let local = store.child_by_name(root, "n1").unwrap();
    assert!(!store.object(local).unwrap().children().is_empty());
}

#[test]
fn foreign_image_and_async_construction_preserve_package_boundaries() {
    let (host, theme, registry) = packages();
    let mut store = ObjectStore::new();
    let index = theme.item_index_by_name("PopupMenu").unwrap();
    let mut builder = AsyncBuilder::new([index, index], 1);
    assert!(!builder.advance_with_registry(&mut store, &theme, &LoadContext::none(), &registry));
    assert!(builder.advance_with_registry(&mut store, &theme, &LoadContext::none(), &registry));
    assert_eq!(builder.failures(), 0);
    for root in builder.roots() {
        let image = store.child_by_name(*root, "n0").unwrap();
        let list = store.child_by_name(*root, "list").unwrap();
        assert_eq!(
            store.object(image).unwrap().source_package_id(),
            Some(host.id())
        );
        assert_eq!(
            store.object(list).unwrap().source_package_id(),
            Some(theme.id())
        );
    }
}

#[test]
fn cyclic_resource_references_fail_without_leaking_the_partial_tree_or_touching_existing_objects() {
    let package = FuiPackage::parse("Basics", include_bytes!("fixtures/PopupCycle.fui")).unwrap();
    let mut store = ObjectStore::new();
    let survivor = store.create(fairygui_core::package::ObjectType::Component);
    let error = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Popup").unwrap(),
        &LoadContext::none(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("cyclic component reference"),
        "{error}"
    );
    assert!(store.contains(survivor));
    assert_eq!(store.len(), 1);
}

#[test]
fn malformed_or_non_package_urls_are_rejected_without_utf8_boundary_panics() {
    let (host, _, registry) = packages();
    for url in [
        "http://PopupHost/PopupMenu",
        "ui://你好你好x",
        "ui://",
        "ui://missing/resource",
    ] {
        assert!(registry.resolve_url(url).is_none(), "{url}");
        assert!(host.resolve_local_url(url).is_none(), "{url}");
    }
}

#[test]
fn combo_instance_title_and_runtime_selection_reach_the_real_text_and_icon_children() {
    let (host, _, registry) = packages();
    let mut store = ObjectStore::new();
    let root = create_from_package_with_registry(
        &mut store,
        &host,
        host.item_index_by_name("Demo_ComboBox").unwrap(),
        &LoadContext::none(),
        &registry,
    )
    .unwrap();
    let combo = store.child_by_name(root, "n1").unwrap();
    let title = store.child_by_name(combo, "title").unwrap();
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Item 1",
        "initial serialized selection reaches title"
    );
    let icon = store.create(fairygui_core::package::ObjectType::Loader);
    store.set_name(icon, "icon");
    store.set_parent(icon, Some(combo));
    assert!(store.apply_icon_to(combo, "ui://PopupTheme/VtSeperator"));
    store.apply_title_of(combo);
    assert!(
        matches!(store.object(icon).unwrap().control(), ControlData::Loader(data) if data.url.as_deref() == Some("ui://PopupTheme/VtSeperator"))
    );
    assert!(store.select_combo_index(combo, 2));
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Item 3"
    );
    assert!(
        matches!(store.object(icon).unwrap().control(), ControlData::Loader(data) if data.url.as_deref() == Some("")),
        "an item without an icon clears the old image"
    );
    store.set_object_text(combo, "Custom");
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Custom"
    );
    store.select_combo_index(combo, -1);
    assert_eq!(
        store
            .object(title)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        ""
    );
}
