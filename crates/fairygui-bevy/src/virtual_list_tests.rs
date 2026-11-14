use super::*;

fn fixture() -> (App, GObjectId, GObjectId) {
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/VirtualList".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "mailList").unwrap();
    gui.enable_virtual_list(list, |store, index, row| {
        store.set_object_text(row, &format!("Row {index}"));
    })
    .unwrap();
    gui.set_virtual_item_count(list, 1000).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_virtual_lists,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();
    (app, root, list)
}

#[test]
fn virtual_thousand_rows_refresh_automatically_retain_views_and_dispose_their_pool() {
    let (mut app, root, list) = fixture();
    let (views, initial_objects) = {
        let gui = app.world().non_send::<GuiRuntime>();
        let info = gui.store().virtual_list_info(list).unwrap();
        assert!(info.active_count > 1 && info.active_count < 30, "{info:?}");
        assert_eq!(info.item_count, 1000);
        let views: Vec<_> = gui
            .store()
            .list_items(list)
            .unwrap()
            .into_iter()
            .map(|id| (id, gui.entity_of(id).unwrap()))
            .collect();
        for &(id, _) in &views {
            let index = gui.store().virtual_item_index(list, id).unwrap();
            let ControlData::Button(data) = gui.store().object(id).unwrap().control() else {
                unreachable!()
            };
            assert_eq!(data.title, format!("Row {index}"));
        }
        (views, gui.store().len())
    };
    for fraction in [0.5, 1., 0.2, 0.] {
        {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let max = gui.store().scroll_pane(list).unwrap().max_scroll_pos();
            gui.store_mut()
                .set_scroll_pos(list, 0., max.y * fraction, false);
        }
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        assert!(gui.store().len() <= initial_objects + 40);
        assert!(
            views
                .iter()
                .all(|&(id, view)| gui.entity_of(id) == Some(view))
        );
        assert!(gui.store().virtual_list_info(list).unwrap().active_count < 30);
    }
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        assert!(
            gui.store_mut()
                .add_list_selection_with_scroll(list, 500, true)
        );
    }
    app.update();
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let row = gui.store().virtual_item_object(list, 500).unwrap();
        assert_eq!(gui.store().list_item_of(row).unwrap().2, 500);
        assert_eq!(gui.store().button_selected(row), Some(true));
        gui.set_virtual_item_count(list, 0).unwrap();
        assert_eq!(gui.store().list_item_count(list), 0);
        assert!(gui.store().virtual_list_info(list).unwrap().pooled_count > 0);
    }
    app.update();
    for &(_, view) in &views {
        assert_eq!(
            app.world().get::<Visibility>(view),
            Some(&Visibility::Hidden)
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(
        views
            .iter()
            .all(|&(_, view)| app.world().get_entity(view).is_err())
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .virtual_renderers
            .is_empty()
    );
}

#[test]
fn ecs_properties_drive_virtual_list_selection_and_scroll() {
    let (mut app, _root, list) = fixture();
    let list_entity = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(list)
        .unwrap();
    app.add_systems(Update, crate::properties::sync);

    let max_scroll = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(list)
        .unwrap()
        .max_scroll_pos();
    if let Some(mut object) = app.world_mut().gui_mut(list_entity) {
        object.select_list_index(500, true)
    }
    if let Some(mut object) = app.world_mut().gui_mut(list_entity) {
        object.set_scroll_position(Vec2::new(max_scroll.x, max_scroll.y), false)
    }
    app.world_mut()
        .entity_mut(list_entity)
        .insert(GuiListSelection(vec![123]));

    app.update();
    {
        let runtime = app.world().non_send::<GuiRuntime>();
        assert_eq!(runtime.store().list_selections(list), Some(vec![123]));
        let position = runtime.store().scroll_pane(list).unwrap().pos();
        assert_eq!(position, max_scroll);
    }
    assert_eq!(
        app.world().get::<GuiListSelection>(list_entity),
        Some(&GuiListSelection(vec![123]))
    );

    app.world_mut()
        .get_mut::<GuiListSelection>(list_entity)
        .unwrap()
        .0
        .clear();
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_selections(list),
        Some(vec![])
    );
    assert_eq!(
        app.world().get::<GuiListSelection>(list_entity),
        Some(&GuiListSelection(vec![]))
    );
}

