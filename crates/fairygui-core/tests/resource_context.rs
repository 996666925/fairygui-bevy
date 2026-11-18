use fairygui_core::{
    async_build::AsyncBuilder,
    object::ObjectStore,
    package::{
        FuiPackage, LoadContext, ObjectType, PackageItemType, PackageRegistry, ResourceContext,
    },
    widget::{create_from_package, create_from_package_with_registry},
};

fn package() -> FuiPackage {
    FuiPackage::parse(
        "ResourceVariants",
        include_bytes!("fixtures/ResourceVariants.fui"),
    )
    .unwrap()
}

#[test]
fn variants_keep_base_identity_and_branch_geometry_while_selecting_hd_content() {
    let package = package();
    let mut store = ObjectStore::new();
    let mut registry = PackageRegistry::new();
    registry.set_branch("night");
    registry.set_content_scale_level(1);
    let context = registry.load_context(&package);
    for (kind, hd) in [
        (PackageItemType::Image, "v0hd"),
        (PackageItemType::MovieClip, "v1hd"),
    ] {
        let index = package
            .items()
            .iter()
            .position(|item| item.item_type == kind)
            .unwrap();
        let base = package.item(index).unwrap();
        let object = create_from_package(&mut store, &package, index, &context).unwrap();
        let object = store.object(object).unwrap();
        assert_eq!(object.package_item_id(), Some(base.id.as_str()));
        assert_eq!(object.content_item_id(), Some(hd));
        assert_eq!((object.width(), object.height()), (60., 30.));
        assert_eq!(object.resource_context().branch, "night");
        assert_eq!(object.resource_context().content_scale_level, 1);
    }
    let index = package.item_index_by_name("Demo_Popup").unwrap();
    let object = create_from_package(&mut store, &package, index, &context).unwrap();
    let object = store.object(object).unwrap();
    assert_eq!(
        object.package_item_id(),
        Some(package.item(index).unwrap().id.as_str())
    );
    assert_eq!(
        object.content_item_id(),
        Some("v3night"),
        "components select branches without HD"
    );
    assert_eq!(object.source_size(), fairygui_core::Size::new(60., 30.));
}

#[test]
fn movie_and_bitmap_glyphs_select_variant_data_but_keep_logical_glyph_metrics() {
    let package = package();
    let mut context = ResourceContext {
        branch: "night".into(),
        content_scale_level: 1,
    }
    .load_context(&package);
    let movie = package
        .items()
        .iter()
        .position(|item| item.item_type == PackageItemType::MovieClip)
        .unwrap();
    assert_eq!(
        package
            .load_movie_clip(movie, &LoadContext::default())
            .unwrap()
            .interval,
        100
    );
    assert_eq!(
        package.load_movie_clip(movie, &context).unwrap().interval,
        300
    );
    // Missing HD levels fall back to the selected branch, rather than another density.
    context.content_scale_level = 2;
    assert_eq!(
        package.load_movie_clip(movie, &context).unwrap().interval,
        150
    );
    let font = package.item_index_by_name("HitNumber").unwrap();
    assert_eq!(
        package
            .load_bitmap_font(font, &LoadContext::default())
            .unwrap()
            .font_size,
        24
    );
    let selected = package.load_bitmap_font(font, &context).unwrap();
    assert_eq!(selected.font_size, 36);
    context.content_scale_level = 1;
    let selected = package.load_bitmap_font(font, &context).unwrap();
    let hd = package.item_index_by_id("v0hd").unwrap();
    assert!(selected.glyphs.values().any(|glyph| {
        glyph.width == 60. && glyph.height == 30.
            && matches!(glyph.texture, Some(fairygui_core::package::GlyphTexture::Item { item }) if item == hd)
    }));
}

