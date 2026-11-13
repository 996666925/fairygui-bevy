use super::tests::{assert_painted_frame, fixture, send, state, view};
use crate::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::{ecs::system::SystemState, prelude::*};

#[test]
fn delayed_owner_properties_and_timeline_metadata_are_available_at_initialize() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            let animation = gui.animation(owner).unwrap();
            assert!(!animation.playing && !animation.is_ended());
            assert_eq!((animation.frame, animation.time_scale), (3, 2.));
            assert!(animation.frame_count() > 3);
            seen.0 = true;
        }
    }
    let (mut app, owner, handle, asset) = fixture("movie", true);
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    app.world_mut()
        .entity_mut(owner)
        .insert(GuiAnimationState::new(false, 3, 2.));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(handle.id(), asset)
        .unwrap();
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let entity = view(&app, owner);
    assert_painted_frame(&app, entity);
    assert_eq!(
        app.world().get::<GuiAnimationState>(owner),
        app.world().get::<GuiAnimationState>(entity)
    );
    let tick = app
        .world()
        .entity(entity)
        .get_ref::<GuiAnimationState>()
        .unwrap()
        .last_changed();
    app.update();
    assert_eq!(
        app.world()
            .entity(entity)
            .get_ref::<GuiAnimationState>()
            .unwrap()
            .last_changed(),
        tick
    );
}

#[test]
fn respond_play_seek_speed_and_pause_apply_before_this_frames_clock_and_do_not_replay() {
    #[derive(Resource)]
    struct Edit {
        entity: Entity,
        step: u8,
    }
    fn edit(mut edit: ResMut<Edit>, mut animations: Query<&mut GuiAnimationState>) {
        let mut animation = animations.get_mut(edit.entity).unwrap();
        match edit.step {
            0 => {
                animation.playing = true;
                animation.frame = 1;
                animation.time_scale = 100.;
            }
            1 => animation.time_scale = 50.,
            2 => animation.playing = false,
            _ => {}
        }
        edit.step += 1;
    }
    let (mut app, owner, _, _) = fixture("movie", false);
    let entity = view(&app, owner);
    send(&mut app, owner, |object| {
        object.set_animation_playing(false);
        object.set_animation_frame(0);
    });
    app.update();
    app.insert_resource(Edit { entity, step: 0 })
        .add_systems(Update, edit.in_set(GuiSystems::Respond));
    app.update();
    assert_eq!(
        state(&mut app, owner).frame,
        2,
        "seek then advance exactly once"
    );
    assert_painted_frame(&app, entity);
    app.update();
    assert_eq!(
        state(&mut app, owner).frame,
        3,
        "speed-only edit retains the advanced frame"
    );
    app.update();
    assert_eq!(
        state(&mut app, owner).frame,
        3,
        "pause prevents this tick's advance"
    );
    assert!(
        !app.world()
            .get::<crate::movie::MovieClipPlayer>(entity)
            .unwrap()
            .is_playing()
    );
    assert_painted_frame(&app, entity);
}

#[test]
fn native_commands_merge_with_component_edits_and_invalid_speed_does_not_block_a_seek() {
    let (mut app, owner, _, _) = fixture("movie", false);
    let entity = view(&app, owner);
    send(&mut app, owner, |object| {
        object.set_animation_playing(false);
        object.set_animation_frame(2);
    });
    app.update();
    let mut expected = app
        .world()
        .get::<crate::movie::MovieClipPlayer>(entity)
        .unwrap()
        .clone();
    expected.advance_time(0.25);
    send(&mut app, owner, |object| object.advance_animation(0.25));
    app.world_mut()
        .get_mut::<GuiAnimationState>(entity)
        .unwrap()
        .time_scale = 3.;
    app.update();
    let current = state(&mut app, owner);
    assert_eq!(current.frame, expected.frame() as i32);
    assert_eq!(current.time_scale, 3.);
    assert!(!current.playing);
    for invalid in [f32::NAN, -1., f32::INFINITY] {
        let mut animation = app
            .world_mut()
            .get_mut::<GuiAnimationState>(entity)
            .unwrap();
        animation.frame = i32::MAX;
        animation.time_scale = invalid;
        app.update();
        let current = state(&mut app, owner);
        assert_eq!(current.frame, current.frame_count() as i32 - 1);
        assert_eq!(current.time_scale, 3.);
        assert_painted_frame(&app, entity);
    }
    app.world_mut()
        .get_mut::<GuiAnimationState>(entity)
        .unwrap()
        .frame = -8;
    app.update();
    assert_eq!(state(&mut app, owner).frame, 0);
    app.world_mut()
        .entity_mut(entity)
        .remove::<GuiAnimationState>();
    app.update();
    assert_eq!(state(&mut app, owner).time_scale, 3.);
}

