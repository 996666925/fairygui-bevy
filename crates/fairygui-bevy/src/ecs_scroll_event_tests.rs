use super::*;
use crate::{GuiMut, GuiScroll, GuiScrollEnd};

#[derive(Resource, Default)]
struct Seen(Vec<(bool, GuiScrollState)>);

fn scroll(event: On<GuiScroll>, gui: GuiQuery, mut seen: ResMut<Seen>) {
    assert_eq!(event.target, Some(event.entity));
    assert!(!event.event.bubbling);
    seen.0.push((false, *gui.scroll(event.entity).unwrap()));
}

fn end(event: On<GuiScrollEnd>, gui: GuiQuery, mut seen: ResMut<Seen>) {
    seen.0.push((true, *gui.scroll(event.entity).unwrap()));
}

fn watch(app: &mut App, owner: Entity, list: GObjectId, view: Entity) {
    app.init_resource::<Seen>();
    app.world_mut()
        .entity_mut(view)
        .observe(scroll)
        .observe(end);
    let parent = app
        .world()
        .non_send::<GuiRuntime>()
        .store()
        .object(list)
        .unwrap()
        .parent()
        .unwrap();
    let parent = app
        .world()
        .non_send::<GuiRuntime>()
        .entity_of(parent)
        .unwrap();
    for entity in [parent, owner] {
        app.world_mut()
            .entity_mut(entity)
            .observe(|_: On<GuiScroll>| panic!("Scroll must not bubble"))
            .observe(|_: On<GuiScrollEnd>| panic!("ScrollEnd must not bubble"));
    }
}

#[test]
fn scroll_events_publish_current_state_and_finish_once_without_bubbling() {
    let (mut app, owner, list, view) = fixture();
    watch(&mut app, owner, list, view);
    send(&mut app, view, |object| {
        object.scroll_to(Vec2::new(0., 1000.), true)
    });
    for _ in 0..8 {
        app.update();
    }
    let seen = &app.world().resource::<Seen>().0;
    assert!(seen.len() >= 5);
    assert_eq!(seen.iter().filter(|(end, _)| *end).count(), 1);
    let (ended, state) = seen.last().unwrap();
    assert!(*ended);
    assert_eq!(state.position().y, 1000.);
    assert_eq!(state.display_position(), state.position());
    assert!(!state.is_animating());
    assert!(seen.iter().any(|(_, state)| state.display_position().y > 0.
        && state.display_position().y < 1000.
        && state.is_animating()));
    let count = seen.len();
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Seen>().0.len(), count);
}

#[test]
fn immediate_scrolls_coalesce_and_refresh_callbacks_can_edit_without_feedback() {
    let (mut app, owner, list, view) = fixture();
    watch(&mut app, owner, list, view);
    app.world_mut()
        .entity_mut(view)
        .observe(|event: On<GuiScroll>, mut gui: GuiMut| {
            gui.edit(event.entity, |object| {
                object.scroll_to(Vec2::new(0., 4000.), false)
            });
        });
    send(&mut app, view, |object| {
        object.scroll_to(Vec2::new(0., 500.), false);
        object.scroll_to(Vec2::new(0., 1000.), false);
    });
    app.update();
    assert_eq!(app.world().resource::<Seen>().0.len(), 1);
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .position()
            .y,
        4000.
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .virtual_item_object(list, 32)
            .is_some()
    );
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().resource::<Seen>().0.len(), 1);
}

#[test]
fn component_edits_from_scroll_callbacks_apply_with_scroll_edits_in_the_same_frame() {
    let (mut app, owner, list, view) = fixture();
    watch(&mut app, owner, list, view);
    app.world_mut().entity_mut(view).observe(
        |event: On<GuiScroll>,
         mut settings: Query<&mut crate::GuiScrollSettings>,
         mut gui: GuiMut| {
            settings.get_mut(event.entity).unwrap().scroll_step = 17.;
            gui.edit(event.entity, |object| {
                object.scroll_to(Vec2::new(0., 4000.), false)
            });
        },
    );
    send(&mut app, view, |object| {
        object.scroll_to(Vec2::new(0., 1000.), false)
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<GuiScrollState>(view)
            .unwrap()
            .position()
            .y,
        4000.
    );
    assert!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .virtual_item_object(list, 32)
            .is_some()
    );
    assert_eq!(
        app.world()
            .non_send::<GuiRuntime>()
            .store()
            .scroll_pane(list)
            .unwrap()
            .scroll_step(),
        17.
    );
    app.update();
    assert_eq!(app.world().resource::<Seen>().0.len(), 1);
}