#[test]
fn global_changes_affect_new_requests_and_leave_existing_resource_selections_intact() {
    let package = package();
    let mut store = ObjectStore::new();
    let index = package
        .items()
        .iter()
        .position(|item| item.item_type == PackageItemType::Image)
        .unwrap();
    let image = create_from_package(&mut store, &package, index, &LoadContext::default()).unwrap();
    let loader = store.create(ObjectType::Loader);
    let field = store.create(ObjectType::Text);
    store.apply_icon_to(loader, "ui://ResourceVariants/grid_header_png");
    store.set_object_font(field, Some("ui://ResourceVariants/HitNumber"));
    let next = ResourceContext {
        branch: "night".into(),
        content_scale_level: 1,
    };
    store.set_resource_defaults(next.clone());
    assert_eq!(
        store.object(image).unwrap().resource_context(),
        &ResourceContext::default()
    );
    assert_eq!(
        store.object(loader).unwrap().resource_context(),
        &ResourceContext::default()
    );
    assert_eq!(
        store.object(field).unwrap().resource_context(),
        &ResourceContext::default()
    );
    store.apply_icon_to(loader, "ui://ResourceVariants/other");
    store.set_object_font(field, Some("ui://ResourceVariants/other-font"));
    assert_eq!(store.object(loader).unwrap().resource_context(), &next);
    assert_eq!(store.object(field).unwrap().resource_context(), &next);
    let new = store.create(ObjectType::Loader);
    assert_eq!(store.object(new).unwrap().resource_context(), &next);
    assert_eq!(
        store.object(image).unwrap().content_item_id(),
        Some(package.item(index).unwrap().id.as_str())
    );
}

#[test]
fn asynchronous_children_retain_the_requested_context_across_updates() {
    let package = package();
    let mut store = ObjectStore::new();
    let index = package.item_index_by_name("Demo_Popup").unwrap();
    let requested = ResourceContext {
        branch: "night".into(),
        content_scale_level: 1,
    };
    let context = requested.load_context(&package);
    let mut builder = AsyncBuilder::new([index], 1);
    assert!(!builder.advance(&mut store, &package, &context));
    store.set_resource_defaults(ResourceContext {
        branch: "day".into(),
        content_scale_level: 0,
    });
    let mut updates = 0;
    while !builder.advance(&mut store, &package, &LoadContext::none()) {
        updates += 1;
        assert!(updates < 1000);
    }
    assert_eq!(builder.failures(), 0);
    let root = builder.roots()[0];
    assert_eq!(
        store.object(root).unwrap().content_item_id(),
        Some("v3night")
    );
    store.for_each_descendant(root, &mut |id| {
        assert_eq!(store.object(id).unwrap().resource_context(), &requested);
    });
}

#[test]
fn global_branch_name_survives_packages_without_a_matching_local_index() {
    let package = package();
    let other = FuiPackage::parse("Basics", include_bytes!("fixtures/Basics.fui")).unwrap();
    let mut registry = PackageRegistry::new();
    registry.set_branch("night");
    let missing = registry.load_context(&other);
    assert_eq!(missing.branch_index, -1);
    let retained = missing.resources(&other);
    assert_eq!(retained.branch, "night");
    assert_eq!(retained.load_context(&package).branch_index, 1);
    assert_eq!(LoadContext::default().branch_index, -1);
    for (scale, expected) in [
        (1., 0),
        (1.4999, 0),
        (1.5, 1),
        (2.4999, 1),
        (2.5, 2),
        (3.4999, 2),
        (3.5, 3),
        (5., 3),
    ] {
        assert_eq!(ResourceContext::scale_level(scale), expected);
    }
}

#[test]
fn dependency_components_map_global_branch_names_in_both_construction_paths() {
    let package = std::sync::Arc::new(package());
    let host = std::sync::Arc::new(
        FuiPackage::parse("ResourceHost", include_bytes!("fixtures/ResourceHost.fui")).unwrap(),
    );
    let mut registry = PackageRegistry::new();
    registry.add(host.clone(), "ResourceHost");
    registry.add(package.clone(), "ResourceVariants");
    registry.set_branch("night");
    registry.set_content_scale_level(1);
    let context = registry.load_context(&host);
    assert_eq!(context.branch_index, -1);
    let index = host.item_index_by_name("Demo_Popup").unwrap();
    let mut store = ObjectStore::new();
    let direct =
        create_from_package_with_registry(&mut store, &host, index, &context, &registry).unwrap();
    let mut builder = AsyncBuilder::new([index], 1);
    let mut updates = 0;
    while !builder.advance_with_registry(&mut store, &host, &context, &registry) {
        updates += 1;
        assert!(updates < 1000);
    }
    assert_eq!(builder.failures(), 0);
    for root in [direct, builder.roots()[0]] {
        let nested = store.object(root).unwrap().children()[0];
        let object = store.object(nested).unwrap();
        assert_eq!(object.source_package_id(), Some("variants"));
        assert_eq!(
            object.package_item_id(),
            Some(package.item_by_name("Demo_Popup").unwrap().id.as_str())
        );
        assert_eq!(object.content_item_id(), Some("v3night"));
        assert_eq!(object.source_size(), fairygui_core::Size::new(60., 30.));
        assert_eq!(object.resource_context().branch, "night");
    }
}
