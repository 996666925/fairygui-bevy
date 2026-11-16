use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture() -> (App, Entity, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::ZERO,
    ));
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: crate::core::FuiPackage::parse(
                "ScrollPane",
                include_bytes!("../../fairygui-demo/assets/ui/ScrollPane.fui"),
            )
            .unwrap(),
            res_key: "ui/ScrollPane".into(),
        });
    let container = app
        .world_mut()
        .spawn((
            FairyGuiRoot::new(asset.clone(), "item").with_size(Vec2::new(200., 100.)),
            GuiScrollSettings {
                snap_to_item: false,
                ..default()
            },
        ))
        .id();
    let wrapper = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset.clone(), "item")
                .with_parent(container)
                .with_position(Vec2::new(300., 0.))
                .with_size(Vec2::new(200., 100.)),
        )
        .id();
    let target = app
        .world_mut()
        .spawn(
            FairyGuiRoot::new(asset.clone(), "item")
                .with_parent(wrapper)
                .with_position(Vec2::new(80., 0.))
                .with_size(Vec2::new(20., 20.)),
        )
        .id();
    app.world_mut().spawn(
        FairyGuiRoot::new(asset, "item")
            .with_parent(container)
            .with_position(Vec2::new(900., 0.))
            .with_size(Vec2::new(20., 20.)),
    );
    app.update();
    (app, container, wrapper, target)
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn scroll(app: &App, owner: Entity) -> &GuiScrollState {
    app.world().get::<GuiScrollState>(view(app, owner)).unwrap()
}

#[test]
fn entity_scroll_targets_convert_nested_geometry_and_publish_retained_animation() {
    let (mut app, container, wrapper, target) = fixture();
    let container_view = view(&app, container);
    let target_view = view(&app, target);
    assert!(scroll(&app, container).max_position().x > 400.);
    app.world_mut()
        .gui_mut(wrapper)
        .unwrap()
        .set_scale(Vec2::new(2., 1.));
    assert!(
        app.world_mut()
            .gui_scroll_object_to_view(container, target, false, true)
    );
    app.update();
    assert_eq!(scroll(&app, container).position().x, 400.);
    assert_eq!(scroll(&app, container).display_position().x, 400.);
    assert_eq!(
        app.world()
            .get::<ScrollPosition>(container_view)
            .unwrap()
            .0
            .x,
        400.
    );
    // Native nested conversion includes the current displayed content offset:
    // 460 - 400 = 60, so page alignment requests page zero on a second reveal.
    assert!(
        app.world_mut()
            .gui_scroll_object_to_view(container_view, target_view, false, false)
    );
    app.update();
    assert_eq!(scroll(&app, container).position().x, 0.);
    app.world_mut()
        .gui_mut(container)
        .unwrap()
        .scroll_to(Vec2::ZERO, false);
    let mut service = SystemState::<GuiMut>::new(app.world_mut());
    assert!(
        service
            .get_mut(app.world_mut())
            .unwrap()
            .scroll_object_to_view(container_view, target, true, true)
    );
    app.update();
    assert_eq!(scroll(&app, container).position().x, 400.);
    assert_eq!(scroll(&app, container).display_position().x, 0.);
    assert!(scroll(&app, container).is_animating());
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(125),
    ));
    app.update();
    let moving = scroll(&app, container).display_position().x;
    assert!(moving > 0. && moving < 400.);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(scroll(&app, container).display_position().x, 400.);
    assert!(!scroll(&app, container).is_animating());
    assert_eq!(view(&app, container), container_view);
    assert_eq!(view(&app, target), target_view);
    // Direct children use the logical x, ignoring their own scale.
    assert!(
        app.world_mut()
            .gui_scroll_object_to_view(container, wrapper, false, true)
    );
    app.update();
    assert_eq!(scroll(&app, container).position().x, 200.);
}

#[test]
fn entity_scroll_targets_reject_invalid_geometry_and_stale_owners_without_moving() {
    let (mut app, container, _, target) = fixture();
    let original = scroll(&app, container).position();
    assert!(
        !app.world_mut()
            .gui_scroll_object_to_view(Entity::PLACEHOLDER, target, false, true)
    );
    assert!(!app.world_mut().gui_scroll_object_to_view(
        container,
        Entity::PLACEHOLDER,
        false,
        true
    ));
    // Singular conversion belongs to the destination container's inverse.
    app.world_mut()
        .gui_mut(container)
        .unwrap()
        .set_scale(Vec2::new(0., 1.));
    assert!(
        !app.world_mut()
            .gui_scroll_object_to_view(container, target, true, true)
    );
    app.world_mut()
        .gui_mut(container)
        .unwrap()
        .set_scale(Vec2::ONE);
    let source = app.world().get::<FairyGuiRoot>(target).unwrap().clone();
    app.world_mut()
        .entity_mut(target)
        .remove::<FairyGuiRoot>()
        .insert(source);
    assert!(
        !app.world_mut()
            .gui_scroll_object_to_view(container, target, true, true)
    );
    let mut service = SystemState::<GuiMut>::new(app.world_mut());
    assert!(
        !service
            .get_mut(app.world_mut())
            .unwrap()
            .scroll_object_to_view(container, target, false, true)
    );
    assert_eq!(scroll(&app, container).position(), original);
    app.update();
    assert!(
        app.world_mut()
            .gui_scroll_object_to_view(container, target, false, true)
    );
    let source = app.world().get::<FairyGuiRoot>(container).unwrap().clone();
    app.world_mut()
        .entity_mut(container)
        .remove::<FairyGuiRoot>()
        .insert(source);
    assert!(
        !app.world_mut()
            .gui_scroll_object_to_view(container, target, false, true)
    );
}
