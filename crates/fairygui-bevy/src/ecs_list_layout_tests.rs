use crate::prelude::*;
use bevy::prelude::*;
use fairygui_core::{object::ListLayoutSettings, package::ObjectType};

#[test]
fn layout_setter_recomputes_rows_bounds_and_rejects_invalid_counts() {
    let mut runtime = GuiRuntime::default();
    let list = runtime.store_mut().create(ObjectType::List);
    runtime.store_mut().set_size(list, 100., 80.);
    for height in [20., 30., 10.] {
        let row = runtime.store_mut().create(ObjectType::Component);
        runtime.store_mut().set_size(row, 40., height);
        runtime.store_mut().set_parent(row, Some(list));
    }
    let before = runtime.store().list_layout_settings(list).unwrap();
    let settings = ListLayoutSettings {
        layout: ListLayoutType::SingleColumn,
        auto_resize_item: true,
        line_gap: 5,
        ..before
    };
    assert!(runtime.store_mut().set_list_layout_settings(list, settings));
    let rows = runtime.store().object(list).unwrap().children();
    assert_eq!(runtime.store().object(rows[0]).unwrap().width(), 100.);
    assert_eq!(runtime.store().object(rows[1]).unwrap().y(), 25.);
    assert_eq!(runtime.store().object(rows[2]).unwrap().y(), 60.);
    assert!(!runtime.store_mut().set_list_layout_settings(
        list,
        ListLayoutSettings {
            column_count: -1,
            ..settings
        }
    ));
    assert_eq!(runtime.store().list_layout_settings(list), Some(settings));
}

#[test]
fn ecs_component_merges_concurrent_layout_field_changes_without_losing_native_fields() {
    use crate::properties::sync_property;
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let id = world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .create(ObjectType::List);
    let entity = world.spawn((GObjectRef(id), GuiListLayout::default())).id();
    sync_property::<GuiListLayout>(&mut world);
    world.get_mut::<GuiListLayout>(entity).unwrap().line_gap = 9;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_layout_settings(
            id,
            ListLayoutSettings {
                column_gap: 13,
                ..Default::default()
            },
        );
    sync_property::<GuiListLayout>(&mut world);
    let settings = world.get::<GuiListLayout>(entity).unwrap();
    assert_eq!(settings.line_gap, 9);
    assert_eq!(settings.column_gap, 13);
}
