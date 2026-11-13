use super::*;
use crate::{FairyGuiPlugin, FairyGuiRoot, FuiAsset, GuiObjectAction, GuiObjectCommand};
use bevy::ecs::system::SystemState;
use fairygui_core::{
    geom::Rect,
    object::{EventKind, GObjectId},
    package::FuiPackage,
};

fn fixture() -> (App, Entity, GObjectId, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        FairyGuiPlugin,
    ))
    .init_resource::<ButtonInput<MouseButton>>()
    .add_message::<bevy::input::mouse::MouseWheel>()
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_millis(125),
    ));
    let package = FuiPackage::parse(
        "VirtualList",
        include_bytes!("../../fairygui-demo/assets/ui/VirtualList.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package,
            res_key: "ui/VirtualList".into(),
        });
    let owner = app.world_mut().spawn(FairyGuiRoot::new(asset, "Main")).id();
    app.update();
    let list = {
        let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let list = runtime.store().child_by_name(root, "mailList").unwrap();
        runtime
            .enable_virtual_list(list, |store, index, row| {
                store.set_object_text(row, &format!("Row {index}"));
            })
            .unwrap();
        runtime.set_virtual_item_count(list, 1000).unwrap();
        list
    };
    app.update();
    let view = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(list)
        .unwrap();
    (app, owner, list, view)
}

fn send(app: &mut App, target: Entity, action: GuiObjectAction) {
    app.world_mut()
        .write_message(GuiObjectCommand::new(target, action));
}

#[test]
fn ecs_scroll_commands_keep_logical_targets_display_animation_and_virtual_rows_in_sync() {
    let (mut app, owner, list, view) = fixture();
    let mut query = SystemState::<GuiQuery>::new(app.world_mut());
    let initial = *app.world().get::<GuiScrollState>(view).unwrap();
    let gui = query.get(app.world()).unwrap();
    assert_eq!(
        gui.scroll(gui.find_by_name(owner, "mailList").unwrap()),
        Some(&initial)
    );
    assert!(gui.scroll(owner).is_none());
    assert_eq!(initial.position(), Vec2::ZERO);
    assert_eq!(initial.current_page(), IVec2::ZERO);
    assert!(initial.content_size().y > initial.view_size().y * 10.0);
    assert_eq!(
        initial.max_position().y,
        (initial.content_size().y - initial.view_size().y).ceil()
    );

    send(
        &mut app,
        view,
        GuiObjectAction::ScrollTo {
            position: Vec2::new(0., 1000.),
            animated: true,
        },
    );
    app.update();
    let started = *app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(started.position().y, 1000.);
    assert_eq!(started.display_position(), Vec2::ZERO);
    assert!(started.is_animating());
    app.update();
    let moving = *app.world().get::<GuiScrollState>(view).unwrap();
    assert!(moving.display_position().y > 0. && moving.display_position().y < 1000.);
    assert_eq!(moving.position(), started.position());
    assert!(moving.display_percentage().y < moving.percentage().y);
    assert_eq!(
        app.world().get::<ScrollPosition>(view).unwrap().0,
        moving.display_position()
    );
    for _ in 0..4 {
        app.update();
    }
    let settled = *app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(settled.position(), settled.display_position());
    assert!(!settled.is_animating());

    send(
        &mut app,
        view,
        GuiObjectAction::SetScrollPercentage {
            percentage: Vec2::splat(2.),
            animated: false,
        },
    );
    app.update();
    let bottom = *app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(bottom.position(), bottom.max_position());
    assert_eq!(bottom.percentage(), Vec2::new(0., 1.));
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .virtual_item_object(list, 999)
            .is_some()
    );
    send(
        &mut app,
        view,
        GuiObjectAction::SetScrollPercentage {
            percentage: Vec2::splat(-1.),
            animated: false,
        },
    );
    app.update();
    assert_eq!(
        app.world().get::<GuiScrollState>(view).unwrap().position(),
        Vec2::ZERO
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .virtual_item_object(list, 0)
            .is_some()
    );

    send(
        &mut app,
        view,
        GuiObjectAction::ScrollToView {
            rect: Rect::new(0., 700., 10., 45.),
            animated: false,
            set_first: true,
        },
    );
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .position()
            .y,
        750.,
        "the exported snap-to-item policy aligns to the next 125px row"
    );
    let unchanged = app
        .world()
        .entity(view)
        .get_ref::<GuiScrollState>()
        .unwrap()
        .last_changed();
    for action in [
        GuiObjectAction::SetScrollPercentage {
            percentage: Vec2::splat(f32::NAN),
            animated: true,
        },
        GuiObjectAction::ScrollTo {
            position: Vec2::splat(f32::INFINITY),
            animated: true,
        },
        GuiObjectAction::ScrollToView {
            rect: Rect::new(0., f32::NAN, 10., 45.),
            animated: true,
            set_first: true,
        },
    ] {
        send(&mut app, view, action);
    }
    app.update();
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiScrollState>()
            .unwrap()
            .last_changed(),
        unchanged
    );

    send(
        &mut app,
        view,
        GuiObjectAction::SetSize(Vec2::new(400., 200.)),
    );
    app.update();
    assert!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .view_size()
            .y
            < initial.view_size().y
    );
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .set_virtual_item_count(list, 0)
        .unwrap();
    app.update();
    let empty = app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(empty.max_position(), Vec2::ZERO);
    assert_eq!(empty.percentage(), Vec2::ZERO);
    assert_eq!(empty.display_percentage(), Vec2::ZERO);
    app.world_mut().despawn(owner);
    app.update();
    assert!(query.get(app.world()).unwrap().scroll(view).is_none());
}

#[test]
fn ecs_scroll_observers_see_input_changes_before_publishing_and_commands_use_current_ranges() {
    #[derive(Component)]
    struct WatchScroll(Entity);
    fn respond(
        event: On<crate::GuiEvent>,
        gui: GuiQuery,
        watch: Query<&WatchScroll>,
        mut commands: Commands,
    ) {
        if event.event().event.kind != EventKind::Click {
            return;
        }
        let target = watch.get(event.entity).unwrap().0;
        let state = gui.scroll(target).unwrap();
        assert_eq!(state.position().y, 123.);
        assert_eq!(state.display_position().y, 123.);
        commands.write_message(GuiObjectCommand::new(
            target,
            GuiObjectAction::SetScrollPercentage {
                percentage: Vec2::Y,
                animated: false,
            },
        ));
    }
    let (mut app, owner, list, view) = fixture();
    let (button, id) = {
        let runtime = app.world().non_send::<GuiRuntime>();
        let root = app.world().get::<GuiRootInstance>(owner).unwrap().object;
        let id = runtime.store().child_by_name(root, "n8").unwrap();
        (runtime.entity_of(id).unwrap(), id)
    };
    app.world_mut()
        .entity_mut(button)
        .insert(WatchScroll(view))
        .observe(respond);
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        runtime.store_mut().set_scroll_pos(list, 0., 123., false);
        runtime.dispatch(EventKind::Click, id);
    }
    // Resize before applying the observer's endpoint command. The command must
    // use the model's new range, not the range in the preceding ECS snapshot.
    send(
        &mut app,
        view,
        GuiObjectAction::SetSize(Vec2::new(400., 200.)),
    );
    app.update();
    let state = app.world().get::<GuiScrollState>(view).unwrap();
    assert_eq!(state.position(), state.max_position());
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .virtual_item_object(list, 999)
            .is_some()
    );
}
