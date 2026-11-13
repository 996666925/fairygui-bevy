use super::*;
use crate::{
    FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiObjectAction, GuiObjectCommand, GuiTransform,
};

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
