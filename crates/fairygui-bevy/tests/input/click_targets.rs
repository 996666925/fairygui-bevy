use super::*;
use bevy::input::touch::TouchPhase;

#[derive(Clone, Copy, Debug)]
enum Contact {
    Left,
    Right,
    Touch,
}
impl Contact {
    fn begin(self, app: &mut App, window: Entity, view: Entity) {
        match self {
            Self::Left => press(app),
            Self::Right => press_right(app),
            Self::Touch => {
                touches::hits(app, 73, &[view]);
                touches::send(app, window, 73, TouchPhase::Started, Vec2::splat(10.));
            }
        }
    }
    fn end(self, app: &mut App, window: Entity, view: Option<Entity>) {
        match self {
            Self::Left => release(app),
            Self::Right => release_right(app),
            Self::Touch => {
                touches::hits(app, 73, &view.into_iter().collect::<Vec<_>>());
                touches::send(app, window, 73, TouchPhase::Ended, Vec2::splat(10.));
            }
        }
    }
    fn kind(self) -> EventKind {
        if matches!(self, Self::Right) {
            EventKind::RightClick
        } else {
            EventKind::Click
        }
    }
}

#[test]
fn all_contact_types_keep_press_time_recipients_across_tree_and_policy_changes() {
    for contact in [Contact::Left, Contact::Right, Contact::Touch] {
        for mutation in 0..9 {
            let (mut app, window) = setup();
            let (root_view, root) = target(&mut app, 0);
            let (parent_view, parent) = target(&mut app, 1);
            let (pressed_view, pressed) = target(&mut app, 3);
            let (released_view, released) = target(&mut app, 4);
            for view in [root_view, parent_view, released_view] {
                app.world_mut().entity_mut(view).insert(Hovered(false));
            }
            {
                let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                gui.store_mut().set_parent(parent, Some(root));
                gui.store_mut().set_parent(pressed, Some(parent));
                gui.store_mut().set_parent(released, Some(parent));
            }
            if matches!(contact, Contact::Touch) {
                app.init_resource::<bevy::picking::hover::HoverMap>();
            }
            contact.begin(&mut app, window, pressed_view);
            let expected = {
                let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
                match mutation {
                    0 => Some(pressed), // another release hit
                    1 => {
                        gui.store_mut().detach_from_parent(pressed);
                        Some(parent)
                    }
                    2 => {
                        gui.store_mut().remove(pressed);
                        Some(parent)
                    }
                    3 => {
                        gui.store_mut().set_visible(pressed, false);
                        Some(pressed)
                    }
                    4 => {
                        gui.store_mut().set_touchable(pressed, false);
                        Some(pressed)
                    }
                    5 => {
                        gui.store_mut().set_parent(pressed, Some(root));
                        Some(pressed)
                    }
                    6 => {
                        gui.store_mut().set_parent(released, Some(root));
                        gui.store_mut().detach_from_parent(parent);
                        Some(root)
                    }
                    7 => {
                        gui.store_mut().detach_from_parent(pressed);
                        None
                    } // release outside GUI
                    8 => Some(pressed), // original is still active, even with no release hit
                    _ => unreachable!(),
                }
            };
            let release_view = (mutation != 7 && mutation != 8).then_some(released_view);
            app.world_mut()
                .entity_mut(pressed_view)
                .insert(Hovered(false));
            app.world_mut()
                .entity_mut(released_view)
                .insert(Hovered(release_view.is_some()));
            contact.end(&mut app, window, release_view);
            let events: Vec<_> = app
                .world_mut()
                .non_send_mut::<GuiRuntime>()
                .drain_events()
                .filter(|event| event.kind == contact.kind())
                .collect();
            let targets: Vec<_> = events
                .iter()
                .filter(|event| event.is_at_target())
                .map(|event| event.target)
                .collect();
            assert_eq!(
                targets,
                expected.into_iter().collect::<Vec<_>>(),
                "{contact:?} mutation {mutation}"
            );
            if let Some(expected) = expected {
                let store = app.world().non_send::<GuiRuntime>();
                let mut path = vec![];
                let mut current = Some(expected);
                while let Some(id) = current {
                    path.push(id);
                    current = store.store().object(id).unwrap().parent();
                }
                assert_eq!(
                    events
                        .iter()
                        .map(|event| event.current_target)
                        .collect::<Vec<_>>(),
                    path
                );
                for event in &events {
                    assert_eq!(
                        event.touch_id,
                        matches!(contact, Contact::Touch).then_some(73)
                    );
                }
            }
            assert!(
                app.world()
                    .resource::<GuiInputState>()
                    .captures()
                    .is_empty()
            );
        }
    }
}

#[test]
fn a_release_without_a_recipient_still_advances_native_double_click_history() {
    let (mut app, _) = setup();
    let (view, first) = target(&mut app, 1);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    press(&mut app);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(first);
    app.world_mut().entity_mut(view).insert(Hovered(false));
    release(&mut app);
    assert!(clicks(&mut app).is_empty());
    target(&mut app, 2);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_millis(50));
    press(&mut app);
    release(&mut app);
    assert_eq!(clicks(&mut app)[0].click_count, 2);
}

#[derive(Resource, Default)]
struct Seen(Vec<(Entity, Option<Entity>)>);

#[test]
fn typed_observers_receive_one_fallback_click_on_the_current_ancestor_path() {
    let (mut app, _) = setup();
    app.init_resource::<Seen>().add_systems(
        Update,
        fairygui_bevy::runtime::emit_gui_events.after(GuiSystems::Input),
    );
    let (root_view, root) = target(&mut app, 0);
    let (parent_view, parent) = target(&mut app, 1);
    let (pressed_view, pressed) = target(&mut app, 3);
    let (release_view, released) = target(&mut app, 2);
    {
        let mut gui = app.world_mut().non_send_mut::<GuiRuntime>();
        gui.store_mut().set_parent(parent, Some(root));
        gui.store_mut().set_parent(pressed, Some(parent));
        gui.store_mut().set_parent(released, Some(parent));
    }
    for view in [root_view, parent_view, release_view] {
        app.world_mut().entity_mut(view).insert(Hovered(false));
    }
    for view in [root_view, parent_view] {
        app.world_mut()
            .entity_mut(view)
            .observe(|event: On<GuiClick>, mut seen: ResMut<Seen>| {
                seen.0.push((event.entity, event.target));
            });
    }
    press(&mut app);
    app.world_mut()
        .non_send_mut::<GuiRuntime>()
        .store_mut()
        .remove(pressed);
    app.world_mut()
        .entity_mut(pressed_view)
        .insert(Hovered(false));
    app.world_mut()
        .entity_mut(release_view)
        .insert(Hovered(true));
    release(&mut app);
    assert_eq!(
        app.world().resource::<Seen>().0,
        [
            (parent_view, Some(parent_view)),
            (root_view, Some(parent_view))
        ]
    );
}