#[test]
fn ecs_commands_set_slider_value_by_entity() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    let root = runtime
        .create_root(Handle::default(), &package, "Demo_Slider")
        .unwrap();
    let _root_object = runtime.root_object(root).unwrap();
    let slider = runtime
        .store()
        .iter()
        .find_map(|(id, object)| matches!(object.control(), ControlData::Slider(_)).then_some(id))
        .unwrap();
    let max = match runtime.store().object(slider).unwrap().control() {
        ControlData::Slider(data) => data.max,
        _ => unreachable!(),
    };
    let initial_value = runtime.store().slider_value(slider).unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(runtime)
        .add_systems(Update, crate::properties::sync);
    let target = app
        .world_mut()
        .spawn((GObjectRef(slider), GuiSliderValue(initial_value)))
        .id();
    app.world_mut().get_mut::<GuiSliderValue>(target).unwrap().0 = max;

    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .slider_value(slider),
        Some(max)
    );
    assert_eq!(
        app.world().get::<GuiSliderValue>(target),
        Some(&GuiSliderValue(max))
    );
}

#[test]
fn ecs_commands_project_button_selection_by_entity() {
    let package = FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    runtime
        .create_root(Handle::default(), &package, "Demo_Button")
        .unwrap();
    let button = runtime
        .store()
        .iter()
        .find_map(|(id, object)| {
            matches!(
                object.control(),
                ControlData::Button(data)
                    if data.mode != fairygui_core::widget::ButtonMode::Common
            )
            .then_some(id)
        })
        .unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(runtime)
        .add_systems(Update, crate::properties::sync);
    let target = app
        .world_mut()
        .spawn((GObjectRef(button), GuiButtonSelected(false)))
        .id();
    app.world_mut()
        .get_mut::<GuiButtonSelected>(target)
        .unwrap()
        .0 = true;

    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .button_selected(button),
        Some(true)
    );
    assert_eq!(
        app.world().get::<GuiButtonSelected>(target),
        Some(&GuiButtonSelected(true))
    );
}

#[test]
fn ecs_commands_set_progress_by_entity() {
    let package = FuiPackage::parse(
        "Cooldown",
        include_bytes!("../../fairygui-demo/assets/ui/Cooldown.fui"),
    )
    .unwrap();
    let mut runtime = GuiRuntime::default();
    let root = runtime
        .create_root(Handle::default(), &package, "Main")
        .unwrap();
    let root = runtime.root_object(root).unwrap();
    let progress = runtime.store().find_by_name(root, "b0").unwrap();
    let initial_value = runtime.store().progress_value(progress).unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(runtime)
        .add_systems(Update, crate::properties::sync);
    let target = app
        .world_mut()
        .spawn((GObjectRef(progress), GuiProgressValue(initial_value)))
        .id();
    app.world_mut()
        .get_mut::<GuiProgressValue>(target)
        .unwrap()
        .0 = 37.;

    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .progress_value(progress),
        Some(37.)
    );
    assert_eq!(
        app.world().get::<GuiProgressValue>(target),
        Some(&GuiProgressValue(37.))
    );
}

#[test]
fn gui_config_is_captured_by_ecs_roots_without_rebuilding_existing_panes() {
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_resource::<GuiConfig>()
        .insert_non_send(GuiRuntime::default())
        .add_systems(Update, (sync_ui_config, sync_ecs_roots).chain());
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/VirtualList".into(),
        });
    let first = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    app.world_mut().resource_mut::<GuiConfig>().scroll = fairygui_core::scroll::ScrollDefaults {
        bar_display: fairygui_core::scroll::ScrollBarDisplayType::Auto,
        touch_effect: false,
        bounceback_effect: false,
        scroll_step: 14.0,
        paging_threshold: 0.4,
    };

    app.update();
    let first_root = app.world().get::<GuiRootInstance>(first).unwrap().object;
    let first_list = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .child_by_name(first_root, "mailList")
        .unwrap();
    {
        let runtime = app.world().non_send::<GuiRuntime>();
        let pane = runtime.store().scroll_pane(first_list).unwrap();
        assert_eq!(pane.scroll_step(), 14.0);
        assert_eq!(pane.paging_threshold(), 0.4);
    }

    app.world_mut()
        .resource_mut::<GuiConfig>()
        .scroll
        .scroll_step = 21.0;
    let second = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let second_root = app.world().get::<GuiRootInstance>(second).unwrap().object;
    let runtime = app.world().non_send::<GuiRuntime>();
    let second_list = runtime
        .store()
        .child_by_name(second_root, "mailList")
        .unwrap();
    assert_eq!(
        runtime
            .store()
            .scroll_pane(second_list)
            .unwrap()
            .scroll_step(),
        21.0
    );
    assert_eq!(
        runtime
            .store()
            .scroll_pane(first_list)
            .unwrap()
            .scroll_step(),
        14.0
    );
    assert_eq!(runtime.ecs_root_object(first), Some(first_root));
}

