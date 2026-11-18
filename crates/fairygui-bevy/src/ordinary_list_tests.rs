use super::tests::fixture;
use super::*;
use crate::prelude::*;
use std::sync::Mutex;

type Calls = Arc<Mutex<Vec<usize>>>;

fn provider(mode: usize, calls: Calls) -> impl Fn(usize) -> Option<String> + Send + Sync {
    move |index| {
        calls.lock().unwrap().push(index);
        match mode {
            0 => Some(
                if index % 2 == 0 {
                    "chatRight"
                } else {
                    "ui://Chat/chatLeft"
                }
                .into(),
            ),
            1 => Some("chatLeft".into()),
            _ => match index % 3 {
                0 => None,
                1 => Some(String::new()),
                _ => Some("chatLeft".into()),
            },
        }
    }
}

#[test]
fn ordinary_provider_and_fifo_pool_match_native_calls_templates_visibility_and_row_identity() {
    #[derive(Component)]
    struct Marker(usize);
    let (mut app, owner, list, view) = fixture(
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
        "list",
    );
    // The exported demo includes preview rows; the native trace starts empty.
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in runtime.store().list_items(list).unwrap() {
            runtime.store_mut().remove(row);
        }
    }
    app.update();
    let providers = Calls::default();
    let renderers = Calls::default();
    let render_calls = renderers.clone();
    app.world_mut()
        .entity_mut(view)
        .insert(GuiListContent::new(0, move |index, row| {
            render_calls.lock().unwrap().push(index);
            if let Some(mut text) = row.child_by_path("msg") {
                text.set_text(&format!("Row {index}"));
            }
        }));
    let mut known = HashMap::new();
    let mut snapshots = 0;
    for line in include_str!("../tests/fixtures/ordinary-list-reference.txt").lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        if values.is_empty() || values[0].starts_with('#') {
            continue;
        }
        let value: usize = values[2].parse().unwrap();
        match values[1] {
            "count" => {
                app.world_mut()
                    .get_mut::<GuiListContent>(view)
                    .unwrap()
                    .item_count = value
            }
            "provider" => app
                .world_mut()
                .get_mut::<GuiListContent>(view)
                .unwrap()
                .set_item_provider(provider(value, providers.clone())),
            "hide" => {
                let row = app.world().get::<GuiListState>(view).unwrap().items()[value];
                app.world_mut().gui_mut(row).unwrap().set_visible(false);
            }
            _ => panic!("{line}"),
        }
        app.update();
        for (calls, expected) in [(&providers, values[3]), (&renderers, values[4])] {
            let actual = std::mem::take(&mut *calls.lock().unwrap());
            let expected: Vec<usize> = if expected == "_" {
                Vec::new()
            } else {
                expected
                    .split(',')
                    .map(|index| index.parse().unwrap())
                    .collect()
            };
            assert_eq!(actual, expected, "{line}");
        }
        let rows = app
            .world()
            .get::<GuiListState>(view)
            .unwrap()
            .items()
            .to_vec();
        assert_eq!(rows.len(), values[7].parse::<usize>().unwrap(), "{line}");
        assert_eq!(values.len(), 8 + rows.len() * 3);
        for (&row, expected) in rows.iter().zip(values[8..].as_chunks::<3>().0) {
            let next = known.len();
            let identity = *known.entry(row).or_insert_with(|| {
                app.world_mut().entity_mut(row).insert(Marker(next));
                next
            });
            assert_eq!(identity, expected[0].parse::<usize>().unwrap(), "{line}");
            assert_eq!(app.world().get::<Marker>(row).unwrap().0, identity);
            let object = app.world().get::<GObjectRef>(row).unwrap().0;
            let runtime = app.world().non_send::<GuiRuntime>();
            let native = runtime.store().object(object).unwrap();
            assert_eq!(
                native.package_item_id(),
                Some(if expected[1] == "a" {
                    "kfcb1j"
                } else {
                    "kfcb1h"
                }),
                "{line}"
            );
            assert_eq!(native.visible(), expected[2] == "1", "{line}");
            assert_eq!(native.parent(), Some(list));
        }
        assert_eq!(known.len(), values[6].parse::<usize>().unwrap(), "{line}");
        assert_eq!(
            known.len() - rows.len(),
            values[5].parse::<usize>().unwrap(),
            "{line}"
        );
        for &row in known.keys().filter(|row| !rows.contains(row)) {
            let object = app.world().get::<GObjectRef>(row).unwrap().0;
            let runtime = app.world().non_send::<GuiRuntime>();
            assert!(
                runtime.store().contains(object),
                "pooled model remains alive: {line}"
            );
            assert!(!runtime.store().is_on_stage(object));
            assert!(runtime.asset_handle_for(object).is_some());
            assert!(app.world().get::<GuiListRow>(row).is_none());
        }
        assert_eq!(
            app.world()
                .get::<GuiListContentState>(view)
                .unwrap()
                .error(),
            None
        );
        snapshots += 1;
    }
    assert_eq!(snapshots, 14);
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        known
            .keys()
            .all(|&row| app.world().get_entity(row).is_err())
    );
}

