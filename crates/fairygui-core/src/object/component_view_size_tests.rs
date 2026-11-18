use super::*;
use crate::{
    geom::Margin,
    object::SizeConstraints,
    package::ObjectType,
    scroll::ScrollPane,
    widget::{ControlData, ListLayoutType},
};

#[test]
fn view_axis_setters_match_594_native_snapshots_with_margins_bars_and_size_limits() {
    let (mut cases, mut snapshots) = (0, 0);
    let mut current = None;
    for line in include_str!("../../tests/fixtures/component-view-size-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        if words[0] == "case" {
            let v: Vec<f32> = words[1..].iter().map(|s| s.parse().unwrap()).collect();
            let mut store = ObjectStore::new();
            let id = store.create(ObjectType::Component);
            if v[7] == 1. {
                store.set_size_constraints(
                    id,
                    SizeConstraints {
                        min: Size::new(30., 25.),
                        max: Size::new(140., 100.),
                    },
                );
            }
            let component = store.objects[id].control.component_mut().unwrap();
            component.margin = Margin::new(v[3], v[4], v[5], v[6]);
            if v[0] == 1. {
                let mut pane = ScrollPane::default();
                pane.floating = v[2] == 1.;
                pane.set_content_size(Size::new(400., 300.));
                component.scroll_pane = Some(pane);
                let bars = v[1] as u8;
                for (bit, size) in [(1, Size::new(8., 60.)), (2, Size::new(80., 6.))] {
                    if bars & bit == 0 {
                        continue;
                    }
                    let bar = store.create(ObjectType::ScrollBar);
                    store.set_size(bar, size.width, size.height);
                    store.set_parent(bar, Some(id));
                    let p = store.objects[id]
                        .control
                        .component_mut()
                        .unwrap()
                        .scroll_pane
                        .as_mut()
                        .unwrap();
                    if bit == 1 {
                        p.vt_bar = Some(bar);
                    } else {
                        p.hz_bar = Some(bar);
                    }
                }
            }
            store.set_size(id, 120., 80.);
            store.layout_scroll_bars(id);
            current = Some((store, id));
            cases += 1;
            continue;
        }
        let (store, id) = current.as_mut().unwrap();
        let id = *id;
        let (command, expected) = line.split_once(':').unwrap();
        let command: Vec<_> = command.split_whitespace().collect();
        let value: f32 = command[1].parse().unwrap();
        match command[0] {
            "initial" => (),
            "width" => {
                store.set_component_view_width(id, value);
            }
            "height" => {
                store.set_component_view_height(id, value);
            }
            _ => panic!("{line}"),
        }
        let object = store.object(id).unwrap();
        let view = store.component_view_size(id).unwrap();
        let requested = object.requested_size();
        let actual = [
            object.width(),
            object.height(),
            view.width,
            view.height,
            requested.width,
            requested.height,
        ];
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(actual.as_slice(), expected, "case {cases}: {line}");
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (54, 594));
}

#[test]
fn view_axis_changes_preserve_other_requested_axis_and_reflow_tree_rows_without_replaying_requests()
{
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    store.set_size(tree, 400., 300.);
    store.set_size_constraints(
        tree,
        SizeConstraints {
            min: Size::ZERO,
            max: Size::new(250., 200.),
        },
    );
    store.set_component_margin(tree, Margin::new(7., 11., 13., 19.));
    let row = store.create(ObjectType::Button);
    store.set_size(row, 20., 25.);
    store.set_parent(row, Some(tree));
    store.list_data_mut(tree).unwrap().auto_resize_item = true;
    store.set_component_view_width(tree, 100.);
    assert_eq!(
        store.object(tree).unwrap().requested_size(),
        Size::new(120., 300.)
    );
    store.flush_list_layouts();
    assert_eq!(store.object(row).unwrap().width(), 100.);
    assert_eq!(store.component_view_size(tree), Some(Size::new(100., 170.)));
    store.set_size_constraints(tree, SizeConstraints::default());
    assert_eq!(store.component_view_size(tree), Some(Size::new(100., 270.)));
    assert!(!store.set_component_view_size(tree, Size::new(f32::NAN, f32::INFINITY)));
    assert!(store.set_component_view_size(tree, Size::new(f32::NAN, 80.)));
    assert_eq!(
        store.object(tree).unwrap().requested_size(),
        Size::new(120., 110.)
    );
    assert_eq!(store.component_view_size(row), Some(Size::new(100., 25.)));
    let graph = store.create(ObjectType::Graph);
    assert_eq!(store.component_view_size(graph), None);
    assert!(!store.set_component_view_width(graph, 50.));
}

#[test]
fn virtual_flow_refills_after_resizing_the_view_without_recreating_the_pane() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 120., 80.);
    let ControlData::List(data) = &mut store.objects[list].control else {
        unreachable!()
    };
    data.layout = ListLayoutType::FlowHorizontal;
    data.auto_resize_item = false;
    data.component.scroll_pane = Some(ScrollPane::default());
    store.layout_scroll_bars(list);
    let mut create = |s: &mut ObjectStore, _: &str| {
        let row = s.create(ObjectType::Button);
        s.set_size(row, 20., 20.);
        Some(row)
    };
    store.enable_virtual_list(list, "row", &mut create).unwrap();
    store.set_virtual_item_count(list, 100).unwrap();
    store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {})
        .unwrap();
    assert_eq!(store.virtual_states[&list].geometry.line_items, 6);
    let first = store.list_item_at(list, 0).unwrap();
    store.set_component_margin(list, Margin::all(10.));
    store.set_component_view_size(list, Size::new(180., 100.));
    store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {})
        .unwrap();
    assert_eq!(store.virtual_states[&list].geometry.line_items, 9);
    assert_eq!(store.component_view_size(list), Some(Size::new(180., 100.)));
    assert_eq!(store.object(list).unwrap().size(), Size::new(200., 120.));
    assert_eq!(store.list_item_at(list, 0), Some(first));
}