#[test]
fn renderer_mutations_are_reconciled_before_bevy_row_views_are_spawned() {
    use std::cell::Cell;

    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/VirtualList".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "mailList").unwrap();
    let changed = Cell::new(false);
    gui.enable_virtual_list(list, move |store, index, row| {
        store.set_object_text(row, &format!("Row {index}"));
        if !changed.replace(true) {
            store.set_virtual_item_count(list, 2).unwrap();
            store.set_size(list, 400., 240.);
        }
    })
    .unwrap();
    gui.set_virtual_item_count(list, 1000).unwrap();
    assert_eq!(gui.store().list_num_items(list), 2);
    assert_eq!(gui.store().list_item_count(list), 2);
    assert_eq!(gui.store().virtual_list_info(list).unwrap().active_count, 2);

    app.insert_non_send(gui).add_systems(
        Update,
        (
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();

    let gui = app.world().non_send::<GuiRuntime>();
    let rows = gui.store().list_items(list).unwrap();
    assert_eq!(rows.len(), 2);
    for (index, row) in rows.into_iter().enumerate() {
        let entity = gui
            .entity_of(row)
            .expect("visible virtual row has an entity");
        assert!(app.world().get::<Node>(entity).is_some());
        assert_eq!(
            app.world()
                .get::<GObjectRef>(entity)
                .map(|reference| reference.0),
            Some(row)
        );
        let ControlData::Button(button) = gui.store().object(row).unwrap().control() else {
            unreachable!();
        };
        assert_eq!(button.title, format!("Row {index}"));
    }
}

#[test]
fn ecs_root_component_waits_for_asset_and_owns_view_lifetime() {
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let asset = Handle::<FuiAsset>::default();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "Main"))
        .id();
    app.insert_non_send(GuiRuntime::default()).add_systems(
        Update,
        (sync_ecs_roots, spawn_pending_roots, spawn_pending_objects).chain(),
    );

    app.update();
    assert!(app.world().get::<GuiRootInstance>(owner).is_none());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .ecs_root_object(owner)
            .is_none()
    );
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/VirtualList".into(),
            },
        )
        .unwrap();

    app.update();
    let instance = *app.world().get::<GuiRootInstance>(owner).unwrap();
    let view = {
        let gui = app.world().non_send::<GuiRuntime>();
        assert_eq!(gui.ecs_root_handle(owner), Some(instance.handle));
        assert_eq!(gui.ecs_root_object(owner), Some(instance.object));
        assert!(gui.store().is_on_stage(instance.object));
        gui.entity_of(instance.object).unwrap()
    };
    assert!(app.world().get::<Node>(view).is_some());

    app.update();
    assert_eq!(app.world().non_send::<GuiRuntime>().root_count(), 1);

    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = false;
    app.update();
    {
        let gui = app.world().non_send::<GuiRuntime>();
        assert!(!gui.store().is_on_stage(instance.object));
        assert_eq!(gui.entity_of(instance.object), Some(view));
    }

    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .active = true;
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .is_on_stage(instance.object)
    );

    app.world_mut().entity_mut(owner).remove::<FairyGuiRoot>();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(gui.store().object(instance.object).is_none());
    assert!(gui.entity_of(instance.object).is_none());
    assert!(app.world().get::<GuiRootInstance>(owner).is_none());

    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let instance = *app.world().get::<GuiRootInstance>(owner).unwrap();
    app.world_mut().despawn(owner);
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(gui.store().object(instance.object).is_none());
    assert!(gui.entity_of(instance.object).is_none());
}

