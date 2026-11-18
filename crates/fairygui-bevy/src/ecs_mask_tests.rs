use super::*;
use crate::prelude::*;
use bevy::ecs::system::RunSystemOnce;

fn fixture() -> (App, Handle<FuiAsset>, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "Basics",
                include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
            )
            .unwrap(),
            res_key: "ui/Basics".into(),
        });
    let mut spawn = |name| {
        app.world_mut()
            .spawn(FairyGuiRoot::new(asset.clone(), name))
            .id()
    };
    let owner = spawn("Component12");
    let a = spawn("Button1");
    let b = spawn("Button2");
    app.update();
    (app, asset, owner, a, b)
}

fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn id(app: &App, owner: Entity) -> GObjectId {
    app.world().get::<GObjectRef>(view(app, owner)).unwrap().0
}
fn mask(app: &App, owner: Entity) -> Option<(GObjectId, bool)> {
    app.world()
        .non_send::<GuiRuntime>()
        .store()
        .component_mask(id(app, owner))
}

#[test]
fn declarations_apply_reverse_clear_and_publish_actual_entity_snapshots() {
    let (mut app, _, owner, a, _) = fixture();
    app.world_mut().entity_mut(owner).insert(GuiMask::new(a));
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    assert_eq!(
        app.world()
            .get::<GuiMaskState>(view(&app, owner))
            .unwrap()
            .source,
        Some(view(&app, a))
    );
    assert!(app.world().get::<GuiMask>(view(&app, owner)).is_none());
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiMask::reversed(a));
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), true)));
    assert!(
        app.world()
            .get::<GuiMaskState>(view(&app, owner))
            .unwrap()
            .reversed
    );
    app.world_mut().entity_mut(owner).insert(GuiMask::default());
    app.update();
    assert_eq!(mask(&app, owner), None);
    assert_eq!(
        *app.world().get::<GuiMaskState>(view(&app, owner)).unwrap(),
        GuiMaskState::default()
    );
}

#[test]
fn unchanged_declarations_preserve_immediate_edits_without_replaying_native_side_effects() {
    let (mut app, _, owner, a, b) = fixture();
    app.world_mut().entity_mut(owner).insert(GuiMask::new(a));
    app.update();
    let source = id(&app, a);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_cache_as_bitmap(source, false);
    app.update();
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(source)
            .unwrap()
            .cache_as_bitmap()
    );
    app.world_mut()
        .run_system_once(move |mut gui: GuiMut| assert!(gui.set_mask(owner, Some(b), true)))
        .unwrap();
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, b), true)));
    app.world_mut().entity_mut(owner).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), None);
}

#[test]
fn view_overrides_owner_and_copied_references_cannot_declare_masks() {
    let (mut app, _, owner, a, b) = fixture();
    let owner_view = view(&app, owner);
    let owner_id = id(&app, owner);
    let copied = app
        .world_mut()
        .spawn((GObjectRef(owner_id), GuiMask::new(b)))
        .id();
    app.world_mut().entity_mut(owner).insert(GuiMask::new(a));
    app.world_mut()
        .entity_mut(owner_view)
        .insert(GuiMask::reversed(b));
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, b), true)));
    app.world_mut().entity_mut(owner_view).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    app.world_mut().entity_mut(owner).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), None);
    app.world_mut().despawn(copied);
}

#[test]
fn pending_sources_retain_previous_mask_and_apply_when_loaded() {
    let (mut app, asset, owner, a, _) = fixture();
    assert!(app.world_mut().gui_set_mask(owner, Some(a), false));
    let pending = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let target = app
        .world_mut()
        .spawn(FairyGuiRoot::new(pending.clone(), "Button2"))
        .id();
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiMask::reversed(target));
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    // A request which never applied must not clear an existing native mask.
    app.world_mut().entity_mut(owner).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiMask::reversed(target));
    let loaded = {
        let assets = app.world().resource::<Assets<FuiAsset>>();
        let source = assets.get(&asset).unwrap();
        FuiAsset {
            package: source.package.clone(),
            res_key: source.res_key.clone(),
        }
    };
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(pending.id(), loaded)
        .unwrap();
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, target), true)));
}

