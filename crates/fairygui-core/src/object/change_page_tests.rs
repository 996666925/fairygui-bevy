use super::*;
use crate::action::ControllerAction;
use crate::widget::{Controller, Gear, GearKind, GearTweenConfig, GearValue};

fn controller(name: &str, pages: &[(&str, &str)], selected: i32) -> Controller {
    Controller {
        name: name.into(),
        page_ids: pages.iter().map(|(id, _)| (*id).into()).collect(),
        page_names: pages.iter().map(|(_, name)| (*name).into()).collect(),
        selected_index: selected,
        ..Default::default()
    }
}

fn add_controller(store: &mut ObjectStore, owner: GObjectId, controller: Controller) {
    store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller);
}

fn add_action(
    store: &mut ObjectStore,
    owner: GObjectId,
    source: &str,
    target: &str,
    name: &str,
    page: &str,
) {
    store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controller_mut(source)
        .unwrap()
        .actions
        .push(ControllerAction::ChangePage {
            from_pages: vec![],
            to_pages: vec![],
            object_id: target.into(),
            controller_name: name.into(),
            target_page: page.into(),
        });
}

fn selected(store: &ObjectStore, owner: GObjectId, name: &str) -> i32 {
    store
        .object(owner)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller(name)
        .unwrap()
        .selected_index
}

fn fixture(target_page: &str) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let target = store.create(ObjectType::Component);
    store.set_parent(target, Some(owner));
    let object = store.object_mut(target).unwrap();
    object.instance_id = Some("instance-id".into());
    object.name = "target-name".into();
    object.package_item_id = Some("resource-id".into());
    add_controller(
        &mut store,
        owner,
        controller("source", &[("a-id", "A"), ("b-id", "B"), ("c-id", "C")], 0),
    );
    add_controller(
        &mut store,
        target,
        controller("destination", &[("x-id", "B"), ("y-id", "A")], 0),
    );
    add_action(
        &mut store,
        owner,
        "source",
        "instance-id",
        "destination",
        target_page,
    );
    (store, owner, target)
}

#[test]
fn change_page_uses_instance_and_page_ids_even_when_names_and_resource_ids_differ() {
    let (mut store, owner, target) = fixture("y-id");
    // A name collision must not intercept the ID lookup.
    let decoy = store.create(ObjectType::Component);
    store.set_parent(decoy, Some(owner));
    store.object_mut(decoy).unwrap().name = "instance-id".into();
    add_controller(
        &mut store,
        decoy,
        controller("destination", &[("x-id", "B"), ("y-id", "A")], 0),
    );
    assert!(store.set_controller_page(owner, "source", "B"));
    assert_eq!(selected(&store, target, "destination"), 1);
    assert_eq!(selected(&store, decoy, "destination"), 0);
    assert_eq!(store.child_by_id(owner, "resource-id"), None);
    assert_eq!(store.child_by_id(owner, "target-name"), None);
}

#[test]
fn child_id_lookup_and_actions_only_search_direct_children() {
    let (mut store, owner, target) = fixture("y-id");
    let wrapper = store.create(ObjectType::Component);
    store.set_parent(wrapper, Some(owner));
    store.set_parent(target, Some(wrapper));
    add_controller(
        &mut store,
        owner,
        controller("destination", &[("x-id", "B"), ("y-id", "A")], 0),
    );
    assert_eq!(store.child_by_id(owner, "instance-id"), None);
    assert_eq!(store.child_by_id(wrapper, "instance-id"), Some(target));
    store.set_controller_page(owner, "source", "B");
    assert_eq!(selected(&store, target, "destination"), 0);
    assert_eq!(selected(&store, owner, "destination"), 0);
}

#[test]
fn missing_target_does_not_fall_back_to_the_owners_controller() {
    let (mut store, owner, target) = fixture("y-id");
    add_controller(
        &mut store,
        owner,
        controller("destination", &[("x-id", "B"), ("y-id", "A")], 0),
    );
    store.remove(target);
    store.set_controller_page(owner, "source", "B");
    assert_eq!(selected(&store, owner, "destination"), 0);
}

#[test]
fn empty_object_id_addresses_another_controller_on_the_same_component() {
    let (mut store, owner, _) = fixture("y-id");
    add_controller(
        &mut store,
        owner,
        controller("local", &[("left-id", "Left"), ("right-id", "Right")], 0),
    );
    add_action(&mut store, owner, "source", "", "local", "right-id");
    store.set_controller_page(owner, "source", "B");
    assert_eq!(selected(&store, owner, "local"), 1);
}

#[test]
fn index_forwarding_keeps_a_shorter_target_and_propagates_deselection() {
    let (mut store, owner, target) = fixture("~1");
    store.set_controller_page_index(owner, "source", 1);
    assert_eq!(selected(&store, target, "destination"), 1);
    store.set_controller_page_index(owner, "source", 2);
    assert_eq!(
        selected(&store, target, "destination"),
        1,
        "source index outside target range is ignored"
    );
    store.set_controller_page_index(owner, "source", -1);
    assert_eq!(selected(&store, target, "destination"), -1);
}

