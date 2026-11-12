use super::*;
use crate::action::ControllerAction;
use crate::transition::{ActionType, Transition, TransitionItem, TransitionValue, TweenConfig};
use crate::widget::Controller;

fn fixture(action: ControllerAction) -> (ObjectStore, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let data = store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap();
    data.controllers.push(Controller {
        name: "state".into(),
        page_ids: vec!["idle-id".into(), "active-id".into(), "other-id".into()],
        page_names: vec!["Idle".into(), "Active".into(), "Other".into()],
        selected_index: 0,
        actions: vec![action],
        ..Default::default()
    });
    let mut transition = Transition::default();
    transition.name = "fade".into();
    transition.total_duration = 1.0;
    transition.items.push(TransitionItem {
        action: ActionType::Alpha,
        time: 0.0,
        target_index: -1,
        label: None,
        value: TransitionValue::None,
        tween: Some(TweenConfig {
            duration: 1.0,
            ease_type: 0,
            repeat: 0,
            yoyo: false,
            end_label: None,
            path: None,
            start_value: TransitionValue::Scalar(0.0),
            end_value: TransitionValue::Scalar(1.0),
        }),
    });
    data.transitions.push(transition);
    store.set_alpha(owner, 0.3);
    (store, owner)
}

fn play_action(delay: f32, stop_on_exit: bool) -> ControllerAction {
    ControllerAction::PlayTransition {
        from_pages: vec![],
        to_pages: vec!["active-id".into()],
        transition_name: "fade".into(),
        play_times: 1,
        delay,
        stop_on_exit,
    }
}

fn alpha(store: &ObjectStore, owner: GObjectId) -> f32 {
    store.object(owner).unwrap().alpha()
}

#[test]
fn controller_action_matches_previous_and_current_ids_instead_of_names() {
    let mut action = play_action(0.0, false);
    if let ControllerAction::PlayTransition { from_pages, .. } = &mut action {
        from_pages.push("idle-id".into());
    }
    assert!(action.matches_pages(Some("idle-id"), Some("active-id")));
    assert!(!action.matches_pages(Some("other-id"), Some("active-id")));
    assert!(!action.matches_pages(Some("idle-id"), Some("other-id")));
    assert!(!action.matches_pages(None, Some("active-id")));
    assert!(!action.matches_pages(Some("Idle"), Some("Active")));
    let (mut store, owner) = fixture(action);
    assert!(store.set_controller_page(owner, "state", "Other"));
    assert_eq!(store.transition_state(owner, "fade"), Some((false, 0.0)));
    assert!(store.set_controller_page(owner, "state", "Active"));
    assert_eq!(store.transition_state(owner, "fade"), Some((false, 0.0)));
    store.set_controller_page(owner, "state", "Idle");
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.25);
    assert_eq!(alpha(&store, owner), 0.25);
}

#[test]
fn delay_consumes_only_the_waiting_part_of_a_frame() {
    let (mut store, owner) = fixture(play_action(0.5, true));
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.25);
    assert_eq!(alpha(&store, owner), 0.3);
    assert_eq!(store.transition_state(owner, "fade"), Some((true, 0.0)));
    store.advance_transitions(0.5);
    assert_eq!(store.transition_state(owner, "fade"), Some((true, 0.25)));
    assert_eq!(alpha(&store, owner), 0.25);
}

#[test]
fn leaving_during_delay_cancels_without_applying_the_animation() {
    let (mut store, owner) = fixture(play_action(1.0, true));
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.25);
    store.set_controller_page(owner, "state", "Other");
    store.advance_transitions(2.0);
    assert_eq!(store.transition_state(owner, "fade"), Some((false, 0.0)));
    assert_eq!(alpha(&store, owner), 0.3);
    // Entering again starts a fresh delay.
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.5);
    assert_eq!(alpha(&store, owner), 0.3);
    store.advance_transitions(0.75);
    assert_eq!(alpha(&store, owner), 0.25);
}

#[test]
fn leaving_completes_active_tweens_only_when_stop_on_exit_is_enabled() {
    for stop_on_exit in [true, false] {
        let (mut store, owner) = fixture(play_action(0.0, stop_on_exit));
        store.set_controller_page(owner, "state", "Active");
        store.advance_transitions(0.25);
        store.set_controller_page(owner, "state", "Other");
        if stop_on_exit {
            assert_eq!(alpha(&store, owner), 1.0);
            assert!(!store.transition_state(owner, "fade").unwrap().0);
        } else {
            assert_eq!(alpha(&store, owner), 0.25);
            assert!(store.transition_state(owner, "fade").unwrap().0);
            store.advance_transitions(0.25);
            assert_eq!(alpha(&store, owner), 0.5);
        }
    }
}

