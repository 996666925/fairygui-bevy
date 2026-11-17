use super::*;
use bevy::{input::touch::TouchPhase, math::Affine2};
use fairygui_core::{Size, package::LoadContext, scroll::ScrollBarDisplayType};

// Derive both-axis data in memory from Chat's real scrollbar export. Reuse its
// skin for both axes; the test controls part geometry explicitly below.
fn package_with_both_bars() -> fairygui_core::FuiPackage {
    use fairygui_core::binary::ByteBuffer;
    let bytes = include_bytes!("../../../fairygui-core/tests/fixtures/Chat.fui");
    let package = fairygui_core::FuiPackage::parse("Chat", bytes).unwrap();
    let main = package.item_by_name("Main").unwrap();
    let base = main.raw_data.clone().unwrap().start;
    let mut buffer = package.raw_data(main).unwrap();
    let mut body = package.data().to_vec();
    assert!(buffer.seek(0, 2).unwrap());
    let count = buffer.read_i16().unwrap();
    let mut patched = false;
    for _ in 0..count {
        let length = buffer.read_i16().unwrap() as usize;
        let start = buffer.position();
        assert!(buffer.seek(start, 0).unwrap());
        let kind = buffer.read_i8().unwrap();
        if kind == 10 && buffer.seek(start, 7).unwrap() {
            let scroll = buffer.position();
            body[base + scroll] = 2; // ScrollType::Both
            buffer.skip(6);
            if buffer.read_bool().unwrap() {
                buffer.skip(16);
            }
            let refs = base + buffer.position();
            body.copy_within(refs..refs + 2, refs + 2);
            patched = true;
        }
        buffer.set_position(start + length);
    }
    assert!(
        patched,
        "Chat.Main must contain its native list scroll block"
    );
    if bytes[8] == 0 {
        return fairygui_core::FuiPackage::parse("Chat", &body).unwrap();
    }
    let mut header = ByteBuffer::new(bytes);
    header.skip(9);
    header.read_utf_string().unwrap();
    header.read_utf_string().unwrap();
    header.skip(20);
    let mut derived = bytes[..header.position()].to_vec();
    derived[8] = 0; // uncompressed body
    derived.extend(body);
    fairygui_core::FuiPackage::parse("Chat", &derived).unwrap()
}

fn near(value: f32, expected: f32) {
    assert!((value - expected).abs() < 0.0002, "{value} != {expected}");
}
struct Fixture {
    app: App,
    window: Entity,
    pane: GObjectId,
    bar: GObjectId,
    track: GObjectId,
    grip: GObjectId,
    bar_view: Entity,
    hit: Entity,
    vertical: bool,
}
fn fixture(vertical: bool) -> Fixture {
    let (mut app, window) = setup();
    let package = package_with_both_bars();
    let (pane, bar, track, grip) = {
        let mut runtime = app.world_mut().non_send_mut::<GuiRuntime>();
        let root = runtime
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = runtime.root_object(root).unwrap();
        let store = runtime.store_mut();
        store.attach_to_stage(root);
        let mut descendants = Vec::new();
        store.for_each_descendant(root, &mut |id| descendants.push(id));
        let pane = descendants
            .into_iter()
            .find(|&id| store.scroll_pane(id).is_some())
            .unwrap();
        store.set_size(pane, 300., 200.);
        store.attach_scroll_bars(pane, &mut |store, url| {
            fairygui_core::widget::create_from_package(
                store,
                &package,
                package.resolve_local_url(url)?,
                &LoadContext::none(),
            )
            .ok()
        });
        store.set_scroll_bar_display(pane, ScrollBarDisplayType::Visible);
        let data = store.scroll_pane(pane).unwrap();
        let (vt, hz) = (data.vt_bar.unwrap(), data.hz_bar.unwrap());
        store.set_size(vt, 12., 200.);
        store.set_size(hz, 300., 12.);
        store.layout_scroll_bars(pane);
        let mut settings = store.list_layout_settings(pane).unwrap();
        settings.auto_resize_item = false;
        store.set_list_layout_settings(pane, settings);
        for row in store.list_items(pane).unwrap() {
            store.remove(row);
        }
        let row = store.create(ObjectType::Component);
        store.set_parent(row, Some(pane));
        store.set_size(row, 1000., 1000.);
        store.layout_list(pane);
        for (bar, axis_vertical) in [(vt, true), (hz, false)] {
            let track = store.child_by_name(bar, "bar").unwrap();
            let grip = store.child_by_name(bar, "grip").unwrap();
            store.set_xy(
                track,
                if axis_vertical { 0. } else { 10. },
                if axis_vertical { 10. } else { 0. },
            );
            store.set_size(
                track,
                if axis_vertical { 12. } else { 140. },
                if axis_vertical { 140. } else { 12. },
            );
            store.set_xy(grip, 0., 0.);
            store.set_size(
                grip,
                if axis_vertical { 12. } else { 28. },
                if axis_vertical { 28. } else { 12. },
            );
        }
        store.update_scroll_content_size(pane, Size::new(1000., 1000.));
        store.set_scroll_perc(pane, vertical, 0.3);
        store.sync_scroll_bars(pane);
        let data = store.scroll_pane(pane).unwrap();
        let bar = if vertical {
            data.vt_bar.unwrap()
        } else {
            data.hz_bar.unwrap()
        };
        (
            pane,
            bar,
            store.child_by_name(bar, "bar").unwrap(),
            store.child_by_name(bar, "grip").unwrap(),
        )
    };
    let (hit, _) = target(&mut app, 100);
    app.world_mut().entity_mut(hit).insert(GObjectRef(grip));
    let bar_view = app
        .world_mut()
        .spawn((
            GObjectRef(bar),
            Hovered(false),
            ComputedNode::default(),
            UiGlobalTransform::from(Affine2::IDENTITY),
        ))
        .id();
    Fixture {
        app,
        window,
        pane,
        bar,
        track,
        grip,
        bar_view,
        hit,
        vertical,
    }
}
fn perc(f: &Fixture) -> f32 {
    let pane = f
        .app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .scroll_pane(f.pane)
        .unwrap();
    if f.vertical {
        pane.perc_y()
    } else {
        pane.perc_x()
    }
}
fn sample(f: &mut Fixture, touch: bool, phase: TouchPhase, point: Vec2) {
    if touch {
        f.app.init_resource::<bevy::picking::hover::HoverMap>();
        touches::hits(&mut f.app, 71, &[f.hit]);
        touches::send(&mut f.app, f.window, 71, phase, point);
    } else {
        f.app
            .world_mut()
            .get_mut::<Window>(f.window)
            .unwrap()
            .set_cursor_position(Some(point));
        match phase {
            TouchPhase::Started => press(&mut f.app),
            TouchPhase::Ended => release(&mut f.app),
            _ => f.app.update(),
        }
    }
    if phase == TouchPhase::Started {
        let runtime = f.app.world().non_send::<GuiRuntime>();
        let path: Vec<_> = std::iter::successors(Some(f.grip), |&id| {
            runtime.store().object(id).and_then(|o| o.parent())
        })
        .map(|id| {
            let object = runtime.store().object(id).unwrap();
            (
                object.name().to_owned(),
                object.visible(),
                object.touchable(),
                runtime.store().is_on_stage(id),
            )
        })
        .collect();
        let captured = if touch {
            captured_touch_object(&f.app, 71)
        } else {
            captured_object(&f.app)
        };
        assert_eq!(
            captured,
            Some(f.grip),
            "vertical={} touch={touch} path={path:?}",
            f.vertical
        );
        assert!(
            runtime
                .store()
                .scroll_pane(f.pane)
                .unwrap()
                .grip_dragging(f.vertical),
            "grip gesture did not begin"
        );
    }
}
fn grab(f: &Fixture) -> Vec2 {
    let runtime = f.app.world().non_send::<GuiRuntime>();
    let grip = runtime.store().object(f.grip).unwrap();
    Vec2::new(grip.x() + 2., grip.y() + 3.)
}

