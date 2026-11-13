use crate::prelude::*;
use bevy::prelude::*;

#[test]
fn property_queries_follow_new_archetypes_rebound_objects_and_entity_generations() {
    use crate::{
        core::package::ObjectType,
        properties::{NativeValue, sync_property},
    };
    #[derive(Component)]
    struct ApplicationChild;
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let (first, second, image) = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        let first = runtime.store_mut().create(ObjectType::Text);
        let second = runtime.store_mut().create(ObjectType::Text);
        let image = runtime.store_mut().create(ObjectType::Image);
        runtime.store_mut().set_object_text(first, "First");
        runtime.store_mut().set_object_text(second, "Second");
        (first, second, image)
    };
    // Warm the cache before any matching archetype exists.
    sync_property::<GuiText>(&mut world);
    let entity = world.spawn(GObjectRef(first)).id();
    sync_property::<GuiText>(&mut world);
    assert_eq!(world.get::<GuiText>(entity).unwrap().value(), "First");
    world.entity_mut(entity).insert(ApplicationChild);
    world.get_mut::<GuiText>(entity).unwrap().set("Edited");
    let later = world.spawn((GObjectRef(second), ApplicationChild)).id();
    sync_property::<GuiText>(&mut world);
    assert_eq!(
        world.non_send::<GuiRuntime>().store().object_text(first),
        Some("Edited")
    );
    assert_eq!(world.get::<GuiText>(later).unwrap().value(), "Second");
    // Rebinding discards the old object's baseline and writes only explicit
    // application fields; unsupported objects reclaim both projected values.
    world.entity_mut(entity).insert(GObjectRef(second));
    sync_property::<GuiText>(&mut world);
    assert_eq!(
        world.non_send::<GuiRuntime>().store().object_text(second),
        Some("Edited")
    );
    world.entity_mut(entity).insert(GObjectRef(image));
    sync_property::<GuiText>(&mut world);
    assert!(world.get::<GuiText>(entity).is_none());
    assert!(world.get::<NativeValue<GuiText>>(entity).is_none());
    world.despawn(entity);
    let replacement = world.spawn(GObjectRef(first)).id();
    world
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_object_text(first, "Reused");
    sync_property::<GuiText>(&mut world);
    assert_eq!(world.get::<GuiText>(replacement).unwrap().value(), "Reused");
    assert!(world.get_entity(entity).is_err());
}

#[test]
fn insert_observers_can_edit_or_dispose_later_candidates_in_the_same_pass() {
    use crate::{core::package::ObjectType, properties::sync_property};
    let mut world = World::new();
    world.init_non_send::<GuiRuntime>();
    let objects = {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        ["Source", "Target", "Disposed"].map(|text| {
            let object = runtime.store_mut().create(ObjectType::Text);
            runtime.store_mut().set_object_text(object, text);
            object
        })
    };
    let [source, target, disposed] = objects.map(|object| world.spawn(GObjectRef(object)).id());
    sync_property::<GuiText>(&mut world);
    world.entity_mut(source).observe(
        move |event: On<Insert<GuiText>>,
              mut texts: Query<&mut GuiText>,
              mut commands: Commands| {
            if event.entity == source {
                texts.get_mut(target).unwrap().set("From observer");
                commands.entity(disposed).despawn();
            }
        },
    );
    world.flush();
    {
        let mut runtime = world.non_send_mut::<GuiRuntime>();
        runtime
            .store_mut()
            .set_object_text(objects[0], "Refresh source");
        runtime
            .store_mut()
            .set_object_text(objects[2], "Refresh disposed");
    }
    sync_property::<GuiText>(&mut world);
    assert_eq!(
        world
            .non_send::<GuiRuntime>()
            .store()
            .object_text(objects[1]),
        Some("From observer")
    );
    assert_eq!(
        world.get::<GuiText>(target).unwrap().value(),
        "From observer"
    );
    assert!(world.get_entity(disposed).is_err());
}

fn fixture(item: &str, loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ));
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), item))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}
fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}

