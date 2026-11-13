use super::*;
use crate::{FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiTransitionComplete, GuiWorldExt};
use bevy::ecs::system::SystemState;
use fairygui_core::{package::FuiPackage, transition::TransitionPlayOptions};

fn fixture(item: &str) -> (App, Entity, Entity) {
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
    let package = FuiPackage::parse(
        "Transition",
        include_bytes!("../../fairygui-core/tests/fixtures/Transition.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Transition".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, item)).id();
    app.update();
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    (app, owner, view)
}

fn send(app: &mut App, target: Entity, action: impl FnOnce(&mut crate::GuiObjectMut)) {
    action(
        &mut app
            .world_mut()
            .gui_mut(target)
            .expect("GUI object must be ready"),
    );
}

fn take(app: &mut App) -> Vec<GuiTransitionComplete> {
    app.world_mut()
        .resource_mut::<Messages<GuiTransitionComplete>>()
        .drain()
        .collect()
}

#[test]
fn ecs_post_layout_commands_share_one_cursor_and_do_not_replay_early_commands() {
    #[derive(Resource)]
    struct Target(Entity);
    fn play(
        target: Res<Target>,
        mut once: Local<bool>,
        _commands: Commands,
        mut gui_edit: crate::GuiMut,
    ) {
        if !*once {
            *once = true;
            gui_edit.edit(target.0, |object: &mut crate::GuiObjectMut| {
                object.play_transition_with_options(
                    "t0",
                    TransitionPlayOptions {
                        end_time: Some(0.),
                        ..Default::default()
                    },
                )
            });
        }
    }
    let (mut app, owner, view) = fixture("PowerUp");
    let initial = app
        .world()
        .get::<GuiTransitions>(view)
        .unwrap()
        .get("t0")
        .unwrap()
        .play_generation();
    app.insert_resource(Target(owner))
        .add_systems(Update, play.in_set(crate::GuiSystems::PostLayout));
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.play_transition_with_options(
            "t0",
            TransitionPlayOptions {
                end_time: Some(0.),
                ..Default::default()
            },
        )
    });
    app.update();
    let done = take(&mut app);
    assert_eq!(
        done.iter()
            .map(|done| done.play_generation)
            .collect::<Vec<_>>(),
        vec![initial + 1, initial + 2]
    );
    app.update();
    assert!(take(&mut app).is_empty());
    assert_eq!(
        app.world()
            .get::<GuiTransitions>(view)
            .unwrap()
            .get("t0")
            .unwrap()
            .play_generation(),
        initial + 2
    );
}

#[test]
fn ecs_transition_completion_captures_immediate_playbacks_and_preserves_each_generation() {
    let (mut app, owner, view) = fixture("PowerUp");
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let initial = query
        .get(app.world())
        .unwrap()
        .transition(owner, "t0")
        .unwrap()
        .clone();
    assert!(!initial.is_playing());
    assert!(initial.duration() > 0.);
    assert_eq!(initial.time_scale(), 1.);
    assert!(
        query
            .get(app.world())
            .unwrap()
            .transition(owner, "missing")
            .is_none()
    );
    assert!(take(&mut app).is_empty());
    let instant = || {
        |object: &mut crate::GuiObjectMut| {
            object.play_transition_with_options(
                "t0",
                TransitionPlayOptions {
                    end_time: Some(0.),
                    ..Default::default()
                },
            )
        }
    };
    send(&mut app, owner, instant());
    app.update();
    let done = take(&mut app);
    assert_eq!(
        done.len(),
        1,
        "no frame boundary ever observed playing=true"
    );
    assert_eq!(done[0].entity, view);
    assert_eq!(done[0].transition, "t0");
    assert_eq!(done[0].play_generation, initial.play_generation() + 1);
    let state = query
        .get(app.world())
        .unwrap()
        .transition(owner, "t0")
        .unwrap()
        .clone();
    assert!(!state.is_playing());
    assert_eq!(state.play_generation(), done[0].play_generation);
    send(&mut app, view, instant());
    send(&mut app, owner, instant());
    app.update();
    let done = take(&mut app);
    assert_eq!(
        done.iter()
            .map(|event| event.play_generation)
            .collect::<Vec<_>>(),
        vec![state.play_generation() + 1, state.play_generation() + 2]
    );
    assert_eq!(
        query
            .get(app.world())
            .unwrap()
            .transition(owner, "t0")
            .unwrap()
            .play_generation(),
        done[1].play_generation
    );
    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiTransitions>()
        .unwrap()
        .last_changed();
    app.update();
    assert!(take(&mut app).is_empty());
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiTransitions>()
            .unwrap()
            .last_changed(),
        unchanged
    );
    let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let options = TransitionPlayOptions {
            end_time: Some(0.),
            ..Default::default()
        };
        runtime
            .store_mut()
            .play_transition_with_options(root, "t0", options);
        assert_eq!(runtime.store_mut().take_transition_completions().len(), 1);
        runtime
            .store_mut()
            .play_transition_with_options(root, "t0", options);
    }
    app.world_mut().despawn(owner);
    app.update();
    assert!(
        take(&mut app).is_empty(),
        "disposal cancels a queued natural completion"
    );
    assert!(app.world().get::<GuiTransitions>(view).is_none());
}

