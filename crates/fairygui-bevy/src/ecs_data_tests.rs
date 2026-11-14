use crate::{GuiData, GuiRuntime, core::package::ObjectType, properties::sync_property};
use bevy::prelude::*;

#[test]
fn data_property_reads_native_values_merges_external_updates_and_clears() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let object = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        let object = runtime.store_mut().create(ObjectType::Graph);
        runtime.store_mut().set_data(object, Some("Package".into()));
        object
    };
    let entity = world.spawn(crate::GObjectRef(object)).id();
    sync_property::<GuiData>(&mut world);
    assert_eq!(
        world.get::<GuiData>(entity).unwrap().0.as_deref(),
        Some("Package")
    );

    world.get_mut::<GuiData>(entity).unwrap().0 = Some("App-owned".into());
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_data(object, Some("Native".into()));
    sync_property::<GuiData>(&mut world);
    assert_eq!(
        world.get::<GuiData>(entity).unwrap().0.as_deref(),
        Some("App-owned")
    );
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(object)
            .unwrap()
            .data(),
        Some("App-owned")
    );

    world.get_mut::<GuiData>(entity).unwrap().0 = None;
    sync_property::<GuiData>(&mut world);
    assert_eq!(world.get::<GuiData>(entity).unwrap().0, None);
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(object)
            .unwrap()
            .data(),
        None
    );

    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_data(object, Some("Native update".into()));
    sync_property::<GuiData>(&mut world);
    assert_eq!(
        world.get::<GuiData>(entity).unwrap().0.as_deref(),
        Some("Native update")
    );
}
