use super::*;
use crate::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::system::SystemState;

#[derive(Resource, Default)]
struct Completed(Vec<(Entity, GuiAnimationState)>);
fn completed(event: On<GuiAnimationComplete>, gui: GuiQuery, mut records: ResMut<Completed>) {
    records
        .0
        .push((event.entity, *gui.animation(event.entity).unwrap()));
}
fn fixture(item: &str, delayed: bool) -> (App, Entity, Handle<FuiAsset>, FuiAsset) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Completed>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ));
    let package = crate::core::FuiPackage::parse(
        "Basics",
        include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
    )
    .unwrap();
    let name = if item == "movie" {
        package
            .item(package.item_index_by_id("hixt1v").unwrap())
            .unwrap()
            .name
            .clone()
    } else {
        item.to_owned()
    };
    let handle = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(handle.clone(), name))
        .id();
    if !delayed {
        let package = crate::core::FuiPackage::parse(
            "Basics",
            include_bytes!("../../fairygui-demo/assets/ui/Basics.fui"),
        )
        .unwrap();
        app.world_mut()
            .resource_mut::<Assets<FuiAsset>>()
            .insert(
                handle.id(),
                FuiAsset {
                    package,
                    res_key: "ui/Basics".into(),
                },
            )
            .unwrap();
        app.update();
    }
    (
        app,
        owner,
        handle,
        FuiAsset {
            package,
            res_key: "ui/Basics".into(),
        },
    )
}
fn send(app: &mut App, entity: Entity, action: impl FnOnce(&mut crate::GuiObjectMut)) {
    action(
        &mut app
            .world_mut()
            .gui_mut(entity)
            .expect("GUI object must be ready"),
    );
}
fn view(app: &App, owner: Entity) -> Entity {
    app.world().get::<GuiRootView>(owner).unwrap().0
}
fn state(app: &mut App, entity: Entity) -> GuiAnimationState {
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    *queries.get(app.world()).unwrap().animation(entity).unwrap()
}
fn assert_painted_frame(app: &App, entity: Entity) {
    let player = app.world().get::<MovieClipPlayer>(entity).unwrap();
    let frame = player.current().unwrap();
    let image = app
        .world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .find_map(|child| app.world().get::<ImageNode>(child))
        .unwrap();
    assert_eq!(
        image.rect,
        Some(Rect::new(
            frame.rect.x,
            frame.rect.y,
            frame.rect.x + frame.rect.width,
            frame.rect.y + frame.rect.height
        ))
    );
}

#[test]
fn initial_and_post_layout_animation_methods_apply_in_the_same_sync() {
    let (mut app, owner, handle, asset) = fixture("movie", true);
    app.add_systems(
        Update,
        (|roots: Query<Entity, Added<GuiRootView>>, mut gui: GuiMut| {
            for owner in &roots {
                let mut object = gui.get_mut(owner).unwrap();
                object.set_animation_playing(false);
                object.set_animation_time_scale(2.);
                object.set_animation_frame(3);
                object.advance_animation(0.25);
                object.set_animation_play_settings(GuiAnimationPlaySettings {
                    start: 1,
                    end: 3,
                    times: 1,
                    end_at: 2,
                });
            }
        })
        .in_set(GuiSystems::Initialize),
    );
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().get::<GuiRootView>(owner).is_none());
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(handle.id(), asset)
        .unwrap();
    app.update();
    let entity = view(&app, owner);
    let current = state(&mut app, owner);
    assert_eq!(current.frame(), 1);
    assert!(!current.is_playing());
    assert!(!current.is_ended());
    assert!(current.frame_count() > 3);
    assert_eq!(current.time_scale(), 2.);
    assert_painted_frame(&app, entity);
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

    #[derive(Resource)]
    struct Target(Entity);
    fn edit(mut target: ResMut<Target>, mut commands: crate::GuiMut) {
        if target.0 == Entity::PLACEHOLDER {
            return;
        }

        commands.edit(target.0, |object: &mut crate::GuiObjectMut| {
            object.rewind_animation()
        });

        commands.edit(target.0, |object: &mut crate::GuiObjectMut| {
            object.advance_animation(0.5)
        });

        commands.edit(target.0, |object: &mut crate::GuiObjectMut| {
            object.set_animation_frame(2)
        });

        target.0 = Entity::PLACEHOLDER;
    }
    app.insert_resource(Target(owner))
        .add_systems(Update, edit.in_set(GuiSystems::PostLayout));
    app.update();
    assert_eq!(state(&mut app, owner).frame(), 2);
    assert_painted_frame(&app, entity);
    app.update();
    assert_eq!(state(&mut app, owner).frame(), 2);
}

fn play_once(app: &mut App, owner: Entity) {
    send(app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_playing(true)
    });
    send(app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_time_scale(100.)
    });
    send(app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_play_settings(GuiAnimationPlaySettings {
            start: 1,
            end: 2,
            times: 1,
            end_at: 3,
        })
    });
    app.world_mut()
        .run_system_once(crate::animation::sync_state)
        .unwrap();
}

