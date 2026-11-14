use crate::prelude::*;
use bevy::{ecs::system::SystemState, prelude::*};

fn fixture(loaded: bool) -> (App, Entity, Handle<FuiAsset>) {
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
    let asset = app.world().resource::<Assets<FuiAsset>>().reserve_handle();
    let owner = app
        .world_mut()
        .spawn(FairyGuiRoot::new(asset.clone(), "item"))
        .id();
    if loaded {
        load(&mut app, &asset);
        app.update();
    }
    (app, owner, asset)
}

fn load(app: &mut App, asset: &Handle<FuiAsset>) {
    app.world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .insert(
            asset.id(),
            FuiAsset {
                package: crate::core::FuiPackage::parse(
                    "ScrollPane",
                    include_bytes!("../../fairygui-demo/assets/ui/ScrollPane.fui"),
                )
                .unwrap(),
                res_key: "ui/ScrollPane".into(),
            },
        )
        .unwrap();
}

#[test]
fn live_inertia_flag_changes_release_planning_without_restarting_motion_or_disabling_bounce() {
    let (mut app, _, _) = fixture(true);
    // The usual ScrollPane/item fixture uses page alignment. Use an ordinary
    // vertical list to distinguish inertia from independent release snapping.
    let id = {
        let package = crate::core::FuiPackage::parse(
            "ListEffect",
            include_bytes!("../../fairygui-demo/assets/ui/ListEffect.fui"),
        )
        .unwrap();
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = runtime
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = runtime.root_object(index).unwrap();
        runtime.store().child_by_name(root, "mailList").unwrap()
    };
    let view = app.world_mut().spawn(GObjectRef(id)).id();
    crate::properties::sync(app.world_mut());
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        assert!(!store.scroll_pane(id).unwrap().page_mode);
        store.layout_scroll_bars(id);
        assert!(store.scroll_pane(id).unwrap().view_size().height > 0.);
        store.set_scroll_snap_to_item(id, false);
        store.set_scroll_bounceback_effect(id, true);
        store.update_scroll_content_size(id, crate::core::Size::new(100., 3000.));
        store.set_scroll_pos(id, 0., 200., false);
        store.begin_scroll_drag(id);
        store.drag_scroll_by(id, 0., -80., 0.016);
        store.drag_scroll_by(id, 0., -80., 0.016);
    }
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .inertia_disabled = true;
    crate::properties::sync(app.world_mut());
    let stop = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        assert!(store.scroll_pane(id).unwrap().is_dragging());
        let stop = store.scroll_pane(id).unwrap().display_pos();
        store.end_scroll_drag(id);
        store.advance_scrolls(0.2);
        assert_eq!(store.scroll_pane(id).unwrap().display_pos(), stop);
        assert!(!store.scroll_pane(id).unwrap().is_animating());
        store.begin_scroll_drag(id);
        store.drag_scroll_by(id, 0., -80., 0.016);
        store.drag_scroll_by(id, 0., -80., 0.016);
        stop
    };
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .inertia_disabled = false;
    crate::properties::sync(app.world_mut());
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        store.end_scroll_drag(id);
        assert!(store.scroll_pane(id).unwrap().is_animating());
        store.advance_scrolls(0.05);
        assert!(store.scroll_pane(id).unwrap().display_pos().y > stop.y + 160.);
    }
    let before = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(id)
        .unwrap()
        .display_pos();
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .inertia_disabled = true;
    crate::properties::sync(app.world_mut());
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        assert_eq!(store.scroll_pane(id).unwrap().display_pos(), before);
        assert!(store.scroll_pane(id).unwrap().is_animating());
        store.advance_scrolls(0.05);
        assert!(store.scroll_pane(id).unwrap().display_pos().y > before.y);
        store.set_scroll_pos(id, 0., 0., false);
        assert_eq!(
            store.scroll_pane(id).unwrap().display_pos(),
            crate::core::Point::ZERO
        );
        assert!(store.scroll_pane(id).unwrap().bounceback_effect);
        store.begin_scroll_drag(id);
        store.drag_scroll_by(id, 0., 60., 0.016);
        assert!(store.scroll_pane(id).unwrap().display_pos().y < 0.);
        store.end_scroll_drag(id);
        assert!(
            store.scroll_pane(id).unwrap().is_animating(),
            "inertia disabled must retain overscroll bounce"
        );
        store.advance_scrolls(1.);
        assert_eq!(
            store.scroll_pane(id).unwrap().display_pos(),
            crate::core::Point::ZERO
        );
        store.set_scroll_inertia_disabled(id, false);
    }
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .scroll_step = 9.;
    crate::properties::sync(app.world_mut());
    assert!(
        !app.world()
            .get::<GuiScrollSettings>(view)
            .unwrap()
            .inertia_disabled,
        "unrelated ECS edits must preserve native flag changes"
    );
}

