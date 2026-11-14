use crate::{prelude::*, properties::sync_property};
use bevy::prelude::*;
use fairygui_core::package::ObjectType;

#[test]
fn interaction_fields_merge_native_updates_and_folding_reflows_same_frame() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let (list, last) = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        let list = runtime.store_mut().create(ObjectType::List);
        runtime.store_mut().set_size(list, 100., 80.);
        let first = runtime.store_mut().create(ObjectType::Component);
        let hidden = runtime.store_mut().create(ObjectType::Component);
        let last = runtime.store_mut().create(ObjectType::Component);
        for row in [first, hidden, last] {
            runtime.store_mut().set_size(row, 30., 20.);
            runtime.store_mut().set_parent(row, Some(list));
        }
        runtime.store_mut().set_visible(hidden, false);
        runtime.store_mut().layout_list(list);
        (list, last)
    };
    let entity = world
        .spawn((GObjectRef(list), GuiListSettings::default()))
        .id();
    sync_property::<GuiListSettings>(&mut world);
    world
        .get_mut::<GuiListSettings>(entity)
        .unwrap()
        .fold_invisible_items = true;
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_list_scroll_item_to_view_on_click(list, false);
    sync_property::<GuiListSettings>(&mut world);
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(last)
            .unwrap()
            .y(),
        20.
    );
    assert_eq!(
        world.get::<GuiListSettings>(entity),
        Some(&GuiListSettings {
            scroll_item_to_view_on_click: false,
            fold_invisible_items: true,
        })
    );
}