#[test]
fn completion_targets_the_view_with_final_snapshot_and_rewind_does_not_restart() {
    let (mut app, owner, _, _) = fixture("movie", false);
    let entity = view(&app, owner);
    app.world_mut().entity_mut(entity).observe(completed);
    play_once(&mut app, owner);
    assert_eq!(state(&mut app, owner).frame(), 1);
    app.update();
    assert!(app.world().resource::<Completed>().0.is_empty());
    app.update();
    let records = &app.world().resource::<Completed>().0;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, entity);
    assert_eq!(records[0].1.frame(), 3);
    assert!(records[0].1.is_playing());
    assert!(records[0].1.is_ended());
    assert_painted_frame(&app, entity);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<GuiAnimationComplete>>()
            .drain()
            .collect::<Vec<_>>(),
        [GuiAnimationComplete { entity }]
    );
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.rewind_animation()
    });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(state(&mut app, owner).frame(), 0);
    assert!(state(&mut app, owner).is_ended());
    assert_eq!(app.world().resource::<Completed>().0.len(), 1);
    play_once(&mut app, owner);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Completed>().0.len(), 2);
}

#[test]
fn replacement_and_disposal_suppress_old_completions_but_invalid_requests_do_not() {
    let (mut app, owner, _, _) = fixture("movie", false);
    let entity = view(&app, owner);
    app.world_mut().entity_mut(entity).observe(completed);
    play_once(&mut app, owner);
    app.update(); // end frame reached; next Advance produces completion
    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_play_settings(GuiAnimationPlaySettings::default())
    });
    app.update();
    assert!(!state(&mut app, owner).is_ended());
    assert!(app.world().resource::<Completed>().0.is_empty());
    play_once(&mut app, owner);
    app.update();

    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_time_scale(f32::NAN)
    });

    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_time_scale(-1.)
    });

    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.advance_animation(f32::INFINITY)
    });

    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.advance_animation(-1.)
    });

    send(&mut app, owner, |object: &mut crate::GuiObjectMut| {
        object.set_animation_play_settings(GuiAnimationPlaySettings {
            start: -1,
            ..default()
        })
    });

    app.update();
    assert_eq!(app.world().resource::<Completed>().0.len(), 1);
    assert_eq!(state(&mut app, owner).time_scale(), 100.);
    play_once(&mut app, owner);
    app.update();
    app.world_mut().entity_mut(owner).despawn();
    app.update();
    assert_eq!(app.world().resource::<Completed>().0.len(), 1);
    assert!(app.world().get_entity(entity).is_err());
}

#[test]
fn loader_source_changes_and_controls_refresh_state_without_replacing_the_control() {
    let (mut app, owner, _, _) = fixture("Demo_MovieClip", false);
    let mut queries = SystemState::<GuiQuery>::new(app.world_mut());
    let loader = queries
        .get(app.world())
        .unwrap()
        .child_by_path(owner, "n18/icon")
        .unwrap();
    let movie_url = queries
        .get(app.world())
        .unwrap()
        .icon(loader)
        .unwrap()
        .to_owned();
    assert!(state(&mut app, loader).frame_count() > 3);
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_animation_playing(false)
    });
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_animation_frame(i32::MAX)
    });
    app.update();
    let current = state(&mut app, loader);
    assert_eq!(current.frame(), current.frame_count() as i32 - 1);
    assert_painted_frame(&app, loader);
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_icon("ui://9leh0eyfrpmbf")
    });
    app.update();
    assert_eq!(state(&mut app, loader).frame_count(), 0);
    assert!(app.world().get::<MovieClipPlayer>(loader).is_none());
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_icon(movie_url)
    });
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_animation_frame(-4)
    });
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.advance_animation(0.25)
    });
    app.update();
    let expected = {
        let mut player = app.world().get::<MovieClipPlayer>(loader).unwrap().clone();
        player.goto(0);
        player.advance_time(0.25);
        player.frame()
    };
    assert_eq!(state(&mut app, loader).frame(), expected as i32);
    assert!(!state(&mut app, loader).is_playing());
    assert_painted_frame(&app, loader);
    app.world_mut().entity_mut(loader).observe(completed);
    play_once(&mut app, loader);
    app.update();
    // Switching to an image before the next Advance
    // must suppress its notification on the retained Loader entity.
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_icon("ui://9leh0eyfrpmbf")
    });
    app.update();
    assert!(app.world().resource::<Completed>().0.is_empty());
    send(&mut app, loader, |object: &mut crate::GuiObjectMut| {
        object.set_icon("ui://9leh0eyfhixt1v")
    });
    play_once(&mut app, loader);
    app.update();
    app.update();
    assert_eq!(app.world().resource::<Completed>().0[0].0, loader);
}