#[test]
fn name_forwarding_uses_target_order_and_defaults_to_its_first_page() {
    let (mut store, owner, target) = fixture("~2");
    store.set_controller_page_index(owner, "source", 1); // B is target page 0.
    assert_eq!(selected(&store, target, "destination"), 0);
    store.set_controller_page_index(owner, "source", 0); // A is target page 1.
    assert_eq!(selected(&store, target, "destination"), 1);
    store.set_controller_page_index(owner, "source", 2); // C is absent.
    assert_eq!(selected(&store, target, "destination"), 0);
    store.set_controller_page_index(owner, "source", 0);
    store.set_controller_page_index(owner, "source", -1); // Null name also defaults to 0.
    assert_eq!(selected(&store, target, "destination"), 0);
}

#[test]
fn unknown_page_id_clears_selection_instead_of_matching_a_page_name() {
    let (mut store, owner, target) = fixture("A");
    store.set_controller_page(owner, "source", "B");
    assert_eq!(selected(&store, target, "destination"), -1);
    assert!(store.set_controller_page_id(target, "destination", "y-id"));
    assert_eq!(selected(&store, target, "destination"), 1);
    assert!(store.set_controller_page_id(target, "destination", "missing"));
    assert_eq!(selected(&store, target, "destination"), -1);
    assert!(!store.set_controller_page_id(target, "missing", "x-id"));
}

#[test]
fn chained_actions_drive_target_gears_and_stop_at_the_changing_controller() {
    let (mut store, owner, target) = fixture("y-id");
    add_controller(
        &mut store,
        target,
        controller("tail", &[("off-id", "Off"), ("on-id", "On")], 0),
    );
    add_action(&mut store, target, "destination", "", "tail", "on-id");
    add_action(&mut store, target, "tail", "", "destination", "x-id");
    let image = store.create(ObjectType::Image);
    store.set_parent(image, Some(target));
    let mut gear = Gear::new(GearKind::Xy);
    gear.controller_index = 1;
    gear.values.insert(
        Some("on-id".into()),
        GearValue::Xy {
            x: 37.0,
            y: 52.0,
            px: 0.0,
            py: 0.0,
        },
    );
    store.object_mut(image).unwrap().gears.push(gear);
    store.set_controller_page(owner, "source", "B");
    assert_eq!(
        selected(&store, target, "destination"),
        1,
        "cycle must not change destination back"
    );
    assert_eq!(selected(&store, target, "tail"), 1);
    assert_eq!(
        store.object(image).unwrap().position(),
        Point::new(37.0, 52.0)
    );
    assert!(store.changing_controllers.is_empty());
    store.set_controller_page_id(target, "destination", "x-id");
    assert_eq!(
        selected(&store, target, "destination"),
        0,
        "guard is released after the chain"
    );
}

#[test]
fn initial_controller_application_runs_actions_without_a_fake_page_change() {
    let (mut store, owner, target) = fixture("y-id");
    assert_eq!(selected(&store, owner, "source"), 0);
    store.apply_all_controllers(owner);
    assert_eq!(selected(&store, target, "destination"), 1);
    let source = store
        .object(owner)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller("source")
        .unwrap();
    assert_eq!(source.selected_index, 0);
    assert_eq!(source.previous_page_id(), None);
}

#[test]
fn first_application_does_not_match_a_filter_requiring_a_previous_page() {
    let (mut store, owner, target) = fixture("y-id");
    if let ControllerAction::ChangePage { from_pages, .. } = &mut store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controller_mut("source")
        .unwrap()
        .actions[0]
    {
        from_pages.push("a-id".into());
    }
    store.apply_all_controllers(owner);
    assert_eq!(selected(&store, target, "destination"), 0);
    store.set_controller_page(owner, "source", "B");
    assert_eq!(selected(&store, target, "destination"), 1);
}

#[test]
fn applying_a_self_targeted_action_does_not_switch_its_own_controller() {
    let (mut store, owner, _) = fixture("y-id");
    add_action(&mut store, owner, "source", "", "source", "b-id");
    store.apply_all_controllers(owner);
    assert_eq!(selected(&store, owner, "source"), 0);
}

#[test]
fn initialization_applies_chained_gears_immediately_then_runtime_changes_can_tween() {
    let (mut store, owner, target) = fixture("y-id");
    let image = store.create(ObjectType::Image);
    store.set_parent(image, Some(target));
    let mut gear = Gear::new(GearKind::Xy);
    gear.controller_index = 0;
    gear.tween = Some(GearTweenConfig::default());
    for (id, x) in [("x-id", 0.0), ("y-id", 100.0)] {
        gear.values.insert(
            Some(id.into()),
            GearValue::Xy {
                x,
                y: 0.0,
                px: 0.0,
                py: 0.0,
            },
        );
    }
    store.object_mut(image).unwrap().gears.push(gear);
    store.initialize_controllers(owner);
    assert_eq!(store.object(image).unwrap().x(), 100.0);
    assert!(store.gear_tweens.is_empty());
    assert!(!store.initializing_controllers);
    store.set_controller_page_id(target, "destination", "x-id");
    assert_eq!(store.object(image).unwrap().x(), 100.0);
    assert_eq!(store.gear_tweens.len(), 1);
    store.advance_gear_tweens(1.0);
    assert_eq!(store.object(image).unwrap().x(), 0.0);
}
