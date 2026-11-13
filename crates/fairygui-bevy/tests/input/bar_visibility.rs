use super::*;
use fairygui_core::{
    package::LoadContext,
    scroll::{GRIP_NAME, ScrollBarDisplayType},
    widget::ControlData,
};

#[derive(Resource, Default)]
struct Step(f32);

fn fixture() -> (App, Entity, GObjectId, GObjectId, GObjectId, GObjectId) {
    let (mut app, window) = setup();
    let package = fairygui_core::FuiPackage::parse(
        "Chat",
        include_bytes!("../../../fairygui-core/tests/fixtures/Chat.fui"),
    )
    .unwrap();
    let (root, list, bar, grip) = {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        let index = gui
            .create_root(Handle::default(), &package, "Main")
            .unwrap();
        let root = gui.root_object(index).unwrap();
        let mut descendants = Vec::new();
        gui.store()
            .for_each_descendant(root, &mut |id| descendants.push(id));
        let list = descendants
            .into_iter()
            .find(|&id| {
                matches!(
                    gui.store().object(id).unwrap().control(),
                    ControlData::List(_)
                ) && gui.store().scroll_pane(id).is_some()
            })
            .unwrap();
        gui.store_mut().attach_scroll_bars(list, &mut |store, url| {
            fairygui_core::widget::create_from_package(
                store,
                &package,
                package.resolve_local_url(url)?,
                &LoadContext::none(),
            )
            .ok()
        });
        let row = gui.store_mut().create(ObjectType::Component);
        gui.store_mut().set_parent(row, Some(list));
        gui.store_mut().set_size(row, 250., 600.);
        gui.store_mut().set_size(list, 300., 100.);
        gui.store_mut().layout_list(list);
        gui.store_mut().layout_scroll_bars(list);
        gui.store_mut()
            .set_scroll_bar_display(list, ScrollBarDisplayType::Auto);
        gui.store_mut().sync_scroll_bars(list);
        gui.store_mut().attach_to_stage(root);
        let bar = gui.store().scroll_pane(list).unwrap().vt_bar.unwrap();
        let grip = gui.store().child_by_name(bar, GRIP_NAME).unwrap();
        (root, list, bar, grip)
    };
    let (view, _) = target(&mut app, 100);
    app.world_mut().entity_mut(view).insert(GObjectRef(grip));
    app.init_resource::<Step>().add_systems(
        Update,
        (move |mut gui: NonSendMut<GuiRuntime>, step: Res<Step>| {
            gui.store_mut().advance_scrolls(step.0);
            gui.store_mut().sync_scroll_bars(list);
        })
        .after(GuiSystems::Input),
    );
    (app, window, root, list, bar, grip)
}

fn wake(app: &mut App, list: GObjectId) {
    let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
    gui.store_mut().begin_scroll_drag(list);
    gui.store_mut().end_scroll_drag(list);
}

#[test]
fn held_grip_survives_idle_time_then_fades_and_hidden_grip_cannot_capture() {
    let (mut app, _, _, list, bar, _) = fixture();
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(bar)
            .unwrap()
            .visible()
    );
    press(&mut app);
    assert!(captured_object(&app).is_none());
    release(&mut app);
    wake(&mut app, list);
    press(&mut app);
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .grip_dragging(true)
    );
    app.world_mut().resource_mut::<Step>().0 = 5.;
    for _ in 0..2 {
        app.update();
    }
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(gui.store().object(bar).unwrap().visible());
    assert_eq!(gui.store().object(bar).unwrap().alpha(), 1.);
    app.world_mut().resource_mut::<Step>().0 = 0.;
    release(&mut app);
    assert!(
        !app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .grip_dragging(true)
    );
    app.world_mut().resource_mut::<Step>().0 = 0.75;
    app.update();
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .object(bar)
            .unwrap()
            .alpha(),
        0.25
    );
    app.world_mut().resource_mut::<Step>().0 = 0.25;
    app.update();
    let gui = app.world().non_send::<GuiRuntime>();
    assert!(!gui.store().object(bar).unwrap().visible());
    assert_eq!(gui.store().object(bar).unwrap().alpha(), 1.);
    app.world_mut().resource_mut::<Step>().0 = 0.;
    press(&mut app);
    assert!(captured_object(&app).is_none());
    release(&mut app);
}

#[test]
fn grip_capture_cancellation_releases_visibility_on_focus_hide_detach_and_disposal() {
    for reason in 0..6 {
        let (mut app, window, root, list, bar, grip) = fixture();
        wake(&mut app, list);
        press(&mut app);
        assert_eq!(captured_object(&app), Some(grip));
        if reason == 0 {
            app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        } else {
            let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
            match reason {
                1 => gui.store_mut().set_visible(bar, false),
                2 => {
                    gui.store_mut().detach_from_parent(bar);
                }
                3 => {
                    gui.store_mut().remove(grip);
                }
                4 => {
                    gui.store_mut().remove(list);
                }
                5 => {
                    gui.store_mut().detach_from_stage(root);
                }
                _ => unreachable!(),
            }
        }
        app.update();
        assert!(captured_object(&app).is_none(), "reason={reason}");
        if let Some(pane) = app
            .world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
        {
            assert!(!pane.grip_dragging(true), "reason={reason}");
        }
    }
}
