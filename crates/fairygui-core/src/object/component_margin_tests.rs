use super::*;
use crate::{
    geom::Size,
    package::ObjectType,
    scroll::ScrollPane,
    widget::{ControlData, ListLayoutType},
};

#[test]
fn component_margin_origins_and_views_match_840_native_snapshots() {
    let mut current = None;
    let (mut cases, mut snapshots) = (0, 0);
    for line in include_str!("../../tests/fixtures/component-margin-reference.txt").lines() {
        if line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        if words[0] == "case" {
            let a: Vec<usize> = words[1..].iter().map(|s| s.parse().unwrap()).collect();
            let mut store = ObjectStore::new();
            let id = store.create(if a[1] == 1 {
                ObjectType::List
            } else {
                ObjectType::Component
            });
            store.set_size(id, 100., 80.);
            let component = store.objects[id].control.component_mut().unwrap();
            component.margin = Margin::new(a[7] as f32, a[8] as f32, a[9] as f32, a[10] as f32);
            component.overflow = OverflowType::from_code(a[2] as i8);
            if a[2] == 2 {
                let mut p = ScrollPane::default();
                p.display_on_left = a[4] == 1;
                p.floating = a[5] == 1;
                p.dont_clip_margin = a[6] == 1;
                p.scroll_bar_margin = Margin::new(2., 4., 6., 8.);
                component.scroll_pane = Some(p);
                let vt = store.create(ObjectType::ScrollBar);
                let hz = store.create(ObjectType::ScrollBar);
                store.set_size(vt, 8., 60.);
                store.set_size(hz, 80., 6.);
                store.set_parent(vt, Some(id));
                store.set_parent(hz, Some(id));
                let p = store.objects[id]
                    .control
                    .component_mut()
                    .unwrap()
                    .scroll_pane
                    .as_mut()
                    .unwrap();
                p.vt_bar = Some(vt);
                p.hz_bar = Some(hz);
                store.layout_scroll_bars(id);
            }
            if a[1] == 1 {
                let margin = store.component_margin(id).unwrap();
                let d = store.list_data_mut(id).unwrap();
                d.item_margin = margin;
                d.align = a[3] as i32;
                d.vertical_align = a[3] as i32;
                d.auto_resize_item = false;
                let child = store.create(ObjectType::Graph);
                store.set_size(child, 50., 20.);
                store.set_parent(child, Some(id));
                store.layout_list(id);
            }
            current = Some((store, id));
            cases += 1;
            continue;
        }
        let (store, id) = current.as_mut().unwrap();
        let id = *id;
        let (command, expected) = line.split_once(':').unwrap();
        let command: Vec<_> = command.split_whitespace().collect();
        match command[0] {
            "initial" => (),
            "width" => {
                store.set_size(id, command[1].parse().unwrap(), 80.);
                store.layout_scroll_bars(id);
            }
            "resize" => {
                store.set_size(id, command[1].parse().unwrap(), 80.);
                store.layout_scroll_bars(id);
                store.layout_list(id);
            }
            "margin" => {
                let v: Vec<f32> = command[1..].iter().map(|s| s.parse().unwrap()).collect();
                store.set_component_margin(id, Margin::new(v[0], v[1], v[2], v[3]));
            }
            "layout" => {
                store.layout_list(id);
            }
            _ => panic!("{line}"),
        }
        let m = store.component_margin(id).unwrap();
        let o = store.object(id).unwrap();
        let view = store.scroll_pane(id).map_or(
            Size::new(o.width() - m.horizontal(), o.height() - m.vertical()),
            |p| p.view_size(),
        );
        let origin = store.component_content_origin(id).unwrap();
        let actual = [origin.x, origin.y, view.width, view.height];
        let expected: Vec<f32> = expected
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(actual.as_slice(), expected, "case {cases}: {line}");
        if let Some(child) = store.content_children(id).unwrap().first() {
            assert_eq!(store.local_to_global(*child, Point::ZERO), Some(origin));
            assert_eq!(store.global_to_local(*child, origin), Some(Point::ZERO));
        }
        snapshots += 1;
    }
    assert_eq!((cases, snapshots), (120, 840));
}

#[test]
fn margin_edits_reflow_tree_rows_and_reject_nonfinite_values_without_resizing() {
    let mut store = ObjectStore::new();
    let tree = store.create(ObjectType::Tree);
    store.set_size(tree, 100., 80.);
    let child = store.create(ObjectType::Graph);
    store.set_size(child, 20., 30.);
    store.set_parent(child, Some(tree));
    store.list_data_mut(tree).unwrap().auto_resize_item = true;
    store.flush_list_layouts();
    store.take_dirty();
    assert!(store.set_component_margin(tree, Margin::new(7., 4., 13., 8.)));
    store.flush_list_layouts();
    assert_eq!(store.object(child).unwrap().width(), 80.);
    assert_eq!(store.object(tree).unwrap().size(), Size::new(100., 80.));
    assert_eq!(
        store.list_data(tree).unwrap().item_margin,
        store.component_margin(tree).unwrap()
    );
    store.take_dirty();
    for m in [Margin::all(f32::NAN), Margin::all(f32::INFINITY)] {
        assert!(!store.set_component_margin(tree, m));
    }
    assert!(!store.set_component_margin(child, Margin::all(1.)));
    assert!(store.take_dirty().is_empty());
    store.remove(tree);
    assert!(store.list_alignments.is_empty());
    assert!(store.visible_content_origins.is_empty());
}

#[test]
fn margins_resize_virtual_flow_views_and_keep_scrollbars_in_owner_coordinates() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 120., 80.);
    let ControlData::List(d) = &mut store.objects[list].control else {
        unreachable!()
    };
    d.layout = ListLayoutType::FlowHorizontal;
    d.auto_resize_item = false;
    d.component.scroll_pane = Some(ScrollPane::default());
    store.layout_scroll_bars(list);
    let mut create = |s: &mut ObjectStore, _: &str| {
        let id = s.create(ObjectType::Button);
        s.set_size(id, 20., 20.);
        Some(id)
    };
    store.enable_virtual_list(list, "row", &mut create).unwrap();
    store.set_virtual_item_count(list, 40).unwrap();
    store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {})
        .unwrap();
    assert_eq!(store.virtual_states[&list].geometry.line_items, 6);
    store.set_component_margin(list, Margin::all(10.));
    store
        .refresh_virtual_list(list, &mut create, &mut |_| None, &mut |_, _, _| {})
        .unwrap();
    assert_eq!(store.virtual_states[&list].geometry.line_items, 5);
    assert_eq!(
        store.scroll_pane(list).unwrap().view_size(),
        Size::new(100., 60.)
    );
    let bar = store.create(ObjectType::ScrollBar);
    store.set_size(bar, 8., 50.);
    store.set_parent(bar, Some(list));
    store.objects[list]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane
        .as_mut()
        .unwrap()
        .vt_bar = Some(bar);
    store.layout_scroll_bars(list);
    store.set_scroll_pos(list, 0., 20., false);
    assert_eq!(
        store.local_to_global(bar, Point::ZERO),
        Some(Point::new(112., 0.))
    );
    let row = store.list_item_at(list, 0).unwrap();
    let row_pos = store.object(row).unwrap().position();
    assert_eq!(
        store.local_to_global(row, Point::ZERO),
        Some(Point::new(row_pos.x + 10., row_pos.y - 10.))
    );
}