#[test]
fn loader_properties_survive_missing_and_static_sources_and_apply_to_the_next_movie() {
    let (mut app, owner, _, _) = fixture("Demo_MovieClip", false);
    let loader = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_path(owner, "n18/icon")
        .unwrap();
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://Missing/Resource");
    {
        let mut animation = app
            .world_mut()
            .get_mut::<GuiAnimationState>(loader)
            .unwrap();
        animation.playing = false;
        animation.frame = 3;
        animation.time_scale = 2.;
    }
    app.world_mut()
        .run_system_once(crate::properties::sync)
        .unwrap();
    assert_eq!(state(&mut app, loader).frame_count(), 0);
    assert!(
        app.world()
            .get::<crate::movie::MovieClipPlayer>(loader)
            .is_some(),
        "the previous source has not been reconciled yet"
    );
    app.update();
    let missing = state(&mut app, loader);
    assert_eq!(
        (missing.frame, missing.time_scale, missing.frame_count()),
        (3, 2., 0)
    );
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://9leh0eyfhixt1v");
    app.update();
    assert_eq!(state(&mut app, loader).frame, 3);
    assert_painted_frame(&app, loader);
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://9leh0eyfrpmbf");
    app.update();
    assert_eq!(state(&mut app, loader).frame_count(), 0);
    app.world_mut()
        .get_mut::<GuiAnimationState>(loader)
        .unwrap()
        .frame = 1;
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://9leh0eyfhixt1v");
    app.update();
    let current = state(&mut app, loader);
    assert_eq!((current.frame, current.time_scale), (1, 2.));
    assert!(!current.playing && !current.is_ended());
    assert_painted_frame(&app, loader);
}

#[derive(Resource, Default)]
struct Completed(Vec<(Entity, GuiAnimationState)>);
fn completed(
    event: On<GuiAnimationComplete>,
    animations: Query<&GuiAnimationState>,
    mut events: ResMut<Completed>,
) {
    events
        .0
        .push((event.entity, *animations.get(event.entity).unwrap()));
}

#[test]
fn completion_metadata_is_current_and_ecs_source_replacement_cancels_the_old_movie() {
    let (mut app, owner, _, _) = fixture("Demo_MovieClip", false);
    let loader = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_path(owner, "n18/icon")
        .unwrap();
    app.init_resource::<Completed>();
    app.world_mut().entity_mut(loader).observe(completed);
    let start = |app: &mut App| {
        app.world_mut()
            .get_mut::<GuiAnimationState>(loader)
            .unwrap()
            .playing = true;
        app.world_mut()
            .get_mut::<GuiAnimationState>(loader)
            .unwrap()
            .time_scale = 100.;
        send(app, loader, |object| {
            object.set_animation_play_settings(GuiAnimationPlaySettings {
                start: 1,
                end: 2,
                times: 1,
                end_at: 3,
            })
        });
    };
    start(&mut app);
    app.update();
    assert_eq!(state(&mut app, loader).frame, 2);
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://9leh0eyfrpmbf");
    app.update();
    assert!(app.world().resource::<Completed>().0.is_empty());
    app.world_mut()
        .get_mut::<GuiIcon>(loader)
        .unwrap()
        .set("ui://9leh0eyfhixt1v");
    start(&mut app);
    app.update();
    app.update();
    app.update();
    let events = &app.world().resource::<Completed>().0;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].0, loader);
    assert_eq!(events[0].1.frame, 3);
    assert!(events[0].1.is_ended() && events[0].1.playing);
    assert_painted_frame(&app, loader);
    app.world_mut()
        .get_mut::<GuiAnimationState>(loader)
        .unwrap()
        .playing = false;
    app.update();
    assert!(state(&mut app, loader).is_ended());
    assert!(!state(&mut app, loader).playing);
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(loader).is_err());
    assert_eq!(app.world().resource::<Completed>().0.len(), 1);
}

#[test]
fn post_layout_edits_update_the_current_image_without_advancing_the_clock_twice() {
    #[derive(Resource)]
    struct Target(Option<Entity>);
    fn edit(mut target: ResMut<Target>, mut animations: Query<&mut GuiAnimationState>) {
        if let Some(entity) = target.0.take() {
            let mut animation = animations.get_mut(entity).unwrap();
            animation.playing = false;
            animation.frame = 2;
            animation.time_scale = 5.;
        }
    }
    let (mut app, owner, _, _) = fixture("movie", false);
    let entity = view(&app, owner);
    app.insert_resource(Target(Some(entity)))
        .add_systems(Update, edit.in_set(GuiSystems::PostLayout));
    app.update();
    let current = state(&mut app, owner);
    assert_eq!((current.frame, current.time_scale), (2, 5.));
    assert!(!current.playing);
    assert_painted_frame(&app, entity);
    app.update();
    assert_eq!(state(&mut app, owner).frame, 2);
}