#[test]
fn source_and_owner_template_replacements_rebind_to_current_objects() {
    let (mut app, _, owner, a, _) = fixture();
    app.world_mut().entity_mut(owner).insert(GuiMask::new(a));
    app.update();
    let old = id(&app, a);
    app.world_mut()
        .get_mut::<FairyGuiRoot>(a)
        .unwrap()
        .item_name = "Button2".into();
    assert!(!app.world_mut().gui_set_mask(owner, Some(a), true));
    app.update();
    assert_ne!(id(&app, a), old);
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    app.world_mut()
        .get_mut::<FairyGuiRoot>(owner)
        .unwrap()
        .item_name = "Button1".into();
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
}

#[test]
fn removing_a_pending_replacement_still_releases_its_previous_managed_mask() {
    let (mut app, _, owner, a, _) = fixture();
    app.world_mut().entity_mut(owner).insert(GuiMask::new(a));
    app.update();
    app.world_mut().get_mut::<GuiMask>(owner).unwrap().source = Some(Entity::PLACEHOLDER);
    app.update();
    assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    app.world_mut().entity_mut(owner).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), None);
}

#[test]
fn invalid_or_deleted_sources_do_not_rebind_to_other_objects() {
    let (mut app, _, owner, a, b) = fixture();
    assert!(app.world_mut().gui_set_mask(owner, Some(a), false));
    for source in [owner, Entity::PLACEHOLDER] {
        assert!(!app.world_mut().gui_set_mask(owner, Some(source), true));
        app.world_mut()
            .entity_mut(owner)
            .insert(GuiMask::new(source));
        app.update();
        assert_eq!(mask(&app, owner), Some((id(&app, a), false)));
    }
    let source_view = view(&app, a);
    let source_id = id(&app, a);
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiMask::new(source_view));
    app.update();
    app.world_mut().despawn(a);
    app.update();
    assert_eq!(mask(&app, owner), Some((source_id, false)));
    let state = app.world().get::<GuiMaskState>(view(&app, owner)).unwrap();
    assert!(state.assigned);
    assert_eq!(state.source, None);
    // Recreate the same template; the old view entity is not an owner alias.
    let root = app.world().get::<FairyGuiRoot>(b).unwrap().clone();
    app.world_mut().spawn(root);
    app.update();
    assert_eq!(mask(&app, owner), Some((source_id, false)));
    app.world_mut().entity_mut(owner).remove::<GuiMask>();
    app.update();
    assert_eq!(mask(&app, owner), None);
}

#[test]
fn window_aliases_and_query_use_actual_materialized_views() {
    let (mut app, asset, owner, _, _) = fixture();
    let source = app
        .world_mut()
        .spawn(FairyGuiWindow::new(asset, "WindowA"))
        .id();
    app.update();
    assert!(app.world_mut().gui_set_mask(owner, Some(source), true));
    app.update();
    let source_view = view(&app, source);
    app.world_mut()
        .run_system_once(move |gui: GuiQuery| {
            assert_eq!(gui.mask(owner).unwrap().source, Some(source_view));
            assert!(gui.mask(owner).unwrap().reversed);
        })
        .unwrap();
    app.world_mut()
        .get_mut::<FairyGuiWindow>(source)
        .unwrap()
        .item_name = "WindowB".into();
    assert!(!app.world_mut().gui_set_mask(owner, Some(source), false));
}

#[test]
fn exported_masks_are_projected_without_becoming_managed_declarations() {
    let (mut app, _, _, _, _) = fixture();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "Masks",
                include_bytes!("../../fairygui-core/tests/fixtures/Masks.fui"),
            )
            .unwrap(),
            res_key: "ui/Masks".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "Demo_Graph"))
        .id();
    app.update();
    let (source, reversed) = mask(&app, owner).unwrap();
    assert!(!reversed);
    let source_view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(source)
        .unwrap();
    assert_eq!(
        app.world()
            .get::<GuiMaskState>(view(&app, owner))
            .unwrap()
            .source,
        Some(source_view)
    );
    assert!(app.world().get::<GuiMask>(view(&app, owner)).is_none());
    app.update();
    assert_eq!(mask(&app, owner), Some((source, false)));
}
