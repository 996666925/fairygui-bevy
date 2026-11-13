use super::*;
use crate::prelude::*;
use bevy::{
    ecs::system::SystemState,
    picking::hover::Hovered,
    ui::{ComputedStackIndex, RelativeCursorPosition},
    window::PrimaryWindow,
};
use fairygui_core::{object::EventKind, package::FuiPackage};

fn load(app: &mut App, handle: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            handle.id(),
            FuiAsset {
                package: FuiPackage::parse(
                    "Basics",
                    include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
                )
                .unwrap(),
                res_key: "ui/Basics".into(),
            },
        )
        .unwrap();
}
fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>();
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &handle);
    }
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "Slider_HZ"))
        .id();
    let mut window = Window {
        focused: true,
        ..default()
    };
    window.set_cursor_position(Some(Vec2::new(50., 10.)));
    let window = app.world_mut().spawn((window, PrimaryWindow)).id();
    if loaded {
        app.update();
    }
    (app, owner, handle, window)
}
fn send(app: &mut App, target: Entity, action: impl FnOnce(&mut crate::GuiObjectMut)) {
    action(
        &mut app
            .world_mut()
            .gui_mut(target)
            .expect("GUI object must be ready"),
    );
}
fn slider(app: &mut App, owner: Entity) -> GuiSliderState {
    *SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .slider(owner)
        .unwrap()
}
fn part(app: &mut App, owner: Entity, name: &str) -> Entity {
    SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, name)
        .unwrap()
}
fn hover(app: &mut App, target: Entity) {
    let views: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<GObjectRef>>()
        .iter(app.world())
        .collect();
    for entity in views {
        app.world_mut()
            .entity_mut(entity)
            .insert(Hovered(entity == target));
    }
    app.world_mut().entity_mut(target).insert((
        ComputedStackIndex(100),
        RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::ZERO),
        },
    ));
}
fn mouse(app: &mut App, down: bool) {
    if down {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
    } else {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}

#[test]
fn slider_properties_wait_for_assets_and_initialize_fractional_geometry_on_the_first_frame() {
    #[derive(Resource, Default)]
    struct Initial(bool);
    fn inspect(
        roots: Query<Entity, Added<GuiRootView>>,
        gui: GuiQuery,
        mut initial: ResMut<Initial>,
    ) {
        for root in &roots {
            let slider = gui.slider(root).unwrap();
            assert_eq!(
                (slider.min(), slider.max(), slider.value()),
                (-2., 2., -0.5)
            );
            assert!(!slider.whole_numbers() && !slider.change_on_click());
            let bar = gui.child_by_name(root, "bar").unwrap();
            assert_eq!(gui.transform(bar).unwrap().size().x, 38.);
            initial.0 = true;
        }
    }
    let (mut app, owner, handle, _) = fixture(false);
    app.init_resource::<Initial>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    let mut settings = GuiSliderState::new(-2., 2., -0.5);
    settings.change_on_click = false;
    app.world_mut().entity_mut(owner).insert(settings);
    for _ in 0..4 {
        app.update();
    }
    assert!(!app.world().resource::<Initial>().0);
    load(&mut app, &handle);
    app.update();
    assert!(app.world().resource::<Initial>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiSliderState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiSliderState>()
            .unwrap()
            .last_changed(),
        tick
    );
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_slider_value(12.345)
    });
    app.update();
    assert_eq!(slider(&mut app, owner).value(), 12.345);
    assert_eq!(app.world().get::<GuiSliderValue>(view).unwrap().0, 12.345);
    let bar = part(&mut app, owner, "bar");
    assert_eq!(app.world().get::<GuiTransform>(bar).unwrap().size().x, 100.);
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(bar).is_err());
}

#[test]
fn grip_drag_does_not_jump_round_unrequested_values_or_lose_the_event_snapshot() {
    #[derive(Resource, Default)]
    struct Events(Vec<f64>);
    fn observe(event: On<GuiEvent>, gui: GuiQuery, mut events: ResMut<Events>) {
        if event.event.kind == EventKind::StateChanged && !event.event.bubbling {
            events.0.push(gui.slider(event.entity).unwrap().value());
        }
    }
    let (mut app, owner, _, window) = fixture(true);
    app.init_resource::<Events>();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(observe);
    let grip = part(&mut app, owner, "grip");
    hover(&mut app, grip);
    mouse(&mut app, true);
    assert_eq!(slider(&mut app, owner).value(), 50.);
    assert!(app.world().resource::<Events>().0.is_empty());
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(62.5, 10.)));
    app.update();
    assert_eq!(slider(&mut app, owner).value(), 62.5);
    assert_eq!(app.world().resource::<Events>().0, [62.5]);
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_slider_can_drag(false)
    });
    app.update();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(90., 10.)));
    app.update();
    assert_eq!(slider(&mut app, owner).value(), 62.5);
    mouse(&mut app, false);
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_slider_whole_numbers(true)
    });
    app.update();
    mouse(&mut app, true);
    assert!(
        slider(&mut app, owner).can_drag(),
        "a new grip-down enables native dragging"
    );
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(Vec2::new(91., 10.)));
    app.update();
    assert_eq!(slider(&mut app, owner).value(), 64.);
    mouse(&mut app, false);
    let count = app.world().resource::<Events>().0.len();
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_slider_value(12.25)
    });
    app.update();
    assert_eq!(slider(&mut app, owner).value(), 12.25);
    assert_eq!(
        app.world().resource::<Events>().0.len(),
        count,
        "programmatic setters do not emit user changes"
    );
}

#[test]
fn holding_the_grip_does_not_normalize_programmatic_values() {
    let (mut app, owner, _, _) = fixture(true);
    app.update();
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_slider_whole_numbers(true)
    });
    let grip = part(&mut app, owner, "grip");
    hover(&mut app, grip);
    for value in [2.5, -12.25, 112.25] {
        send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
            object.set_slider_value(value)
        });
        app.update();
        mouse(&mut app, true);
        for _ in 0..3 {
            app.update();
            assert_eq!(slider(&mut app, owner).value(), value);
        }
        mouse(&mut app, false);
        assert_eq!(slider(&mut app, owner).value(), value);
    }
}