#[test]
fn ecs_transition_state_tracks_pause_speed_reverse_and_stop_without_fake_completion() {
    let (mut app, owner, view) = fixture("PowerUp");
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.play_transition("t0", 1)
    });
    app.update();
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_transition_paused("t0", true)
    });
    send(&mut app, view, |object: &mut crate::GuiObjectMut| {
        object.set_transition_time_scale("t0", 2.)
    });
    app.update();
    let frozen = app
        .world()
        .get::<GuiTransitions>(view)
        .unwrap()
        .get("t0")
        .unwrap()
        .clone();
    assert!(frozen.is_playing() && frozen.is_paused());
    assert_eq!(frozen.time_scale(), 2.);
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<GuiTransitions>(view).unwrap().get("t0"),
        Some(&frozen)
    );
    assert!(take(&mut app).is_empty());
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.stop_transition("t0")
    });
    app.update();
    assert!(
        !app.world()
            .get::<GuiTransitions>(view)
            .unwrap()
            .get("t0")
            .unwrap()
            .is_playing()
    );
    assert!(take(&mut app).is_empty());
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.play_transition_reverse("t0", 0.)
    });
    app.update();
    let reverse = app
        .world()
        .get::<GuiTransitions>(view)
        .unwrap()
        .get("t0")
        .unwrap();
    assert!(reverse.is_reversed() && reverse.is_playing());
    assert!(!reverse.is_paused());
    assert!(reverse.play_generation() > frozen.play_generation());
    assert_eq!(reverse.time_scale(), 2.);
    let generation = reverse.play_generation();
    let mut completions = Vec::new();
    for _ in 0..100 {
        app.update();
        completions.extend(take(&mut app));
        if completions
            .iter()
            .any(|event| event.entity == view && event.transition == "t0")
        {
            break;
        }
    }
    let completion = completions
        .iter()
        .filter(|event| event.entity == view && event.transition == "t0")
        .collect::<Vec<_>>();
    assert_eq!(completion.len(), 1);
    assert_eq!(completion[0].play_generation, generation);
    assert!(
        !app.world()
            .get::<GuiTransitions>(view)
            .unwrap()
            .get("t0")
            .unwrap()
            .is_playing()
    );
}

#[test]
fn ecs_transition_infinite_nested_tasks_do_not_complete_on_stop() {
    let (mut app, owner, view) = fixture("PathDemo");
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.play_transition("t0", 1)
    });
    app.update();
    let transitions = app.world().get::<GuiTransitions>(view).unwrap();
    assert!(transitions.get("t0").unwrap().is_playing());
    assert!(transitions.get("t1").unwrap().is_playing());
    assert!(take(&mut app).is_empty());
    let mut completed = Vec::new();
    for _ in 0..100 {
        app.update();
        completed.extend(take(&mut app));
        if completed
            .iter()
            .any(|event| event.entity == view && event.transition == "t0")
        {
            break;
        }
    }
    assert_eq!(
        completed
            .iter()
            .filter(|event| event.transition == "t0")
            .count(),
        0,
        "PathDemo's exported infinite tween keeps the parent playing"
    );
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.stop_transition("t0")
    });
    app.update();
    assert!(take(&mut app).is_empty());
    assert!(
        !app.world()
            .get::<GuiTransitions>(view)
            .unwrap()
            .get("t1")
            .unwrap()
            .is_playing()
    );
}
