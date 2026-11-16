use fairygui_core::{
    async_build::AsyncBuilder,
    object::{GObjectId, ObjectStore},
    package::{FuiPackage, LoadContext, ObjectType, PackageItemType},
    widget::create_from_package,
};

fn snapshot(store: &ObjectStore, root: GObjectId) -> String {
    fn walk(store: &ObjectStore, id: GObjectId, output: &mut String) {
        use std::fmt::Write;
        let o = store.object(id).unwrap();
        writeln!(
            output,
            "{:?}",
            (
                o.kind(),
                o.instance_id(),
                o.name(),
                o.source_package_id(),
                o.package_item_id(),
                o.x(),
                o.y(),
                o.size(),
                (
                    o.scale_x(),
                    o.scale_y(),
                    o.rotation(),
                    o.alpha(),
                    o.visible(),
                    o.grayed(),
                    o.touchable()
                )
            )
        )
        .unwrap();
        writeln!(
            output,
            "{} {:?}",
            o.control().kind_name(),
            o.control().as_text_field()
        )
        .unwrap();
        if let Some(component) = o.control().component() {
            for c in &component.controllers {
                writeln!(
                    output,
                    "{:?}",
                    (&c.name, &c.page_ids, &c.page_names, c.selected_index)
                )
                .unwrap();
            }
            writeln!(
                output,
                "{:?}",
                (
                    component.margin,
                    component.overflow,
                    component.mask_child,
                    component.mask_reversed,
                    component.transitions.len()
                )
            )
            .unwrap();
        }
        for &child in o.children() {
            walk(store, child, output);
        }
        output.push_str("end\n");
    }
    let mut result = String::new();
    walk(store, root, &mut result);
    result
}

#[test]
fn one_object_budget_matches_synchronous_exported_trees_and_never_exposes_partial_roots() {
    for (name, bytes) in [
        (
            "MainMenu",
            include_bytes!("fixtures/MainMenu.fui").as_slice(),
        ),
        ("Basics", include_bytes!("fixtures/Basics.fui").as_slice()),
        (
            "PopupTheme",
            include_bytes!("fixtures/PopupTheme.fui").as_slice(),
        ),
        (
            "TextTemplate",
            include_bytes!("fixtures/TextTemplate.fui").as_slice(),
        ),
    ] {
        let package = FuiPackage::parse(name, bytes).unwrap();
        for (index, item) in package
            .items()
            .iter()
            .enumerate()
            .filter(|(_, item)| item.item_type == PackageItemType::Component)
        {
            let mut sync = ObjectStore::new();
            let root =
                create_from_package(&mut sync, &package, index, &LoadContext::none()).unwrap();
            let mut store = ObjectStore::new();
            let mut builder = AsyncBuilder::new([index], 1);
            for count in 1..=sync.len() {
                let finished = builder.advance(&mut store, &package, &LoadContext::none());
                assert_eq!(store.len(), count, "{name}/{}", item.name);
                assert_eq!(finished, count == sync.len());
                if !finished {
                    assert!(builder.roots().is_empty());
                }
                assert!(store.iter().all(|(id, _)| !store.is_on_stage(id)));
            }
            assert_eq!(builder.failures(), 0, "{name}/{}", item.name);
            assert_eq!(builder.processed_objects(), sync.len());
            assert_eq!(builder.planned_objects(), sync.len());
            assert_eq!(
                snapshot(&store, builder.roots()[0]),
                snapshot(&sync, root),
                "{name}/{}",
                item.name
            );
        }
    }
}

#[test]
fn cancellation_reclaims_prepared_subtrees_and_budget_edits_keep_existing_objects() {
    let package = FuiPackage::parse("MainMenu", include_bytes!("fixtures/MainMenu.fui")).unwrap();
    let mut store = ObjectStore::new();
    let survivor = store.create(ObjectType::Graph);
    let index = package.item_index_by_name("Main").unwrap();
    let mut builder = AsyncBuilder::new([index, index], 3);
    assert!(!builder.advance(&mut store, &package, &LoadContext::none()));
    let existing: Vec<_> = store.iter().map(|(id, _)| id).collect();
    builder.set_budget(2);
    assert!(!builder.advance(&mut store, &package, &LoadContext::none()));
    assert_eq!(store.len(), 6);
    assert!(existing.iter().all(|&id| store.contains(id)));
    builder.cancel(&mut store);
    assert!(builder.is_finished());
    assert_eq!(builder.remaining(), 0);
    assert_eq!(store.len(), 1);
    assert!(store.contains(survivor));
}

#[test]
fn a_removed_prepared_child_fails_transactionally_then_the_next_root_still_builds() {
    let package = FuiPackage::parse("MainMenu", include_bytes!("fixtures/MainMenu.fui")).unwrap();
    let mut store = ObjectStore::new();
    let survivor = store.create(ObjectType::Graph);
    let index = package.item_index_by_name("Main").unwrap();
    let mut builder = AsyncBuilder::new([index, index], 1);
    builder.advance(&mut store, &package, &LoadContext::none());
    let prepared = store
        .iter()
        .map(|(id, _)| id)
        .find(|&id| id != survivor)
        .unwrap();
    store.remove(prepared);
    while !builder.advance(&mut store, &package, &LoadContext::none()) {}
    assert_eq!(builder.failures(), 1);
    assert_eq!(builder.roots().len(), 1);
    assert!(store.contains(survivor));
    store.remove(builder.roots()[0]);
    assert_eq!(store.len(), 1);
}

#[test]
fn planning_rejects_cycles_and_invalid_roots_without_allocating_or_blocking_the_next_resource() {
    let package = FuiPackage::parse("Basics", include_bytes!("fixtures/PopupCycle.fui")).unwrap();
    let mut store = ObjectStore::new();
    let survivor = store.create(ObjectType::Graph);
    let cyclic = package.item_index_by_name("Demo_Popup").unwrap();
    let leaf = package
        .items()
        .iter()
        .position(|item| item.item_type == PackageItemType::Image)
        .unwrap();
    let mut builder = AsyncBuilder::new([cyclic, usize::MAX, leaf], 1);
    assert!(!builder.advance(&mut store, &package, &LoadContext::none()));
    assert_eq!(builder.failures(), 1);
    assert_eq!(store.len(), 1);
    assert!(!builder.advance(&mut store, &package, &LoadContext::none()));
    assert_eq!(builder.failures(), 2);
    assert_eq!(store.len(), 1);
    assert!(builder.advance(&mut store, &package, &LoadContext::none()));
    assert_eq!(builder.roots().len(), 1);
    assert!(store.contains(survivor));
    assert_eq!(store.len(), 2);
}

#[test]
fn cancelling_later_work_keeps_completed_roots_owned_by_the_caller() {
    let package = FuiPackage::parse("MainMenu", include_bytes!("fixtures/MainMenu.fui")).unwrap();
    let mut store = ObjectStore::new();
    let index = package.item_index_by_name("Main").unwrap();
    let mut builder = AsyncBuilder::new([index, index], 1);
    while builder.roots().is_empty() {
        builder.advance(&mut store, &package, &LoadContext::none());
    }
    let complete = store.len();
    let root = builder.roots()[0];
    builder.set_budget(3);
    builder.advance(&mut store, &package, &LoadContext::none());
    assert_eq!(store.len(), complete + 3);
    builder.cancel(&mut store);
    assert_eq!(store.len(), complete);
    assert!(store.contains(root));
    assert_eq!(builder.roots(), [root]);
    store.remove(root);
    assert!(store.is_empty());
}
