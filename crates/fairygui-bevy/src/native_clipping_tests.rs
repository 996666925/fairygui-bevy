use super::*;
use bevy::ui::{CalculatedClip, OverrideClip};

fn fixture() -> (App, GObjectId, GObjectId) {
    let (mut app, root, list) = super::list_bar_tests::fixture_with_layout(true);
    app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
    app.add_systems(
        PostUpdate,
        project_scroll_overflow
            .after(bevy::ui::UiSystems::Layout)
            .before(bevy::ui::update::update_clipping_system),
    );
    app.add_systems(
        PostUpdate,
        crate::native_clipping::project.after(bevy::ui::update::update_clipping_system),
    );
    (app, root, list)
}

fn point(app: &App, view: Entity, p: Vec2) -> Vec2 {
    let node = app.world().get::<ComputedNode>(view).unwrap();
    let transform = app.world().get::<UiGlobalTransform>(view).unwrap();
    transform.transform_point2(p / node.inverse_scale_factor() - node.size() * 0.5)
}
fn allowed(app: &App, view: Entity, p: Vec2) -> bool {
    app.world()
        .get::<CalculatedClip>(view)
        .is_none_or(|c| c.contains_point(p))
}

#[test]
fn pane_clips_content_inside_margins_but_keeps_ordinary_bars_outside_the_mask() {
    let (mut app, _, list) = fixture();
    let (row, bar) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        for row in gui.store().list_items(list).unwrap() {
            gui.store_mut().remove(row);
        }
        gui.store_mut().set_size(list, 300., 100.);
        gui.store_mut()
            .set_component_margin(list, fairygui_core::geom::Margin::new(13., 17., 19., 23.));
        let row = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_size(row, 250., 400.);
        gui.store_mut().set_parent(row, Some(list));
        gui.queue_subtree(row);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        (row, bar)
    };
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    let owner = gui.entity_of(list).unwrap();
    let row = gui.entity_of(row).unwrap();
    let bar = gui.entity_of(bar).unwrap();
    for p in [Vec2::new(20., 25.), Vec2::new(100., 70.)] {
        assert!(allowed(&app, row, point(&app, owner, p)));
    }
    for p in [
        Vec2::new(5., 25.),
        Vec2::new(20., 5.),
        Vec2::new(20., 85.),
        Vec2::new(290., 25.),
    ] {
        assert!(!allowed(&app, row, point(&app, owner, p)), "{p:?}");
    }
    let bar_point = point(&app, bar, Vec2::new(2., 20.));
    assert!(allowed(&app, bar, bar_point));
    assert!(!allowed(&app, row, bar_point));
    assert!(app.world().get::<CalculatedClip>(owner).is_none());
    // Native clips propagate into application children; their own overflow and
    // OverrideClip retain Bevy semantics rather than inheriting a stale viewport.
    let child = app
        .world_mut()
        .spawn((
            Node {
                width: Val::Px(50.),
                height: Val::Px(40.),
                overflow: Overflow::clip(),
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    let grandchild = app
        .world_mut()
        .spawn((Node::default(), ChildOf(child)))
        .id();
    app.update();
    assert_eq!(
        app.world()
            .get::<CalculatedClip>(grandchild)
            .unwrap()
            .rects()
            .unwrap()
            .len(),
        2
    );
    app.world_mut().entity_mut(child).insert(OverrideClip);
    app.update();
    assert!(app.world().get::<CalculatedClip>(child).is_none());
    assert_eq!(
        app.world()
            .get::<CalculatedClip>(grandchild)
            .unwrap()
            .rects()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn hidden_component_cancels_left_top_content_translation_and_clips_itself_and_children() {
    let (mut app, _, _) = fixture();
    app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = 1.75;
    let package = FuiPackage::parse(
        "Clipping",
        include_bytes!("../../fairygui-core/tests/fixtures/Clipping.fui"),
    )
    .unwrap();
    let asset = app
        .world_mut()
        .resource_mut::<Assets<FuiAsset>>()
        .add(FuiAsset {
            package: package.clone(),
            res_key: "ui/Basics".into(),
        });
    let (owner, child) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui.create_root(asset, &package, "Demo_Graph").unwrap();
        let owner = gui.root_object(index).unwrap();
        for child in gui.store().object(owner).unwrap().children().to_vec() {
            gui.store_mut().remove(child);
        }
        gui.store_mut().set_size(owner, 100., 80.);
        gui.store_mut().set_xy(owner, 40., 50.);
        gui.store_mut().set_scale(owner, 1.2, 0.8);
        gui.store_mut().set_rotation(owner, 17.);
        gui.store_mut()
            .set_component_margin(owner, fairygui_core::geom::Margin::new(12., 8., 20., 16.));
        let child = gui
            .store_mut()
            .create(fairygui_core::package::ObjectType::Graph);
        gui.store_mut().set_size(child, 160., 120.);
        gui.store_mut().set_parent(child, Some(owner));
        gui.queue_subtree(child);
        (owner, child)
    };
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store()
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .overflow,
        fairygui_core::widget::OverflowType::Hidden
    );
    let owner_view = gui.entity_of(owner).unwrap();
    let child_view = gui.entity_of(child).unwrap();
    assert!(
        (point(&app, owner_view, Vec2::ZERO) - point(&app, child_view, Vec2::ZERO)).length() < 1.
    );
    for p in [Vec2::new(1., 1.), Vec2::new(79., 63.)] {
        for e in [owner_view, child_view] {
            assert!(allowed(&app, e, point(&app, owner_view, p)));
        }
    }
    for p in [Vec2::new(-1., 1.), Vec2::new(81., 1.), Vec2::new(1., 65.)] {
        for e in [owner_view, child_view] {
            assert!(!allowed(&app, e, point(&app, owner_view, p)));
        }
    }
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .set_component_margin(owner, fairygui_core::geom::Margin::new(7., 9., 120., 16.));
    app.update();
    assert!(
        app.world()
            .get::<CalculatedClip>(child_view)
            .unwrap()
            .is_fully_clipped()
    );
}

#[test]
fn external_bevy_clips_remain_inherited_and_removal_does_not_leave_stale_native_clips() {
    let (mut app, root, list) = fixture();
    let gui = app.world().non_send::<GuiRuntime>();
    let stage = gui.entity_of(root).unwrap();
    let list_view = gui.entity_of(list).unwrap();
    let row = gui
        .entity_of(gui.store().list_item_at(list, 0).unwrap())
        .unwrap();
    let bar = gui
        .entity_of(gui.store().scroll_pane(list).unwrap().vt_bar.unwrap())
        .unwrap();
    let external = app
        .world_mut()
        .spawn(Node {
            width: Val::Px(220.),
            height: Val::Px(160.),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    // The fixture drives stage hierarchy without root_mount. Install the
    // external mount after that stage pass, as the full plugin does.
    app.add_systems(
        Update,
        (move |mut commands: Commands| {
            commands.entity(stage).insert(ChildOf(external));
        })
        .after(sync_stage_hierarchy),
    );
    app.update();
    assert_eq!(
        app.world().get::<ChildOf>(stage).unwrap().parent(),
        external
    );
    for e in [stage, list_view, bar] {
        assert_eq!(
            app.world()
                .get::<CalculatedClip>(e)
                .unwrap()
                .rects()
                .unwrap()
                .len(),
            1
        );
    }
    assert_eq!(
        app.world()
            .get::<CalculatedClip>(row)
            .unwrap()
            .rects()
            .unwrap()
            .len(),
        2
    );
    app.world_mut().get_mut::<Node>(external).unwrap().overflow = Overflow::visible();
    app.update();
    for e in [stage, list_view, bar] {
        assert!(app.world().get::<CalculatedClip>(e).is_none());
    }
    assert_eq!(
        app.world()
            .get::<CalculatedClip>(row)
            .unwrap()
            .rects()
            .unwrap()
            .len(),
        1
    );
}
