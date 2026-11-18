use super::skew_projection_tests::{assert_native_points, fixture};
use super::*;
use crate::GuiPixelSnapping;
use fairygui_core::{Point, package::ObjectType};

#[test]
fn fractional_layout_and_explicit_snapping_match_native_at_dpi_with_pivots_skew_and_images() {
    for dpi in [1., 1.75, 2.] {
        let (mut app, root) = fixture();
        app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = dpi;
        let (owner, child, image) = {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            let store = gui.store_mut();
            let owner = store.create(ObjectType::Component);
            store.set_parent(owner, Some(root));
            store.set_xy(owner, 130.25, 90.75);
            store.set_size(owner, 120.25, 80.75);
            store.set_pivot(owner, 0.25, 0.75, true);
            store.set_rotation(owner, 27.);
            store.set_skew(owner, 12., -8.);
            let child = store.create(ObjectType::Graph);
            store.set_parent(child, Some(owner));
            store.set_xy(child, -0.5, -1.5);
            store.set_size(child, 61.25, 41.75);
            store.set_pivot(child, 0.5, 0.25, false);
            let image = store.create(ObjectType::Image);
            store.set_parent(image, Some(owner));
            store.set_size(image, 40.25, 20.75);
            store.set_xy(image, 9.5, 11.25);
            store.set_pivot(image, 0.25, 0.75, true);
            store.set_image_flip(image, crate::FlipType::Horizontal);
            store.set_skew(image, -15., 18.);
            gui.queue_subtree(owner);
            (owner, child, image)
        };
        app.update();
        app.update();
        for enabled in [false, true, true, false, true, false] {
            let ids = [owner, child, image];
            let views = ids.map(|id| app.world().non_send::<GuiRuntime>().entity_of(id).unwrap());
            for view in views {
                app.world_mut()
                    .entity_mut(view)
                    .insert(GuiPixelSnapping(enabled));
            }
            app.update();
            // No extra frame may be needed for a display-only property edit.
            assert_native_points(&app, root, &ids);
            for (view, id) in views.into_iter().zip(ids) {
                let gui = app.world().non_send::<GuiRuntime>();
                let object = gui.store().object(id).unwrap();
                let node = app.world().get::<ComputedNode>(view).unwrap();
                assert!(
                    (node.size() * node.inverse_scale_factor())
                        .abs_diff_eq(Vec2::new(object.width(), object.height()), 0.001)
                );
                assert_eq!(object.pixel_snapping(), enabled);
                assert_eq!(
                    app.world()
                        .get::<crate::GuiTransform>(view)
                        .unwrap()
                        .position,
                    Vec2::new(object.x(), object.y())
                );
            }
        }
    }
}

#[test]
fn snapping_uses_logical_pixels_without_rounding_fractional_parent_or_child_dimensions() {
    let (mut app, root) = fixture();
    app.world_mut().resource_mut::<bevy::ui::UiScale>().0 = 1.75;
    let (parent, child) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let store = gui.store_mut();
        let parent = store.create(ObjectType::Component);
        store.set_parent(parent, Some(root));
        store.set_xy(parent, 100.25, 80.75);
        store.set_size(parent, 120.5, 90.25);
        let child = store.create(ObjectType::Component);
        store.set_parent(child, Some(parent));
        store.set_xy(child, -0.5, -1.5);
        store.set_size(child, 20.25, 12.75);
        store.set_pixel_snapping(child, true);
        gui.queue_subtree(parent);
        (parent, child)
    };
    app.update();
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert_eq!(
        gui.store().local_to_global(child, Point::ZERO).unwrap(),
        gui.store()
            .local_to_global(parent, Point::new(0., -1.))
            .unwrap()
    );
    let pview = gui.entity_of(parent).unwrap();
    let view = gui.entity_of(child).unwrap();
    let node = app.world().get::<ComputedNode>(view).unwrap();
    let transform = app.world().get::<UiGlobalTransform>(view).unwrap();
    let parent_node = app.world().get::<ComputedNode>(pview).unwrap();
    let parent_transform = app.world().get::<UiGlobalTransform>(pview).unwrap();
    let actual = transform.transform_point2(node.border_box().min);
    let expected = parent_transform.transform_point2(
        parent_node.border_box().min + Vec2::new(0., -1.) / node.inverse_scale_factor(),
    );
    assert!(
        actual.abs_diff_eq(expected, 0.001),
        "{actual:?} vs {expected:?}"
    );
    assert!(
        (node.size() * node.inverse_scale_factor()).abs_diff_eq(Vec2::new(20.25, 12.75), 0.001)
    );
}