#[test]
fn delayed_owner_properties_are_applied_to_first_render_and_can_be_edited_on_the_view() {
    let (mut app, owner, asset) = fixture("Button1", false);
    assert!(app.world_mut().gui_mut(owner).is_none());
    app.world_mut().entity_mut(owner).insert((
        GuiText::new("Ready"),
        GuiDisplay::hidden(),
        GuiTransform {
            position: Vec2::new(35., 45.),
            size: Vec2::new(160., 40.),
            ..default()
        },
    ));
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    load(&mut app, &asset);
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(app.world().get::<GuiText>(view).unwrap().value(), "Ready");
    assert_eq!(
        app.world().get::<Visibility>(view),
        Some(&Visibility::Hidden)
    );
    let node = app.world().get::<Node>(view).unwrap();
    assert_eq!(
        (node.left, node.top, node.width, node.height),
        (Val::Px(35.), Val::Px(45.), Val::Px(160.), Val::Px(40.))
    );
    app.world_mut()
        .get_mut::<GuiText>(view)
        .unwrap()
        .set("Edited");
    app.world_mut().get_mut::<GuiDisplay>(view).unwrap().visible = true;
    app.update();
    assert_eq!(app.world().get::<GuiText>(owner).unwrap().value(), "Edited");
    assert!(app.world().get::<GuiDisplay>(owner).unwrap().visible());
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiText>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiText>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn geometry_writes_merge_with_native_changes_and_do_not_replay_during_tweens() {
    let (mut app, owner, _) = fixture("Button1", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .set_position(Vec2::new(20., 30.));
    app.world_mut()
        .get_mut::<GuiTransform>(view)
        .unwrap()
        .rotation = 45.;
    app.update();
    let transform = app.world().get::<GuiTransform>(view).unwrap();
    assert_eq!(transform.position, Vec2::new(20., 30.));
    assert_eq!(transform.rotation, 45.);
    app.world_mut().gui_mut(owner).unwrap().tween(
        GuiTweenTarget::Position(Vec2::new(120., 30.)),
        1.,
        0,
    );
    app.update();
    let first = app.world().get::<GuiTransform>(view).unwrap().position.x;
    app.world_mut()
        .get_mut::<GuiTransform>(view)
        .unwrap()
        .rotation = 90.;
    app.update();
    let next = app.world().get::<GuiTransform>(view).unwrap();
    assert!(next.position.x > first && next.position.x < 120.);
    assert_eq!(next.rotation, 90.);
}

#[test]
fn scalar_and_full_slider_properties_do_not_undo_each_other() {
    let (mut app, owner, _) = fixture("Slider_HZ", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    {
        let mut state = app.world_mut().get_mut::<GuiSliderState>(view).unwrap();
        state.min = -10.;
        state.max = 10.;
        state.value = -2.;
    }
    app.update();
    assert_eq!(app.world().get::<GuiSliderValue>(view).unwrap().0, -2.);
    app.world_mut().get_mut::<GuiSliderValue>(view).unwrap().0 = 3.25;
    app.update();
    assert_eq!(app.world().get::<GuiSliderState>(view).unwrap().value, 3.25);
    app.world_mut().gui_mut(view).unwrap().set_slider_value(7.5);
    app.world_mut()
        .get_mut::<GuiSliderState>(view)
        .unwrap()
        .title_type = 2;
    app.update();
    assert_eq!(app.world().get::<GuiSliderValue>(view).unwrap().0, 7.5);
    assert_eq!(app.world().get::<GuiSliderState>(view).unwrap().value, 7.5);
}

#[test]
fn progress_properties_preserve_native_tween_target_and_display() {
    let (mut app, owner, _) = fixture("ProgressBar", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().get_mut::<GuiProgressValue>(view).unwrap().0 = 0.;
    app.update();
    app.world_mut()
        .gui_mut(view)
        .unwrap()
        .tween_progress(80., 1.);
    app.update();
    let first = app
        .world()
        .get::<GuiProgressState>(view)
        .unwrap()
        .displayed_value();
    app.world_mut()
        .get_mut::<GuiProgressState>(view)
        .unwrap()
        .title_type = 2;
    app.update();
    let state = app.world().get::<GuiProgressState>(view).unwrap();
    assert!(state.is_tweening());
    assert_eq!(state.value, 80.);
    assert!(state.displayed_value() > first && state.displayed_value() < 80.);
}

#[test]
fn observers_can_write_components_in_the_current_frame_and_editors_are_immediate() {
    let (mut app, owner, _) = fixture("Button1", true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(
        |event: On<GuiEvent>, mut texts: Query<&mut GuiText>| {
            texts.get_mut(event.entity).unwrap().set("Clicked");
        },
    );
    let object = app.world().get::<GObjectRef>(view).unwrap().0;
    app.world_mut().trigger(GuiEvent::new(
        view,
        crate::core::object::UiEvent::new(crate::core::object::EventKind::Click, object),
    ));
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object_text(object),
        Some("Clicked")
    );
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_text("Immediate");
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object_text(object),
        Some("Immediate")
    );
    app.update();
    assert_eq!(
        app.world().get::<GuiText>(view).unwrap().value(),
        "Immediate"
    );
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world_mut().gui_mut(view).is_none());
}
