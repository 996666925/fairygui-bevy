use super::*;
use crate::{
    package::ObjectType,
    widget::{ButtonMode, Controller},
};

fn fixture(
    mode: ListSelectionMode,
    mixed: bool,
) -> (ObjectStore, GObjectId, GObjectId, Vec<GObjectId>) {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::Component);
    store.objects[parent]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "selection".into(),
            selected_index: 0,
            page_ids: (0..8).map(|i| i.to_string()).collect(),
            page_names: (0..8).map(|i| format!("page{i}")).collect(),
            ..Default::default()
        });
    let list = store.create(ObjectType::List);
    store.set_parent(list, Some(parent));
    store.set_list_selection_mode(list, mode);
    store.set_list_selection_controller(list, Some("selection"));
    let rows = (0..8)
        .map(|i| {
            let row = store.create(if mixed && i == 6 {
                ObjectType::Component
            } else {
                ObjectType::Button
            });
            if let ControlData::Button(data) = &mut store.objects[row].control
                && !(mixed && i == 2)
            {
                data.mode = ButtonMode::Check;
            }
            store.set_parent(row, Some(list));
            row
        })
        .collect();
    (store, parent, list, rows)
}

#[test]
fn materialized_selection_matches_184_native_traces_with_and_without_pane_bars() {
    for overlays in [false, true] {
        let mut current = None;
        let mut state = None;
        let mut count = 0;
        for line in include_str!("../../tests/fixtures/list-selection-reference.txt")
            .lines()
            .filter(|s| !s.starts_with('#'))
        {
            let v: Vec<_> = line.split_whitespace().collect();
            let mode: i8 = v[1].parse().unwrap();
            let key = (v[0], mode);
            if current != Some(key) {
                state = Some(fixture(ListSelectionMode::from_code(mode), v[0] == "mixed"));
                if overlays {
                    let (store, _, list, _) = state.as_mut().unwrap();
                    let bars = super::super::list_items::tests::bars(store, *list);
                    store.add_child_at(*list, bars[0], 0);
                    store.add_child_at(*list, bars[1], 4);
                }
                current = Some(key);
            }
            let (store, parent, list, rows) = state.as_mut().unwrap();
            let index: i32 = v[3].parse().unwrap();
            match v[2] {
                "click" => {
                    store.select_clicked_item_with_modifiers(
                        rows[index as usize],
                        v[4] == "1",
                        v[5] == "1",
                    );
                }
                "set" => {
                    store.set_list_selection(*list, index);
                }
                "add" => {
                    store.add_list_selection(*list, index as usize);
                }
                "remove" => {
                    store.remove_list_selection(*list, index as usize);
                }
                "clear" => {
                    store.clear_list_selection(*list);
                }
                "all" => {
                    store.select_all_list_items(*list);
                }
                "reverse" => {
                    store.reverse_list_selection(*list);
                }
                _ => panic!("operation"),
            }
            let expected: Vec<usize> = if v[6] == "_" {
                vec![]
            } else {
                v[6].split(',').map(|i| i.parse().unwrap()).collect()
            };
            assert_eq!(store.list_selections(*list).unwrap(), expected, "{line}");
            assert_eq!(
                store.list_selection(*list),
                Some(expected.first().map_or(-1, |i| *i as i32)),
                "{line}"
            );
            let data = store.list_data(*list).unwrap();
            assert_eq!(data.selected_indices, expected, "snapshot {line}");
            assert_eq!(
                data.last_selected_index.map_or(-1, |i| i as i32),
                v[7].parse::<i32>().unwrap(),
                "anchor {line}"
            );
            assert_eq!(
                store.objects[*parent]
                    .control
                    .component()
                    .unwrap()
                    .controllers[0]
                    .selected_index,
                v[8].parse::<i32>().unwrap(),
                "controller {line}"
            );
            count += 1;
        }
        assert_eq!(count, 184);
    }
}

#[test]
fn row_selection_follows_reparent_removal_reordering_and_external_button_setters() {
    let (mut store, parent, list, rows) = fixture(ListSelectionMode::Multiple, false);
    assert!(
        !store.click_button(rows[2]),
        "list disables the button's own toggle"
    );
    store.add_list_selection(list, 2);
    store.add_list_selection(list, 5);
    store.add_child_at(list, rows[5], 0);
    assert_eq!(store.list_selections(list), Some(vec![0, 3]));
    store.set_button_selected(rows[1], true);
    assert_eq!(
        store.list_data(list).unwrap().selected_indices,
        vec![0, 2, 3]
    );
    store.remove(rows[5]);
    assert_eq!(store.list_selections(list), Some(vec![1, 2]));
    store.set_parent(rows[2], Some(parent));
    assert_eq!(store.list_selections(list), Some(vec![1]));
    store.set_parent(rows[2], Some(list));
    assert_eq!(
        store.button_selected(rows[2]),
        Some(false),
        "entering a list resets the row"
    );
    let child = store.create(ObjectType::Text);
    store.set_parent(child, Some(rows[4]));
    store.select_clicked_item_with_modifiers(child, false, true);
    assert_eq!(store.list_item_of(child).unwrap().1, rows[4]);
    assert!(
        store
            .list_selections(list)
            .unwrap()
            .contains(&store.child_index(rows[4]).unwrap())
    );
}

