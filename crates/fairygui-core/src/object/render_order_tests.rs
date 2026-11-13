use super::*;
use crate::{
    scroll::ScrollPane,
    widget::{ChildrenRenderOrder, ControlData},
};

#[test]
fn component_paint_order_matches_native_modes_apices_hidden_and_nondisplay_children() {
    let mut snapshots = 0;
    for row in include_str!("../../tests/fixtures/render-order-reference.txt")
        .lines()
        .filter(|row| !row.starts_with('#'))
    {
        let f: Vec<_> = row.split('|').collect();
        let count: usize = f[0].parse().unwrap();
        let order = ChildrenRenderOrder::from_code(f[1].parse().unwrap());
        let apex = f[2].parse().unwrap();
        let hidden: usize = f[3].parse().unwrap();
        let absent: usize = f[4].parse().unwrap();
        let mut store = ObjectStore::new();
        let parent = store.create(ObjectType::Component);
        let children: Vec<_> = (0..count)
            .map(|i| {
                let child = store.create(if absent & (1 << i) != 0 {
                    ObjectType::Group
                } else {
                    ObjectType::Component
                });
                store.set_parent(child, Some(parent));
                store.set_visible(child, hidden & (1 << i) == 0);
                child
            })
            .collect();
        store.set_children_render_order(parent, order);
        store.set_apex_index(parent, apex);
        let mut indices: Vec<_> = (0..count)
            .filter(|&i| (hidden | absent) & (1 << i) == 0)
            .collect();
        indices.sort_by_key(|&i| store.child_paint_index(children[i]).unwrap());
        let expected: Vec<usize> = f[5]
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(indices, expected, "{row}");
        assert_eq!(
            store.object(parent).unwrap().children(),
            children,
            "painting never reorders logical indices"
        );
        assert_eq!(store.apex_index(parent), Some(apex));
        snapshots += 1;
    }
    assert_eq!(snapshots, 756);
}

#[test]
fn scroll_overlays_keep_their_layers_for_components_lists_and_trees() {
    for kind in [ObjectType::Component, ObjectType::List, ObjectType::Tree] {
        let mut store = ObjectStore::new();
        let parent = store.create(kind);
        let rows: Vec<_> = (0..3)
            .map(|_| {
                let id = store.create(ObjectType::Component);
                store.set_parent(id, Some(parent));
                id
            })
            .collect();
        let bar = store.create(ObjectType::ScrollBar);
        store.set_parent(bar, Some(parent));
        let mut pane = ScrollPane::default();
        pane.vt_bar = Some(bar);
        pane.header_res = Some("Header".into());
        store.objects[parent]
            .control
            .component_mut()
            .unwrap()
            .scroll_pane = Some(pane);
        store.attach_scroll_refresh_bars(parent, &mut |store, _| {
            Some(store.create(ObjectType::Component))
        });
        let header = store.scroll_header(parent).unwrap();
        for order in [
            ChildrenRenderOrder::Ascent,
            ChildrenRenderOrder::Descent,
            ChildrenRenderOrder::Arch,
        ] {
            store.set_children_render_order(parent, order);
            store.set_apex_index(parent, 1);
            for &row in &rows {
                assert!(store.child_paint_index(header) < store.child_paint_index(row));
                assert!(store.child_paint_index(bar) > store.child_paint_index(row));
            }
        }
        assert_eq!(store.object(rows[0]).unwrap().parent(), Some(parent));
        assert!(
            store
                .object(parent)
                .unwrap()
                .control()
                .component()
                .is_some()
        );
    }
}

#[test]
fn render_modes_do_not_change_selected_objects_and_apex_edits_only_dirty_arch() {
    let mut store = ObjectStore::new();
    let parent = store.create(ObjectType::List);
    let rows: Vec<_> = (0..3)
        .map(|_| {
            let id = store.create(ObjectType::Button);
            if let ControlData::Button(data) = &mut store.objects[id].control {
                data.mode = crate::widget::ButtonMode::Check;
            }
            store.set_parent(id, Some(parent));
            id
        })
        .collect();
    store.set_list_selection(parent, 1);
    store.take_dirty();
    assert!(store.set_apex_index(parent, 99));
    assert!(store.take_dirty().is_empty());
    assert!(store.set_children_render_order(parent, ChildrenRenderOrder::Arch));
    assert_eq!(store.list_selections(parent), Some(vec![1]));
    assert_eq!(store.object(parent).unwrap().children(), rows);
    assert!(store.child_paint_index(rows[2]) > store.child_paint_index(rows[1]));
    store.take_dirty();
    assert!(store.set_apex_index(parent, 1));
    assert!(!store.take_dirty().is_empty());
    assert!(store.child_paint_index(rows[1]) > store.child_paint_index(rows[2]));
    let image = store.create(ObjectType::Image);
    assert!(!store.set_children_render_order(image, ChildrenRenderOrder::Descent));
    assert_eq!(store.child_paint_index(image), None);
}
