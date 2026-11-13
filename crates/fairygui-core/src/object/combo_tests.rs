use super::*;
use crate::widget::{ComboBoxData, Controller};

fn data(store: &ObjectStore, combo: GObjectId) -> &ComboBoxData {
    let ControlData::ComboBox(data) = store.object(combo).unwrap().control() else {
        panic!("combo")
    };
    data
}
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).into()).collect()
}

#[test]
fn combo_defaults_items_values_and_requested_indices_follow_native_setters() {
    let mut store = ObjectStore::new();
    let combo = store.create(ObjectType::ComboBox);
    assert_eq!(
        (
            data(&store, combo).selected_index,
            data(&store, combo).visible_item_count,
            data(&store, combo).selection_controller
        ),
        (-1, 10, -1)
    );
    store.set_combo_items(combo, strings(&["One", "Two", "Three"]));
    store.set_combo_values(combo, strings(&["", "two"]));
    assert_eq!(data(&store, combo).title, "One");
    assert_eq!(data(&store, combo).selected_value(), Some(""));
    store.select_combo_value(combo, Some("two"));
    assert_eq!(data(&store, combo).selected_index, 1);
    store.select_combo_value(combo, Some("missing"));
    assert_eq!(
        (
            data(&store, combo).selected_index,
            data(&store, combo).title.as_str()
        ),
        (-1, "")
    );
    store.select_combo_value(combo, None);
    assert_eq!(data(&store, combo).selected_index, 0);
    store.select_combo_index(combo, 15);
    assert_eq!(data(&store, combo).selected_index, 15);
    assert_eq!(data(&store, combo).selected_value(), None);
    store.set_combo_items(combo, strings(&["A", "B"]));
    assert_eq!(
        (
            data(&store, combo).selected_index,
            data(&store, combo).title.as_str()
        ),
        (1, "B")
    );
    store.set_combo_items(combo, Vec::new());
    assert_eq!(
        (
            data(&store, combo).selected_index,
            data(&store, combo).title.as_str()
        ),
        (-1, "")
    );
}

#[test]
fn native_null_empty_and_short_icon_arrays_preserve_or_clear_the_displayed_icon() {
    let mut store = ObjectStore::new();
    let combo = store.create(ObjectType::ComboBox);
    store.set_combo_items(combo, strings(&["A", "B", "C"]));
    store.apply_icon_to(combo, "custom");
    store.select_combo_index(combo, 1);
    assert_eq!(data(&store, combo).icon.as_deref(), Some("custom"));
    store.set_combo_icons(combo, Some(vec![Some("first".into())]));
    assert_eq!(data(&store, combo).icon.as_deref(), Some("custom"));
    store.select_combo_index(combo, 0);
    assert_eq!(data(&store, combo).icon.as_deref(), Some("first"));
    store.select_combo_index(combo, 1);
    assert_eq!(data(&store, combo).icon.as_deref(), Some("first"));
    store.set_combo_icons(combo, None);
    store.select_combo_index(combo, -1);
    assert_eq!(data(&store, combo).icon.as_deref(), Some("first"));
    store.set_combo_icons(combo, Some(Vec::new()));
    store.select_combo_index(combo, 99);
    assert_eq!(data(&store, combo).icon, None);
    store.select_combo_index(combo, 0);
    store.set_combo_icons(combo, Some(vec![Some("first".into()), None]));
    store.select_combo_index(combo, 1);
    assert_eq!(data(&store, combo).icon, None);
}

#[test]
fn combo_selection_controller_is_bidirectional_without_rewriting_the_source_or_reentering() {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    store.objects[parent]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "selection".into(),
            selected_index: 2,
            page_ids: strings(&["0", "1", "2"]),
            page_names: strings(&["A", "B", "C"]),
            ..Controller::default()
        });
    let combos: Vec<_> = (0..2)
        .map(|_| {
            let combo = store.create(ObjectType::ComboBox);
            store.set_parent(combo, Some(parent));
            store.set_combo_items(combo, strings(&["A", "B", "C", "Extra"]));
            store.set_combo_selection_controller(combo, Some("selection"));
            combo
        })
        .collect();
    assert_eq!(
        data(&store, combos[0]).selected_index,
        0,
        "binding itself does not select"
    );
    store.apply_all_controllers(parent);
    assert_eq!(data(&store, combos[0]).selected_index, 2);
    store.select_combo_index(combos[0], 1);
    assert_eq!(data(&store, combos[1]).selected_index, 1);
    store.set_object_text(combos[0], "Custom title");
    store.select_combo_index_from_input(combos[0], 1);
    assert_eq!(data(&store, combos[0]).title, "B");
    assert_eq!(
        store
            .object(parent)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controllers[0]
            .previous_index,
        2,
        "reselect must not inject a temporary -1 controller transition"
    );
    store.set_controller_page_id(parent, "selection", "0");
    assert_eq!(data(&store, combos[0]).selected_index, 0);
    store.select_combo_index(combos[0], 3);
    assert_eq!(data(&store, combos[0]).selected_index, 3);
    assert_eq!(
        data(&store, combos[1]).selected_index,
        0,
        "index beyond the controller does not change its page"
    );
    store.select_combo_index(combos[0], -2);
    assert_eq!(
        data(&store, combos[0]).selected_index,
        -2,
        "source combo is detached during controller updates"
    );
    assert_eq!(data(&store, combos[1]).selected_index, -1);
    store.set_combo_items(combos[0], strings(&["Only"]));
    assert_eq!(
        data(&store, combos[1]).selected_index,
        -1,
        "items setter never updates the controller"
    );
    store.set_combo_selection_controller(combos[0], None);
    store.set_controller_page(parent, "selection", "B");
    assert_eq!(data(&store, combos[0]).selected_index, -2);
    assert_eq!(data(&store, combos[1]).selected_index, 1);
    assert!(!store.set_combo_selection_controller(combos[1], Some("missing")));
    store.remove(parent);
    assert!(!store.select_combo_value(combos[0], Some("anything")));
}