#[test]
fn ordinary_provider_retries_missing_cross_package_resources_and_can_supply_an_unexported_list() {
    let (mut app, owner, list, view) = fixture(
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
        "list",
    );
    app.world_mut()
        .entity_mut(view)
        .insert(GuiListContent::new(0, |_, _| {}));
    app.update();
    app.world_mut()
        .entity_mut(view)
        .insert(
            GuiListContent::new(2, |_, _| {}).with_item_provider(|index| {
                Some(
                    if index == 0 {
                        "chatLeft"
                    } else {
                        "ui://VirtualList/mailItem"
                    }
                    .into(),
                )
            }),
        );
    let before = app.world().non_send::<GuiRuntime>().store().len();
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(view)
            .unwrap()
            .error(),
        Some(&GuiListContentError::MissingResource(
            "ui://VirtualList/mailItem".into()
        ))
    );
    assert!(
        app.world()
            .get::<GuiListState>(view)
            .unwrap()
            .items()
            .is_empty()
    );
    assert_eq!(app.world().non_send::<GuiRuntime>().store().len(), before);
    let package = fairygui_core::FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "foreign/VirtualList".into(),
        });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(handle.clone(), &package);
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(view)
            .unwrap()
            .error(),
        None
    );
    let rows = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    assert_eq!(rows.len(), 2);
    let foreign = app.world().get::<GObjectRef>(rows[1]).unwrap().0;
    {
        let runtime = app.world().non_send::<GuiRuntime>();
        assert_eq!(
            runtime.store().object(foreign).unwrap().source_package_id(),
            Some(package.id())
        );
        assert_eq!(runtime.asset_handle_for(foreign), Some(handle.clone()));
        runtime.store().for_each_descendant(foreign, &mut |child| {
            assert_eq!(runtime.asset_handle_for(child), Some(handle.clone()))
        });
    }
    app.world_mut()
        .get_mut::<GuiListContent>(view)
        .unwrap()
        .item_count = 0;
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .asset_handle_for(foreign),
        Some(handle)
    );
    // A list without exported source/default metadata can use explicit URLs.
    let synthetic = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let synthetic = runtime
            .store_mut()
            .create(fairygui_core::package::ObjectType::List);
        runtime.store_mut().set_parent(synthetic, Some(list));
        runtime.queue_subtree(synthetic);
        synthetic
    };
    app.update();
    let synthetic_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(synthetic)
        .unwrap();
    app.world_mut().entity_mut(synthetic_view).insert(
        GuiListContent::new(1, |_, _| {}).with_item_provider(|_| Some("ui://Chat/chatLeft".into())),
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(synthetic_view)
            .unwrap()
            .error(),
        None
    );
    assert_eq!(
        app.world()
            .get::<GuiListState>(synthetic_view)
            .unwrap()
            .item_count(),
        1
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .contains(foreign)
    );
    assert!(rows.iter().all(|&row| app.world().get_entity(row).is_err()));
}