#[test]
fn disabling_inertia_preserves_page_release_alignment() {
    let (mut app, owner, _) = fixture(true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let id = app.world().get::<GObjectRef>(view).unwrap().0;
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .inertia_disabled = true;
    crate::properties::sync(app.world_mut());
    let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
    let store = runtime.store_mut();
    let width = store.scroll_pane(id).unwrap().view_size().width;
    assert!(store.scroll_pane(id).unwrap().page_mode && width > 0.);
    store.update_scroll_content_size(id, crate::core::Size::new(width * 3., 80.));
    store.set_scroll_pos(id, 1., 0., false);
    store.set_scroll_pos(id, 0., 0., false);
    store.begin_scroll_drag(id);
    store.drag_scroll_by(id, -width * 0.4, 0., 0.016);
    let release = store.scroll_pane(id).unwrap().display_pos().x;
    assert!(release > 0. && release < width);
    store.end_scroll_drag(id);
    assert!(
        store.scroll_pane(id).unwrap().is_animating(),
        "page alignment remains active independently of inertia"
    );
    store.advance_scrolls(1.);
    assert_eq!(store.scroll_pane(id).unwrap().display_pos().x, width);
}

#[test]
fn pending_scroll_settings_apply_before_initialize_and_project_only_on_scroll_panes() {
    #[derive(Resource, Default)]
    struct Seen(bool);
    fn inspect(roots: Query<Entity, Added<GuiRootView>>, gui: GuiQuery, mut seen: ResMut<Seen>) {
        for owner in &roots {
            let settings = gui.scroll_settings(owner).unwrap();
            assert!(settings.mouse_wheel_enabled);
            assert!(!settings.touch_effect);
            assert!(settings.inertia_disabled);
            assert_eq!(settings.scroll_step, 12.5);
            assert_eq!(gui.scroll(owner).unwrap().position(), Vec2::ZERO);
            seen.0 = true;
        }
    }
    let (mut app, owner, asset) = fixture(false);
    let settings = GuiScrollSettings {
        scroll_step: 12.5,
        touch_effect: false,
        bounceback_effect: false,
        inertia_disabled: true,
        deceleration_rate: 0.925_000_000_000_000_2,
        ..default()
    };
    app.world_mut().entity_mut(owner).insert(settings.clone());
    app.init_resource::<Seen>()
        .add_systems(Update, inspect.in_set(GuiSystems::Initialize));
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().resource::<Seen>().0);
    load(&mut app, &asset);
    app.update();
    assert!(app.world().resource::<Seen>().0);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    assert_eq!(app.world().get::<GuiScrollSettings>(view), Some(&settings));
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .snap_to_item = true;
    app.update();
    assert!(
        app.world()
            .get::<GuiScrollSettings>(owner)
            .unwrap()
            .snap_to_item
    );
    let child = SystemState::<GuiQuery>::new(app.world_mut())
        .get(app.world())
        .unwrap()
        .child_by_name(owner, "b0")
        .unwrap();
    app.world_mut().entity_mut(child).insert(settings);
    app.update();
    assert!(app.world().get::<GuiScrollSettings>(child).is_none());
    app.world_mut().despawn(owner);
    app.update();
    assert!(app.world().get_entity(view).is_err());
}

