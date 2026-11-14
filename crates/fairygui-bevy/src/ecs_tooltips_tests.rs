use crate::{GuiRuntime, GuiTooltips, core::package::ObjectType, properties::sync_property};
use bevy::prelude::*;

#[test]
fn tooltip_edits_clear_and_merge_native_changes_without_replaying_stale_text() {
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let object = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        let object = runtime.store_mut().create(ObjectType::Button);
        runtime
            .store_mut()
            .set_tooltips(object, Some("Exported".into()));
        object
    };
    let entity = world.spawn(crate::GObjectRef(object)).id();
    sync_property::<GuiTooltips>(&mut world);
    assert_eq!(
        world.get::<GuiTooltips>(entity).unwrap().0.as_deref(),
        Some("Exported")
    );

    world.get_mut::<GuiTooltips>(entity).unwrap().0 = Some("Application".into());
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_tooltips(object, Some("Native".into()));
    sync_property::<GuiTooltips>(&mut world);
    assert_eq!(
        world.get::<GuiTooltips>(entity).unwrap().0.as_deref(),
        Some("Application")
    );
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(object)
            .unwrap()
            .tooltips(),
        Some("Application")
    );

    world.get_mut::<GuiTooltips>(entity).unwrap().0 = None;
    sync_property::<GuiTooltips>(&mut world);
    assert_eq!(world.get::<GuiTooltips>(entity).unwrap().0, None);
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object(object)
            .unwrap()
            .tooltips(),
        None
    );

    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_tooltips(object, Some("Changed natively".into()));
    sync_property::<GuiTooltips>(&mut world);
    assert_eq!(
        world.get::<GuiTooltips>(entity).unwrap().0.as_deref(),
        Some("Changed natively")
    );
}
