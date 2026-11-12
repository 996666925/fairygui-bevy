use super::*;
use crate::widget::{Controller, Gear, GearKind, GearTweenConfig, GearValue};

fn xy(x: f32, y: f32) -> GearValue {
    GearValue::Xy {
        x,
        y,
        px: 0.0,
        py: 0.0,
    }
}

fn fixture(
    kind: GearKind,
    values: [GearValue; 3],
    delay: f32,
) -> (ObjectStore, GObjectId, GObjectId) {
    let mut store = ObjectStore::new();
    let owner = store.create(ObjectType::Component);
    let child = store.create(ObjectType::Image);
    store.set_parent(child, Some(owner));
    store
        .object_mut(owner)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "state".into(),
            page_ids: vec!["a".into(), "b".into(), "c".into()],
            page_names: vec!["A".into(), "B".into(), "C".into()],
            selected_index: 0,
            ..Default::default()
        });
    let mut gear = Gear::new(kind);
    gear.controller_index = 0;
    gear.tween = Some(GearTweenConfig {
        duration: 1.0,
        ease_type: 0,
        delay,
    });
    for (page, value) in ["a", "b", "c"].into_iter().zip(values) {
        gear.values.insert(Some(page.into()), value);
    }
    store.object_mut(child).unwrap().gears.push(gear);
    store.initialize_controllers(owner);
    store.take_dirty();
    (store, owner, child)
}

#[test]
fn coordinate_tween_waits_then_uses_only_the_remainder_of_the_frame() {
    let (mut store, owner, child) = fixture(
        GearKind::Xy,
        [xy(0.0, 0.0), xy(100.0, 50.0), xy(200.0, 100.0)],
        0.5,
    );
    store.set_controller_page(owner, "state", "B");
    assert_eq!(store.object(child).unwrap().position(), Point::ZERO);
    store.advance_gear_tweens(0.25);
    assert_eq!(store.object(child).unwrap().position(), Point::ZERO);
    store.advance_gear_tweens(0.5);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(25.0, 12.5)
    );
    assert!(
        store
            .take_dirty()
            .iter()
            .any(|&(id, flags)| id == child && flags & dirty::XY != 0)
    );
    store.advance_gear_tweens(1.0);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(100.0, 50.0)
    );
    assert!(!store.has_gear_tweens());
}

#[test]
fn another_page_with_the_same_destination_keeps_the_running_tween() {
    let (mut store, owner, child) = fixture(
        GearKind::Xy,
        [xy(0.0, 0.0), xy(100.0, 0.0), xy(100.0, 0.0)],
        0.0,
    );
    store.set_controller_page(owner, "state", "B");
    store.advance_gear_tweens(0.25);
    store.set_controller_page(owner, "state", "C");
    store.advance_gear_tweens(0.25);
    assert_eq!(store.object(child).unwrap().x(), 50.0);
    store.advance_gear_tweens(0.5);
    assert_eq!(store.object(child).unwrap().x(), 100.0);
    assert!(!store.has_gear_tweens());
    store.set_controller_page(owner, "state", "B");
    assert!(!store.has_gear_tweens(), "already at destination");
}

#[test]
fn a_new_destination_finishes_the_previous_tween_before_starting_another() {
    let (mut store, owner, child) = fixture(
        GearKind::Xy,
        [xy(0.0, 0.0), xy(100.0, 0.0), xy(200.0, 0.0)],
        0.0,
    );
    store.set_controller_page(owner, "state", "B");
    store.advance_gear_tweens(0.25);
    assert_eq!(store.object(child).unwrap().x(), 25.0);
    store.set_controller_page(owner, "state", "C");
    assert_eq!(store.object(child).unwrap().x(), 100.0);
    store.advance_gear_tweens(0.25);
    assert_eq!(store.object(child).unwrap().x(), 125.0);
}

#[test]
fn look_flags_apply_immediately_while_alpha_and_rotation_tween() {
    let initial = GearValue::Look {
        alpha: 1.0,
        rotation: 0.0,
        grayed: false,
        touchable: true,
    };
    let target = GearValue::Look {
        alpha: 0.0,
        rotation: 100.0,
        grayed: true,
        touchable: false,
    };
    let same_numeric_target = GearValue::Look {
        alpha: 0.0,
        rotation: 100.0,
        grayed: false,
        touchable: true,
    };
    let (mut store, owner, child) =
        fixture(GearKind::Look, [initial, target, same_numeric_target], 0.5);
    store.set_controller_page(owner, "state", "B");
    let object = store.object(child).unwrap();
    assert!(object.grayed());
    assert!(!object.touchable());
    assert_eq!(object.alpha(), 1.0);
    store.advance_gear_tweens(0.75);
    let object = store.object(child).unwrap();
    assert!(object.grayed());
    assert!(!object.touchable());
    assert_eq!(object.alpha(), 0.75);
    assert_eq!(object.rotation(), 25.0);
    store.set_controller_page(owner, "state", "C");
    assert!(!store.object(child).unwrap().grayed());
    assert!(store.object(child).unwrap().touchable());
    store.advance_gear_tweens(0.25);
    let object = store.object(child).unwrap();
    assert_eq!(
        object.alpha(),
        0.5,
        "flag change must not restart numeric interpolation"
    );
    assert_eq!(object.rotation(), 50.0);
    assert!(!object.grayed());
    assert!(object.touchable());
}

#[test]
fn size_and_scale_interpolate_together() {
    let initial = GearValue::Size {
        width: 100.0,
        height: 50.0,
        scale_x: 1.0,
        scale_y: 1.0,
    };
    let target = GearValue::Size {
        width: 200.0,
        height: 100.0,
        scale_x: 2.0,
        scale_y: 3.0,
    };
    let (mut store, owner, child) =
        fixture(GearKind::Size, [initial.clone(), target, initial], 0.0);
    store.set_controller_page(owner, "state", "B");
    store.advance_gear_tweens(0.5);
    let object = store.object(child).unwrap();
    assert_eq!(object.size(), Size::new(150.0, 75.0));
    assert_eq!((object.scale_x(), object.scale_y()), (1.5, 2.0));
}

#[test]
fn percentage_coordinates_are_resolved_against_the_parent_before_tweening() {
    let (mut store, owner, child) = fixture(
        GearKind::Xy,
        [xy(0.0, 0.0), xy(999.0, 999.0), xy(0.0, 0.0)],
        0.0,
    );
    store.set_size(owner, 200.0, 100.0);
    let gear = &mut store.object_mut(child).unwrap().gears[0];
    gear.positions_in_percent = true;
    gear.values.insert(
        Some("b".into()),
        GearValue::Xy {
            x: 999.0,
            y: 999.0,
            px: 0.5,
            py: 0.5,
        },
    );
    store.set_controller_page(owner, "state", "B");
    store.advance_gear_tweens(0.5);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(50.0, 25.0)
    );
    store.advance_gear_tweens(0.5);
    assert_eq!(
        store.object(child).unwrap().position(),
        Point::new(100.0, 50.0)
    );
}

#[test]
fn removing_a_subtree_cancels_its_pending_gear_tweens() {
    let (mut store, owner, _) = fixture(
        GearKind::Xy,
        [xy(0.0, 0.0), xy(100.0, 0.0), xy(0.0, 0.0)],
        1.0,
    );
    store.set_controller_page(owner, "state", "B");
    assert!(store.has_gear_tweens());
    store.remove(owner);
    assert!(!store.has_gear_tweens());
    store.advance_gear_tweens(2.0);
}