#[test]
fn field_edits_keep_fresh_native_settings_physics_environment_and_running_tween() {
    let (mut app, owner, _) = fixture(true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let id = app.world().get::<GObjectRef>(view).unwrap().0;
    {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = runtime.store_mut();
        store.update_scroll_content_size(id, crate::core::geom::Size::new(3000., 80.));
        store.set_scroll_pos(id, 200., 0., false);
        store.set_scroll_pos_animated(id, 1000., 0., true);
        store.advance_scrolls(0.1);
        let mut physics = store.scroll_pane(id).unwrap().physics();
        physics.slow_frame_rate = true;
        physics.mobile_screen_extent = Some(2048.);
        physics.deceleration_rate = 0.91;
        store.set_scroll_physics(id, physics);
        store.set_scroll_step(id, -9.25);
        store.set_scroll_snap_to_item(id, true);
    }
    let before = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(id)
        .unwrap()
        .display_pos();
    {
        let mut settings = app.world_mut().get_mut::<GuiScrollSettings>(view).unwrap();
        settings.touch_effect = false;
        settings.bounceback_effect = false;
        settings.mouse_wheel_enabled = true;
    }
    crate::properties::sync(app.world_mut());
    let current = app.world().get::<GuiScrollSettings>(view).unwrap().clone();
    assert_eq!(current.scroll_step, -9.25);
    assert!(current.snap_to_item);
    assert_eq!(current.deceleration_rate, 0.91);
    assert!(!current.touch_effect && !current.bounceback_effect);
    {
        let runtime = app.world().non_send::<GuiRuntime>();
        let pane = runtime.store().scroll_pane(id).unwrap();
        assert_eq!(pane.display_pos(), before);
        assert!(pane.is_animating());
    }
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .deceleration_rate = 0.945_000_000_000_000_2;
    crate::properties::sync(app.world_mut());
    let runtime = app.world().non_send::<GuiRuntime>();
    let physics = runtime.store().scroll_pane(id).unwrap().physics();
    assert!(physics.slow_frame_rate);
    assert_eq!(physics.mobile_screen_extent, Some(2048.));
    assert_eq!(physics.deceleration_rate, 0.945_000_000_000_000_2);
}

#[test]
fn invalid_numbers_restore_native_values_zero_resets_default_and_direct_edits_stay_stable() {
    let (mut app, owner, _) = fixture(true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let initial = app.world().get::<GuiScrollSettings>(view).unwrap().clone();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0., 1., -0.5, 2.] {
        {
            let mut settings = app.world_mut().get_mut::<GuiScrollSettings>(view).unwrap();
            settings.deceleration_rate = invalid;
            settings.scroll_step = f32::NAN;
            settings.mouse_wheel_enabled = !initial.mouse_wheel_enabled;
        }
        app.update();
        let current = app.world().get::<GuiScrollSettings>(view).unwrap();
        assert_eq!(current.deceleration_rate, initial.deceleration_rate);
        assert_eq!(current.scroll_step, initial.scroll_step);
        assert_eq!(current.mouse_wheel_enabled, !initial.mouse_wheel_enabled);
    }
    app.world_mut()
        .gui_mut(owner)
        .unwrap()
        .set_scroll_settings(GuiScrollSettings {
            scroll_step: -8.5,
            deceleration_rate: 0.93,
            ..initial.clone()
        });
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollSettings>(view)
            .unwrap()
            .scroll_step,
        -8.5
    );
    app.world_mut()
        .get_mut::<GuiScrollSettings>(view)
        .unwrap()
        .scroll_step = 0.;
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollSettings>(view)
            .unwrap()
            .scroll_step,
        initial.scroll_step
    );
    let tick = app
        .world()
        .entity(view)
        .get_ref::<GuiScrollSettings>()
        .unwrap()
        .last_changed();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world()
            .entity(view)
            .get_ref::<GuiScrollSettings>()
            .unwrap()
            .last_changed(),
        tick
    );
}
