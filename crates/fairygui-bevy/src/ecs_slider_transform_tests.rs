use super::*;
use bevy::{
    input::touch::{TouchInput, TouchPhase},
    math::Affine2,
    picking::{backend::HitData, hover::HoverMap, pointer::PointerId},
};

#[derive(Resource, Default)]
struct Seen(Vec<(f64, Option<u64>)>);
fn changed(event: On<GuiStateChanged>, gui: GuiQuery, mut seen: ResMut<Seen>) {
    seen.0.push((
        gui.slider(event.entity).unwrap().value(),
        event.event.touch_id,
    ));
}
fn move_pointer(
    app: &mut App,
    window: Entity,
    grip: Entity,
    touch: bool,
    phase: TouchPhase,
    point: Vec2,
) {
    if touch {
        app.init_resource::<HoverMap>();
        app.world_mut().resource_mut::<HoverMap>().insert(
            PointerId::Touch(71),
            [(
                grip,
                HitData::new(Entity::PLACEHOLDER, 0., Some(Vec3::ZERO), None),
            )]
            .into(),
        );
        app.world_mut().write_message(TouchInput {
            window,
            id: 71,
            phase,
            position: point,
            force: None,
        });
        app.update();
    } else {
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(point));
        if phase == TouchPhase::Started {
            mouse(app, true);
        } else if phase == TouchPhase::Ended {
            mouse(app, false);
        } else {
            app.update();
        }
    }
}
fn near(value: f64, expected: f64) {
    assert!((value - expected).abs() < 0.0001, "{value} != {expected}");
}

#[test]
fn mouse_and_touch_drag_resample_translation_scale_rotation_skew_and_reflection() {
    for touch in [false, true] {
        for vertical in [false, true] {
            for matrix in [
                Mat2::IDENTITY,
                Mat2::from_diagonal(Vec2::new(1.8, 0.6)),
                Mat2::from_angle(0.8),
                Mat2::from_cols(Vec2::new(1.2, 0.3), Vec2::new(-0.4, 1.)),
                Mat2::from_diagonal(Vec2::new(-1.2, 0.9)),
            ] {
                for dpi in [1., 1.75] {
                    let (mut app, owner, _, window) = fixture(true);
                    if vertical {
                        app.world_mut()
                            .get_mut::<FairyGuiRoot>(owner)
                            .unwrap()
                            .item_name = "Slider_VT".into();
                        app.update();
                    }
                    app.world_mut()
                        .get_mut::<Window>(window)
                        .unwrap()
                        .resolution
                        .set_scale_factor_override(Some(dpi));
                    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
                    let grip = part(&mut app, owner, "grip");
                    hover(&mut app, grip);
                    app.init_resource::<Seen>();
                    app.world_mut().entity_mut(view).observe(changed);
                    let size = if vertical {
                        Vec2::new(23., 112.)
                    } else {
                        Vec2::new(112., 23.)
                    };
                    let origin = Vec2::new(200., 150.);
                    let make = |matrix, origin| {
                        UiGlobalTransform::from(Affine2 {
                            matrix2: matrix,
                            translation: (origin + matrix * size * 0.5) * dpi,
                        })
                    };
                    app.world_mut().entity_mut(view).insert((
                        ComputedNode {
                            size: size * dpi,
                            inverse_scale_factor: 1. / dpi,
                            ..default()
                        },
                        make(Mat2::IDENTITY, origin),
                    ));
                    let press = origin
                        + if vertical {
                            Vec2::new(10., 40.)
                        } else {
                            Vec2::new(40., 10.)
                        };
                    move_pointer(&mut app, window, grip, touch, TouchPhase::Started, press);
                    assert!(app.world().resource::<Seen>().0.is_empty());
                    let shifted = Vec2::new(270., 210.);
                    app.world_mut()
                        .entity_mut(view)
                        .insert(make(matrix, shifted));
                    // An idle pointer must not reapply raw setter values while UI animates.
                    app.update();
                    near(slider(&mut app, owner).value(), 50.);
                    let point = shifted
                        + matrix
                            * if vertical {
                                Vec2::new(10., 65.)
                            } else {
                                Vec2::new(65., 10.)
                            };
                    move_pointer(&mut app, window, grip, touch, TouchPhase::Moved, point);
                    near(slider(&mut app, owner).value(), 75.);
                    let seen = &app.world().resource::<Seen>().0;
                    assert_eq!(seen.len(), 1);
                    near(seen[0].0, 75.);
                    assert_eq!(seen[0].1, touch.then_some(71));
                    move_pointer(
                        &mut app,
                        window,
                        grip,
                        touch,
                        TouchPhase::Ended,
                        point + Vec2::splat(60.),
                    );
                    near(slider(&mut app, owner).value(), 75.);
                    assert_eq!(app.world().resource::<Seen>().0.len(), 1);
                }
            }
        }
    }
}

#[test]
fn singular_view_ignores_motion_and_resumes_from_the_original_local_press() {
    let (mut app, owner, _, window) = fixture(true);
    let view = app.world().get::<GuiRootView>(owner).unwrap().0;
    let grip = part(&mut app, owner, "grip");
    hover(&mut app, grip);
    app.world_mut()
        .entity_mut(view)
        .insert(UiGlobalTransform::from(Affine2::IDENTITY));
    move_pointer(
        &mut app,
        window,
        grip,
        false,
        TouchPhase::Started,
        Vec2::new(40., 10.),
    );
    app.world_mut()
        .entity_mut(view)
        .insert(UiGlobalTransform::from(Affine2::from_scale(Vec2::new(
            0., 1.,
        ))));
    move_pointer(
        &mut app,
        window,
        grip,
        false,
        TouchPhase::Moved,
        Vec2::new(60., 10.),
    );
    near(slider(&mut app, owner).value(), 50.);
    app.world_mut()
        .entity_mut(view)
        .insert(UiGlobalTransform::from(Affine2::IDENTITY));
    move_pointer(
        &mut app,
        window,
        grip,
        false,
        TouchPhase::Moved,
        Vec2::new(65., 10.),
    );
    near(slider(&mut app, owner).value(), 75.);
}