#[test]
fn nonmatching_action_does_not_stop_a_manually_started_transition() {
    let (mut store, owner) = fixture(play_action(0.0, true));
    store.play_transition(owner, "fade", 1);
    store.advance_transitions(0.25);
    store.set_controller_page(owner, "state", "Other");
    assert_eq!(store.transition_state(owner, "fade"), Some((true, 0.25)));
    assert_eq!(alpha(&store, owner), 0.25);
}

#[test]
fn leaving_completes_delayed_properties_without_playing_future_sounds() {
    let (mut store, owner) = fixture(play_action(0.0, true));
    let text = store.create(ObjectType::Text);
    store.set_parent(text, Some(owner));
    let data = store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap();
    data.transitions[0].items.extend([
        TransitionItem {
            action: ActionType::Text,
            time: 0.75,
            target_index: 0,
            label: None,
            value: TransitionValue::Text(Some("Done".into())),
            tween: None,
        },
        TransitionItem {
            action: ActionType::Sound,
            time: 0.75,
            target_index: -1,
            label: None,
            value: TransitionValue::Sound {
                sound: Some("future.wav".into()),
                volume: 1.0,
            },
            tween: None,
        },
    ]);
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.25);
    store.set_controller_page(owner, "state", "Other");
    assert_eq!(
        store
            .object(text)
            .unwrap()
            .control()
            .as_text_field()
            .unwrap()
            .text,
        "Done"
    );
    assert!(store.take_queued_sounds().is_empty());
}

#[test]
fn leaving_a_yoyo_tween_completes_at_the_return_value() {
    let (mut store, owner) = fixture(play_action(0.0, true));
    let tween = store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .transitions[0]
        .items[0]
        .tween
        .as_mut()
        .unwrap();
    tween.repeat = 1;
    tween.yoyo = true;
    store.set_controller_page(owner, "state", "Active");
    store.advance_transitions(0.25);
    store.set_controller_page(owner, "state", "Other");
    assert_eq!(alpha(&store, owner), 0.0);
}

#[test]
fn reentering_a_running_action_keeps_its_delay_and_progress() {
    let mut action = play_action(0.5, false);
    if let ControllerAction::PlayTransition { to_pages, .. } = &mut action {
        to_pages.clear();
    }
    let (mut store, owner) = fixture(action);
    store.set_controller_page_index(owner, "state", 1);
    store.advance_transitions(0.25);
    store.set_controller_page_index(owner, "state", 2);
    store.advance_transitions(0.5);
    assert_eq!(alpha(&store, owner), 0.25);
    store.set_controller_page_index(owner, "state", 1);
    store.advance_transitions(0.25);
    assert_eq!(alpha(&store, owner), 0.5);
    store.advance_transitions(0.5);
    assert!(!store.transition_state(owner, "fade").unwrap().0);
    store.set_controller_page_index(owner, "state", 2);
    store.advance_transitions(0.25);
    assert_eq!(store.transition_state(owner, "fade"), Some((true, 0.0)));
}

#[test]
fn delayed_play_api_handles_exact_boundary_and_missing_names() {
    let (mut store, owner) = fixture(play_action(0.0, false));
    assert!(!store.play_transition_delayed(owner, "missing", 1, 0.5));
    assert!(store.play_transition_delayed(owner, "fade", 1, 0.5));
    store.advance_transitions(0.5);
    assert_eq!(store.transition_state(owner, "fade"), Some((true, 0.0)));
    assert_eq!(alpha(&store, owner), 0.0);
    store.advance_transitions(0.25);
    assert_eq!(alpha(&store, owner), 0.25);
}

#[test]
fn removing_an_owner_cleans_up_action_state() {
    let (mut store, owner) = fixture(play_action(0.5, true));
    store.set_controller_page(owner, "state", "Active");
    assert_eq!(store.active_transition_actions.len(), 1);
    store.remove(owner);
    assert!(store.active_transition_actions.is_empty());
    store.advance_transitions(1.0);
}

#[test]
fn change_page_actions_cannot_reenter_a_controller_in_the_same_chain() {
    let (mut store, owner) = fixture(ControllerAction::ChangePage {
        from_pages: vec![],
        to_pages: vec![],
        object_id: String::new(),
        controller_name: "state".into(),
        target_page: "Idle".into(),
    });
    assert!(store.set_controller_page(owner, "state", "Active"));
    assert_eq!(
        store
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("state")
            .unwrap()
            .selected_page_name(),
        Some("Active")
    );
    assert!(store.changing_controllers.is_empty());
}
