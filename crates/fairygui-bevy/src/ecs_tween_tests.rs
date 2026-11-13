use super::*;
use crate::{
    FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiObjectAction, GuiObjectCommand, GuiTransform,
};

#[derive(Resource, Default)]
struct Completions(Vec<(Entity, GuiTweenProperty, Vec2)>);

fn completed(
    event: On<GuiTweenComplete>,
    transforms: Query<&GuiTransform>,
    mut completions: ResMut<Completions>,
) {
    completions.0.push((
        event.entity,
        event.property,
        transforms.get(event.entity).unwrap().position(),
    ));
}

fn completion_fixture() -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<Completions>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(100),
    ));
    let package = crate::core::FuiPackage::parse(
        "Guide",
        include_bytes!("../../fairygui-demo/assets/ui/Guide.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Guide".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "GuideLayer"))
        .id();
    app.update();
    let view = app.world().get::<crate::GuiRootView>(owner).unwrap().0;
    app.world_mut().entity_mut(view).observe(completed);
    (app, owner, view)
}

fn command(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}

#[test]
fn natural_completion_observes_final_geometry_once_on_the_view() {
    let (mut app, owner, view) = completion_fixture();
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::new(120., 70.)),
            duration: 0.2,
            ease: 0,
        },
    );
    app.update(); // The tween starts in Sync, after Advance.
    app.update();
    assert!(app.world().resource::<Completions>().0.is_empty());
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, GuiTweenProperty::Xy, Vec2::new(120., 70.))]
    );
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);

    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::new(5., 6.)),
            duration: 0.,
            ease: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0[1],
        (view, GuiTweenProperty::Xy, Vec2::new(5., 6.))
    );
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .tween_count(),
        0
    );
}

#[test]
fn stop_replacement_and_disposal_suppress_unconsumed_completions() {
    let (mut app, owner, view) = completion_fixture();
    for target in [
        GuiTweenTarget::Position(Vec2::splat(100.)),
        GuiTweenTarget::Alpha(0.),
    ] {
        command(
            &mut app,
            owner,
            GuiObjectAction::Tween {
                target,
                duration: 0.1,
                ease: 0,
            },
        );
    }
    app.update();
    // Both finish in Advance; Sync replaces XY but leaves Alpha's event intact.
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::splat(200.)),
            duration: 1.,
            ease: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, GuiTweenProperty::Alpha, Vec2::splat(100.))]
    );
    command(
        &mut app,
        owner,
        GuiObjectAction::StopTween(GuiTweenProperty::Xy),
    );
    // Start and stop an immediate tween in one command pass: no completion.
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Rotation(90.),
            duration: 0.,
            ease: 0,
        },
    );
    command(
        &mut app,
        view,
        GuiObjectAction::StopTween(GuiTweenProperty::Rotation),
    );
    app.update();
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::ZERO),
            duration: 0.1,
            ease: 0,
        },
    );
    app.update();
    app.world_mut().entity_mut(owner).despawn();
    app.update(); // Completes in Advance, disposed before notifications in Sync.
    assert_eq!(app.world().resource::<Completions>().0.len(), 1);
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn invalid_replacement_preserves_the_original_completion() {
    let (mut app, owner, view) = completion_fixture();
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::ONE),
            duration: 0.1,
            ease: 0,
        },
    );
    app.update();
    command(
        &mut app,
        owner,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::splat(f32::NAN)),
            duration: 0.,
            ease: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world().resource::<Completions>().0,
        [(view, GuiTweenProperty::Xy, Vec2::ONE)]
    );
}

#[test]
fn property_tween_commands_replace_per_property_stop_validate_and_dispose() {
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
    let package = crate::core::FuiPackage::parse(
        "Guide",
        include_bytes!("../../fairygui-demo/assets/ui/Guide.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/Guide".into(),
        });
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset, "GuideLayer"))
        .id();
    let send = |app: &mut App, action| {
        app.world_mut()
            .write_message(GuiObjectCommand::new(owner, action));
    };
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::new(100., 50.)),
            duration: 1.,
            ease: 0,
        },
    );
    app.update();
    let view = app.world().get::<crate::GuiRootView>(owner).unwrap().0;
    app.update();
    assert!((app.world().get::<GuiTransform>(view).unwrap().position().x - 10.).abs() < 0.01);
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::new(200., 100.)),
            duration: 1.,
            ease: 0,
        },
    );
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Alpha(0.),
            duration: 1.,
            ease: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .tween_count(),
        2
    );
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Position(Vec2::splat(f32::NAN)),
            duration: 1.,
            ease: 0,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .tween_count(),
        2
    );
    send(&mut app, GuiObjectAction::StopTween(GuiTweenProperty::Xy));
    app.update();
    let stopped = app.world().get::<GuiTransform>(view).unwrap().position();
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().position(),
        stopped
    );
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .tween_count(),
        1
    );
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Scale(Vec2::splat(2.)),
            duration: 0.,
            ease: 5,
        },
    );
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().scale(),
        Vec2::splat(2.)
    );
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Size(Vec2::new(160., 80.)),
            duration: 0.,
            ease: 5,
        },
    );
    send(
        &mut app,
        GuiObjectAction::Tween {
            target: GuiTweenTarget::Rotation(45.),
            duration: 0.,
            ease: 5,
        },
    );
    app.update();
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().size(),
        Vec2::new(160., 80.)
    );
    assert_eq!(
        app.world().get::<GuiTransform>(view).unwrap().rotation(),
        45.
    );
    app.world_mut().despawn(owner);
    app.update();
    app.update();
    assert_eq!(
        app.world()
            .non_send::<crate::GuiRuntime>()
            .store()
            .tween_count(),
        0
    );
}