#[test]
fn ordinary_provider_panic_does_not_allocate_detached_rows_or_replace_existing_content() {
    let (mut app, _, list, view) = fixture(
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
        "list",
    );
    app.world_mut()
        .entity_mut(view)
        .insert(GuiListContent::new(1, |_, _| {}));
    app.update();
    let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
    let before = runtime.store().len();
    let rows = runtime.store().list_items(list).unwrap();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.configure_list_content(
            list,
            5,
            |index| {
                assert_ne!(index, 3, "ordinary provider failure");
                Some("chatLeft".into())
            },
            |_, _, _| panic!("renderer must not run before all resources resolve"),
        )
    }));
    assert!(panic.is_err());
    assert_eq!(runtime.store().len(), before);
    assert_eq!(runtime.store().list_items(list).unwrap(), rows);
}

#[test]
fn failed_ordinary_growth_preserves_pooled_row_order_visibility_and_owned_subtrees() {
    let (mut app, owner, list, view) = fixture(
        "Chat",
        include_bytes!("../../fairygui-demo/assets/ui/Chat.fui"),
        "list",
    );
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in runtime.store().list_items(list).unwrap() {
            runtime.store_mut().remove(row);
        }
    }
    app.world_mut()
        .entity_mut(view)
        .insert(GuiListContent::new(3, |_, _| {}).with_item_provider(|_| Some("chatLeft".into())));
    app.update();
    let original = app
        .world()
        .get::<GuiListState>(view)
        .unwrap()
        .items()
        .to_vec();
    let first = app.world().get::<GObjectRef>(original[0]).unwrap().0;
    app.world_mut()
        .gui_mut(original[0])
        .unwrap()
        .set_visible(false);
    app.world_mut()
        .get_mut::<GuiListContent>(view)
        .unwrap()
        .item_count = 0;
    app.update();
    let package = fairygui_core::FuiPackage::parse(
        "cycle",
        include_bytes!("../../fairygui-core/tests/fixtures/PopupCycle.fui"),
    )
    .unwrap();
    let bad_url = format!("ui://{}/Demo_Popup", package.name());
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "cycle".into(),
        });
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .register_package(handle, &package);
    let before = app.world().non_send::<GuiRuntime>().store().len();
    {
        let mut config = app.world_mut().get_mut::<GuiListContent>(view).unwrap();
        config.item_count = 3;
        config.set_item_provider(move |index| {
            Some(match index {
                0 => "chatLeft".into(),
                1 => "chatRight".into(),
                _ => bad_url.clone(),
            })
        });
    }
    // Resource retries must leave the same reusable rows untouched each time.
    for _ in 0..2 {
        app.update();
        assert!(matches!(
            app.world()
                .get::<GuiListContentState>(view)
                .unwrap()
                .error(),
            Some(GuiListContentError::CreationFailed(_))
        ));
        assert!(
            app.world()
                .get::<GuiListState>(view)
                .unwrap()
                .items()
                .is_empty()
        );
        let runtime = app.world().non_send::<GuiRuntime>();
        assert_eq!(
            runtime.store().len(),
            before,
            "partially constructed rows must not leak"
        );
        assert!(
            !runtime.store().object(first).unwrap().visible(),
            "failed growth must preserve a cached row's visibility"
        );
        for &row in &original {
            let object = app.world().get::<GObjectRef>(row).unwrap().0;
            assert!(runtime.store().contains(object));
            assert!(!runtime.store().is_on_stage(object));
            assert!(runtime.asset_handle_for(object).is_some());
            assert!(app.world().get::<GuiListRow>(row).is_none());
        }
    }
    app.world_mut()
        .get_mut::<GuiListContent>(view)
        .unwrap()
        .set_item_provider(|_| Some("chatLeft".into()));
    app.update();
    assert_eq!(
        app.world().get::<GuiListState>(view).unwrap().items(),
        original
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(first)
            .unwrap()
            .visible()
    );
    assert_eq!(
        app.world()
            .get::<GuiListContentState>(view)
            .unwrap()
            .error(),
        None
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        original
            .iter()
            .all(|&row| app.world().get_entity(row).is_err())
    );
}