#[test]
fn scrollbar_drag_uses_current_transform_track_origin_and_grip_size_for_mouse_and_touch() {
    for vertical in [false, true] {
        for touch in [false, true] {
            for matrix in [
                Mat2::IDENTITY,
                Mat2::from_angle(0.7) * Mat2::from_diagonal(Vec2::new(1.5, 0.6)),
                Mat2::from_cols(Vec2::new(-1.1, 0.3), Vec2::new(0.4, 1.)),
            ] {
                let mut f = fixture(vertical);
                let press = grab(&f);
                sample(&mut f, touch, TouchPhase::Started, press);
                near(perc(&f), 0.3);
                {
                    let mut runtime = f.app.world_mut().non_send_mut::<GuiRuntime>();
                    let store = runtime.store_mut();
                    store.set_xy(
                        f.track,
                        if vertical { 0. } else { 20. },
                        if vertical { 20. } else { 0. },
                    );
                    store.set_size(
                        f.track,
                        if vertical { 12. } else { 200. },
                        if vertical { 200. } else { 12. },
                    );
                    store.set_size(
                        f.grip,
                        if vertical { 12. } else { 60. },
                        if vertical { 60. } else { 12. },
                    );
                }
                let transform = Affine2 {
                    matrix2: matrix,
                    translation: Vec2::new(210., 170.),
                };
                f.app
                    .world_mut()
                    .entity_mut(f.bar_view)
                    .insert(UiGlobalTransform::from(transform));
                // Idle geometry changes cannot replay the press-time percentage.
                f.app.update();
                near(perc(&f), 0.3);
                let local = if vertical {
                    Vec2::new(2., 20. + 140. * 0.65 + 3.)
                } else {
                    Vec2::new(20. + 140. * 0.65 + 2., 3.)
                };
                sample(
                    &mut f,
                    touch,
                    TouchPhase::Moved,
                    transform.transform_point2(local),
                );
                near(perc(&f), 0.65);
                sample(
                    &mut f,
                    touch,
                    TouchPhase::Ended,
                    transform.transform_point2(local + Vec2::splat(50.)),
                );
                near(perc(&f), 0.65);
                assert!(
                    !f.app
                        .world()
                        .non_send::<GuiRuntime>()
                        .store()
                        .scroll_pane(f.pane)
                        .unwrap()
                        .grip_dragging(vertical)
                );
            }
        }
    }
}

#[test]
fn tiny_grip_motion_bypasses_content_drag_threshold_and_idle_preserves_scroll_edits() {
    for vertical in [false, true] {
        for touch in [false, true] {
            let mut f = fixture(vertical);
            let press = grab(&f);
            let track = f
                .app
                .world()
                .non_send::<GuiRuntime>()
                .store()
                .scroll_grip_track(f.bar)
                .unwrap();
            sample(&mut f, touch, TouchPhase::Started, press);
            let next = press + if vertical { Vec2::Y } else { Vec2::X };
            sample(&mut f, touch, TouchPhase::Moved, next);
            near(perc(&f), 0.3 + 1. / track);
            f.app
                .world_mut()
                .non_send_mut::<GuiRuntime>()
                .store_mut()
                .set_scroll_perc(f.pane, vertical, 0.8);
            f.app.update();
            near(perc(&f), 0.8);
            sample(
                &mut f,
                touch,
                TouchPhase::Moved,
                next + if vertical { Vec2::Y } else { Vec2::X },
            );
            near(perc(&f), 0.3 + 2. / track);
        }
    }
}