#[test]
fn ecs_object_commands_mutate_the_model_by_entity() {
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let asset = Handle::<FuiAsset>::default();
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package,
                res_key: "ui/VirtualList".into(),
            },
        )
        .unwrap();
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.insert_non_send(GuiRuntime::default()).add_systems(
        Update,
        (
            sync_ecs_roots,
            crate::properties::sync,
            spawn_pending_roots,
            spawn_pending_objects,
        )
            .chain(),
    );
    app.update();
    let instance = *app.world().get::<GuiRootInstance>(owner).unwrap();
    let text = app
        .world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(fairygui_core::package::ObjectType::Text);
    let text_entity = app.world_mut().spawn(GObjectRef(text)).id();
    if let Some(mut object) = app.world_mut().gui_mut(owner) {
        object.set_position(Vec2::new(37., 42.))
    }
    if let Some(mut object) = app.world_mut().gui_mut(owner) {
        object.set_size(Vec2::new(640., 360.))
    }
    if let Some(mut object) = app.world_mut().gui_mut(text_entity) {
        object.set_text("ECS message")
    }
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let object = gui.store().object(instance.object).unwrap();
    assert_eq!((object.x(), object.y()), (37., 42.));
    assert_eq!(object.size(), fairygui_core::geom::Size::new(640., 360.));
    assert_eq!(
        gui.store()
            .object(text)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "ECS message"
    );
}

#[test]
fn exported_loop_list_retains_views_and_native_record_mapping_through_wraps_and_pooling() {
    let package = FuiPackage::parse(
        "LoopList",
        include_bytes!("../../fairygui-demo/assets/ui/LoopList.fui"),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
        .init_asset::<FuiAsset>()
        .init_asset::<Image>()
        .init_asset::<Font>()
        .init_resource::<GuiFont>();
    let handle = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/LoopList".into(),
        });
    let mut gui = GuiRuntime::default();
    let index = gui.create_root(handle, &package, "Main").unwrap();
    let root = gui.root_object(index).unwrap();
    let list = gui.store().child_by_name(root, "list").unwrap();
    let icons: Vec<_> = (1..=5)
        .map(|i| {
            gui.registry()
                .item_url("LoopList", &format!("n{i}"))
                .unwrap()
        })
        .collect();
    let renderer_icons = icons.clone();
    gui.enable_loop_virtual_list(list, move |store, index, row| {
        store.set_pivot(row, 0.5, 0.5, false);
        store.apply_icon_to(row, &renderer_icons[index]);
    })
    .unwrap();
    gui.set_virtual_item_count(list, 5).unwrap();
    app.insert_non_send(gui).add_systems(
        Update,
        (
            sync_virtual_lists,
            spawn_pending_roots,
            spawn_pending_objects,
            sync_dirty,
            sync_stage_hierarchy,
        )
            .chain(),
    );
    app.update();
    let mut views = HashMap::new();
    for x in [0., 1e6, 5., 0., 350., 1e6] {
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .set_scroll_pos_animated(list, x, 0., false);
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let info = gui.store().virtual_list_info(list).unwrap();
        assert!(info.looping);
        assert_eq!((info.item_count, info.real_item_count), (5, 30));
        assert!(info.active_count > 1 && info.active_count < 20);
        assert!(!gui.store().scroll_pane(list).unwrap().bounceback_effect);
        for row in gui.store().list_items(list).unwrap() {
            let index = gui.store().virtual_item_index(list, row).unwrap();
            let record = gui.store().virtual_item_record_index(list, row).unwrap();
            assert_eq!(index, record % 5);
            let ControlData::Button(button) = gui.store().object(row).unwrap().control() else {
                unreachable!()
            };
            assert_eq!(button.icon.as_ref(), Some(&icons[index]));
            let entity = gui.entity_of(row).unwrap();
            if let Some(old) = views.insert(row, entity) {
                assert_eq!(old, entity);
            }
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Inherited)
            );
        }
        let ControlData::List(data) = gui.store().object(list).unwrap().control() else {
            unreachable!()
        };
        if data.component.children_render_order == crate::core::widget::ChildrenRenderOrder::Arch {
            let apex = gui
                .store()
                .list_item_at(list, data.component.apex_index as usize)
                .unwrap();
            let z = app
                .world()
                .get::<ZIndex>(gui.entity_of(apex).unwrap())
                .unwrap()
                .0;
            assert!(gui.store().list_items(list).unwrap().iter().all(|&r| {
                app.world()
                    .get::<ZIndex>(gui.entity_of(r).unwrap())
                    .unwrap()
                    .0
                    <= z
            }));
        }
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_virtual_item_count(list, 0)
        .unwrap();
    app.update();
    for &entity in views.values() {
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_virtual_item_count(list, 5)
        .unwrap();
    app.update();
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .list_item_count(list)
            > 0
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(root);
    app.update();
    assert!(
        views
            .values()
            .all(|&view| app.world().get_entity(view).is_err())
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .virtual_renderers
            .is_empty()
    );
}
