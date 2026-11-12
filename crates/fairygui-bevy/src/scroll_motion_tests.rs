use super::*;

#[test]
fn actual_layout_projects_both_overscroll_edges_without_moving_bars_or_accumulating_offsets() {
    for ui_scale in [1., 2.] {
        let (mut app, _, list) = super::list_bar_tests::fixture_with_layout(true);
        app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = ui_scale;
        app.add_systems(Update, sync_scroll_extents.after(sync_dirty));
        app.add_systems(
            PostUpdate,
            project_scroll_overflow
                .after(bevy::ui::UiSystems::Layout)
                .before(bevy::ui::update::update_clipping_system),
        );
        let (row, child, bar) = {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            gui.store_mut().set_size(list, 200., 100.);
            gui.store_mut().set_scale(list, 1.25, 0.75);
            gui.store_mut().set_rotation(list, 17.);
            gui.store_mut().layout_scroll_bars(list);
            let row = gui
                .store_mut()
                .create(fairygui_core::package::ObjectType::Component);
            gui.store_mut().set_parent(row, Some(list));
            gui.store_mut().set_size(row, 180., 300.);
            let child = gui
                .store_mut()
                .create(fairygui_core::package::ObjectType::Graph);
            gui.store_mut().set_parent(child, Some(row));
            gui.store_mut().set_size(child, 20., 20.);
            gui.store_mut().set_xy(child, 7., 11.);
            gui.queue_subtree(row);
            gui.store_mut()
                .update_scroll_content_size(list, fairygui_core::geom::Size::new(180., 500.));
            let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
            (row, child, bar)
        };
        app.update();
        app.update();
        let gui = app.world().non_send::<GuiRuntime>();
        let views = [list, row, child, bar].map(|id| gui.entity_of(id).unwrap());
        let baseline = views.map(|e| app.world().get::<UiGlobalTransform>(e).unwrap().translation);
        let max = gui.store().scroll_pane(list).unwrap().max_scroll_pos().y;
        for position in [-30., -30., max + 35., max + 35., max, 0.] {
            app.world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_scroll_pos(list, 0., position, true);
            app.update();
            app.update();
            let owner = app.world().get::<UiGlobalTransform>(views[0]).unwrap();
            let node = app.world().get::<ComputedNode>(views[0]).unwrap();
            let expected =
                owner.transform_vector2(Vec2::new(0., -position / node.inverse_scale_factor()));
            for i in [1, 2] {
                let actual = app
                    .world()
                    .get::<UiGlobalTransform>(views[i])
                    .unwrap()
                    .translation
                    - baseline[i];
                assert!(
                    (actual - expected).length() <= 1.,
                    "DPI={ui_scale} position={position} actual={actual:?} expected={expected:?}"
                );
            }
            assert_eq!(
                app.world()
                    .get::<UiGlobalTransform>(views[3])
                    .unwrap()
                    .translation,
                baseline[3]
            );
            assert!(
                app.world()
                    .get::<bevy::ui::CalculatedClip>(views[1])
                    .is_some()
            );
        }
        app.world_mut()
            .non_send_mut::<GuiRuntime>()
            .store_mut()
            .remove(list);
        app.update();
        assert!(views.iter().all(|&e| app.world().get_entity(e).is_err()));
    }
}
