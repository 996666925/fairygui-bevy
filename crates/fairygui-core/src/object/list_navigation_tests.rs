use super::*;
use crate::{
    package::ObjectType,
    widget::{ButtonMode, Controller},
};

#[test]
fn navigation_matches_1600_native_cases_and_ignores_scroll_overlays() {
    for overlays in [false, true] {
        let mut count = 0;
        for line in include_str!("../../tests/fixtures/list-navigation-reference.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let v: Vec<_> = line.split_whitespace().collect();
            let layout = ListLayoutType::from_code(v[0].parse().unwrap());
            let mode = ListSelectionMode::from_code(v[1].parse().unwrap());
            let mixed = v[2] == "1";
            let start: i32 = v[3].parse().unwrap();
            let direction = match v[4] {
                "1" => ListNavigationDirection::Up,
                "3" => ListNavigationDirection::Right,
                "5" => ListNavigationDirection::Down,
                "7" => ListNavigationDirection::Left,
                _ => unreachable!(),
            };
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
                    page_ids: (0..9).map(|i| i.to_string()).collect(),
                    page_names: (0..9).map(|i| i.to_string()).collect(),
                    ..Default::default()
                });
            let list = store.create(ObjectType::List);
            store.set_parent(list, Some(parent));
            store.list_data_mut(list).unwrap().layout = layout;
            store.set_list_selection_mode(list, mode);
            store.set_list_selection_controller(list, Some("selection"));
            let rows: Vec<_> = (0..9)
                .map(|i| {
                    let row = store.create(if mixed && i == 6 {
                        ObjectType::Component
                    } else {
                        ObjectType::Button
                    });
                    if let ControlData::Button(data) = &mut store.objects[row].control {
                        data.mode = if mixed && i == 2 {
                            ButtonMode::Common
                        } else {
                            ButtonMode::Check
                        };
                    }
                    store.set_parent(row, Some(list));
                    row
                })
                .collect();
            if start >= 0 {
                store.set_button_selected(rows[start as usize], true);
                if mixed {
                    store.set_button_selected(rows[7], true);
                }
            }
            if overlays {
                let bars = super::super::list_items::tests::bars(&mut store, list);
                store.add_child_at(list, bars[0], 0);
                store.add_child_at(list, bars[1], 4);
            }
            // Installing pane bars may reflow the list. Restore the native
            // host's coordinates after the complete ownership setup.
            for (i, &row) in rows.iter().enumerate() {
                let (column, line) = if layout == ListLayoutType::FlowVertical {
                    (i / 3, i % 3)
                } else {
                    (i % 3, i / 3)
                };
                store.set_xy(row, column as f32 * 40., line as f32 * 25.);
            }
            let before = store.list_selections(list).unwrap();
            let changed = store.navigate_list_selection(list, direction);
            let expected: Vec<usize> = if v[5] == "_" {
                vec![]
            } else {
                v[5].split(',').map(|s| s.parse().unwrap()).collect()
            };
            assert_eq!(
                store.list_selections(list),
                Some(expected.clone()),
                "{line}; overlays={overlays}"
            );
            assert_eq!(changed, before != expected, "{line}");
            let data = store.list_data(list).unwrap();
            assert_eq!(data.selected_indices, expected, "snapshot {line}");
            assert_eq!(
                data.last_selected_index.map_or(-1, |i| i as i32),
                v[6].parse::<i32>().unwrap(),
                "anchor {line}"
            );
            assert_eq!(
                store
                    .object(parent)
                    .unwrap()
                    .control()
                    .component()
                    .unwrap()
                    .controllers[0]
                    .selected_index,
                v[7].parse::<i32>().unwrap(),
                "controller {line}"
            );
            count += 1;
        }
        assert_eq!(count, 1600);
    }
}
