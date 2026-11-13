use super::*;
use crate::prelude::*;
use bevy::ecs::system::SystemState;
use fairygui_core::FuiPackage;

#[derive(Resource, Default)]
struct Completions(Vec<(Entity, f64, f64, f32)>);
fn completed(event: On<GuiProgressTweenComplete>, gui: GuiQuery, mut events: ResMut<Completions>) {
    let state = gui.progress(event.entity).unwrap();
    assert!(!state.is_tweening());
    assert_eq!(state.value(), event.value);
    let bar = gui.child_by_name(event.entity, "bar").unwrap();
    events.0.push((
        event.entity,
        state.value(),
        state.displayed_value(),
        gui.transform(bar).unwrap().size().x,
    ));
}
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
fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .init_resource::<Completions>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ));
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    if loaded {
        load(&mut app, &handle);
    }
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), "ProgressBar"))
        .id();
    (app, owner, handle)
}
fn send(app: &mut App, owner: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(owner, action));
}
fn state(app: &mut App, owner: Entity) -> GuiProgressState {
    *SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .progress(owner)
        .unwrap()
}
fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}
fn bind(app: &mut App, owner: Entity) -> Entity {
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(completed);
    view
}

#[test]
fn queued_fractional_values_and_tweens_are_ready_for_the_first_initialize() {
    #[derive(Resource, Default)]
    struct Initial(bool);
    fn inspect(
        roots: Query<Entity, Added<GuiRootView>>,
        gui: GuiQuery,
        mut initial: ResMut<Initial>,
    ) {
        for owner in &roots {
            let progress = gui.progress(owner).unwrap();
            assert_eq!(
                (
                    progress.min(),
                    progress.max(),
                    progress.value(),
                    progress.displayed_value()
                ),
                (0., 1., 0.75, 0.25)
            );
            assert!(progress.is_tweening());
            let bar = gui.child_by_name(owner, "bar").unwrap();
            assert_eq!(gui.transform(bar).unwrap().size().x, 35.);
            initial.0 = true;
        }
    }
    let (mut app, owner, handle) = fixture(false);
    app.init_resource::<Initial>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    send(
        &mut app,
        owner,
        GuiObjectAction::SetProgressRange { min: 0., max: 1. },
    );
    send(&mut app, owner, GuiObjectAction::SetProgress(0.25));
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 0.75,
            duration: 0.2,
        },
    );
    for _ in 0..4 {
        app.update();
    }
    assert!(!app.world().resource::<Initial>().0);
    load(&mut app, &handle);
    app.update();
    assert!(app.world().resource::<Initial>().0);
    let view = bind(&mut app, owner);
    app.update();
    near(state(&mut app, owner).displayed_value(), 0.5);
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, 0.75, 0.75, 105.)]
    );
    assert_eq!(app.world().get::<GuiProgressValue>(view).unwrap().0, 0.75);
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiProgressState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiProgressState>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn replacement_stops_and_same_target_setters_keep_native_target_display_semantics() {
    let (mut app, owner, _) = fixture(true);
    send(&mut app, owner, GuiObjectAction::SetProgress(0.));
    app.update();
    let view = bind(&mut app, owner);
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 100.,
            duration: 1.,
        },
    );
    app.update(); // starts in Sync
    assert_eq!(state(&mut app, owner).displayed_value(), 0.);
    app.update(); // sample=10
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 60.,
            duration: 0.2,
        },
    );
    app.update(); // sample=20, replacement starts there
    app.update(); // new sample=40
    near(state(&mut app, owner).displayed_value(), 40.);
    send(&mut app, owner, GuiObjectAction::SetProgress(60.)); // same target doesn't stop
    app.update(); // completes=60
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, 60., 60., 84.)]
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 100.,
            duration: 1.,
        },
    );
    app.update();
    send(&mut app, owner, GuiObjectAction::StopProgressTween);
    app.update();
    let stopped = state(&mut app, owner);
    near(stopped.displayed_value(), 64.);
    assert_eq!(stopped.value(), 100.);
    assert!(!stopped.is_tweening());
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(state(&mut app, owner), stopped);
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);
    send(&mut app, view, GuiObjectAction::SetProgress(-0.25));
    app.update();
    assert_eq!(state(&mut app, owner).displayed_value(), -0.25);
    assert_eq!(state(&mut app, owner).percent(), 0.);
}

#[test]
fn immediate_completion_replacement_and_disposal_publish_only_valid_final_states() {
    let (mut app, owner, _) = fixture(true);
    app.update();
    let view = bind(&mut app, owner);
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 75.,
            duration: 0.,
        },
    );
    send(&mut app, owner, GuiObjectAction::StopProgressTween);
    app.update();
    assert!(app.world().resource::<Completions>().0.is_empty());
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 25.,
            duration: 0.,
        },
    );
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, 25., 25., 35.)]
    );
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 75.,
            duration: 0.1,
        },
    );
    app.update();
    send(&mut app, owner, GuiObjectAction::SetProgress(12.5));
    app.update(); // completes in Advance, overwritten in Sync: don't notify stale 75
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);
    send(
        &mut app,
        owner,
        GuiObjectAction::TweenProgress {
            value: 50.,
            duration: 0.1,
        },
    );
    app.update();
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);
}