#[test]
fn selection_controller_keeps_its_identity_across_reparenting_and_explicit_same_name_rebinding() {
    let (mut store, original, list, _) = fixture(ListSelectionMode::Multiple, false);
    let other = store.create(ObjectType::Component);
    let controller = store
        .object(original)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controllers[0]
        .clone();
    store.objects[other]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller);
    store.set_list_selection(list, 3);
    store.set_parent(list, Some(other));
    store.select_clicked_item_with_modifiers(store.list_item_at(list, 4).unwrap(), false, true);
    let page = |store: &ObjectStore, owner| {
        store
            .object(owner)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("selection")
            .unwrap()
            .selected_index
    };
    assert_eq!(page(&store, original), 4);
    assert_eq!(
        page(&store, other),
        0,
        "a same-name controller in the new parent is unrelated"
    );
    assert_eq!(store.list_selections(list), Some(vec![3, 4]));
    store.set_controller_page_index(other, "selection", 7);
    assert_eq!(store.list_selections(list), Some(vec![3, 4]));
    store.set_controller_page_index(original, "selection", 1);
    assert_eq!(
        store.list_selections(list),
        Some(vec![3, 4]),
        "native controller changes only traverse current children"
    );
    assert!(store.set_list_selection_controller(list, Some("selection")));
    assert_eq!(
        store.list_selections(list),
        Some(vec![3, 4]),
        "assigning a new reference must not reselect"
    );
    store.set_list_selection(list, 6);
    assert_eq!(page(&store, original), 1);
    assert_eq!(page(&store, other), 6);
}

#[test]
fn disposed_selection_controller_owner_never_retargets_a_new_parent_or_reused_slot() {
    let (mut store, original, list, _) = fixture(ListSelectionMode::Single, false);
    let other = store.create(ObjectType::Component);
    let controller = store
        .object(original)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controllers[0]
        .clone();
    store.objects[other]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller.clone());
    store.set_parent(list, Some(other));
    store.remove(original);
    let replacement = store.create(ObjectType::Component);
    store.objects[replacement]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(controller);
    assert_eq!(store.list_selection_controller(list), Some(None));
    let mut settings = store.list_interaction_settings(list).unwrap();
    settings.fold_invisible_items = true;
    store.set_list_interaction_settings(list, settings);
    store.set_list_selection(list, 4);
    for owner in [other, replacement] {
        assert_eq!(
            store
                .object(owner)
                .unwrap()
                .control()
                .component()
                .unwrap()
                .controllers[0]
                .selected_index,
            0
        );
    }
    assert!(store.set_list_selection_controller(list, Some("selection")));
    store.set_list_selection(list, 6);
    assert_eq!(
        store
            .object(other)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controllers[0]
            .selected_index,
        6
    );
    store.remove(list);
    assert!(store.selection_controllers.is_empty());
}

#[test]
fn selection_controller_updates_both_ways_and_does_not_collapse_originating_multiselection() {
    let (mut store, parent, list, _) = fixture(ListSelectionMode::Multiple, false);
    store.add_list_selection(list, 2);
    store.add_list_selection(list, 5);
    assert_eq!(store.list_selections(list), Some(vec![2, 5]));
    assert!(store.set_controller_page_index(parent, "selection", 4));
    assert_eq!(store.list_selections(list), Some(vec![4]));
    assert!(store.set_controller_page(parent, "selection", "page1"));
    assert_eq!(store.list_selections(list), Some(vec![1]));
    store.clear_list_selection(list);
    assert_eq!(
        store.objects[parent]
            .control
            .component()
            .unwrap()
            .controllers[0]
            .selected_index,
        1,
        "clear selection leaves controller unchanged"
    );
    store.apply_all_controllers(parent);
    assert_eq!(store.list_selections(list), Some(vec![1]));
    store.clear_list_selection(list);
    store.initialize_list_selections(parent);
    assert_eq!(
        store.list_selections(list),
        Some(vec![1]),
        "late materialization restores controller selection"
    );
}
