use super::*;
use crate::{Size, package::ObjectType, scroll::ScrollPane, widget::ControlData};

fn object(store: &mut ObjectStore, kind: ObjectType, item: &str) -> GObjectId {
    let object = store.create(kind);
    store.object_mut(object).unwrap().source_package_id = Some("p".into());
    store.set_source(object, kind, Some(item.into()));
    store.set_size(object, 40., 20.);
    object
}

#[test]
fn list_pool_is_owned_fifo_preserves_state_and_rejects_duplicates_foreign_rows_and_cycles() {
    let mut store = ObjectStore::new();
    let list = object(&mut store, ObjectType::List, "list");
    let first = object(&mut store, ObjectType::Component, "a");
    let second = object(&mut store, ObjectType::Component, "a");
    let nested = object(&mut store, ObjectType::Component, "nested");
    store.set_parent(nested, Some(first));
    for row in [first, second] {
        store.set_parent(row, Some(list));
    }
    store.attach_to_stage(list);
    store.set_visible(first, false);
    assert!(store.return_list_item_to_pool(list, first));
    assert!(!store.return_list_item_to_pool(list, first));
    assert!(!store.object(first).unwrap().visible());
    assert!(!store.is_on_stage(nested));
    assert!(store.return_list_item_to_pool(list, second));
    assert_eq!(store.take_list_item_from_pool(list, "ui://pa"), Some(first));
    assert!(store.object(first).unwrap().visible());
    assert_eq!(store.object(nested).unwrap().parent(), Some(first));
    store.set_parent(first, Some(list));
    assert_eq!(
        store.take_list_item_from_pool(list, "ui://pa"),
        Some(second)
    );
    let foreign = object(&mut store, ObjectType::List, "foreign");
    store.set_parent(second, Some(foreign));
    assert!(!store.return_list_item_to_pool(list, second));
    assert!(store.return_list_item_to_pool(foreign, list));
    assert!(
        !store.return_list_item_to_pool(list, foreign),
        "pooled ownership must not form a cycle"
    );
    store.remove(foreign);
    assert!(
        store.is_empty(),
        "disposing an owner also disposes complete pooled subtrees"
    );
}

#[test]
fn enabling_virtual_layout_consumes_the_ordinary_pool_in_native_fifo_order() {
    let mut store = ObjectStore::new();
    let list = object(&mut store, ObjectType::List, "list");
    store.set_size(list, 120., 80.);
    store
        .object_mut(list)
        .unwrap()
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(ScrollPane::default());
    store.layout_scroll_bars(list);
    let rows: Vec<_> = (0..4)
        .map(|i| {
            let row = object(
                &mut store,
                ObjectType::Component,
                if i % 2 == 0 { "a" } else { "b" },
            );
            store.set_size(row, 40. + i as f32, 20. + i as f32);
            store.set_parent(row, Some(list));
            row
        })
        .collect();
    assert!(store.return_list_item_to_pool(list, rows[2]));
    assert!(store.return_list_item_to_pool(list, rows[3]));
    store
        .enable_virtual_list(list, "ui://pa", &mut |_, _| {
            panic!("cached template must be reused")
        })
        .unwrap();
    assert_eq!(store.virtual_item_size(list), Some(Size::new(42., 22.)));
    assert_eq!(
        store.virtual_states[&list]
            .pool
            .iter()
            .map(|(_, row)| *row)
            .collect::<Vec<_>>(),
        vec![rows[3], rows[0], rows[1], rows[2]]
    );
    assert!(store.take_list_item_from_pool(list, "ui://pa").is_none());
    assert!(
        !store.return_list_item_to_pool(list, rows[2]),
        "virtual pool retains exclusive ownership"
    );
    store.remove(list);
    assert!(store.is_empty());
}

#[test]
fn list_pool_does_not_detach_scroll_overlays_and_drops_disposed_cached_rows() {
    let mut store = ObjectStore::new();
    let list = object(&mut store, ObjectType::List, "list");
    let bar = object(&mut store, ObjectType::ScrollBar, "bar");
    let mut pane = ScrollPane::default();
    pane.vt_bar = Some(bar);
    if let ControlData::List(data) = &mut store.object_mut(list).unwrap().control {
        data.component.scroll_pane = Some(pane);
    }
    store.set_parent(bar, Some(list));
    assert!(!store.return_list_item_to_pool(list, bar));
    assert_eq!(store.scroll_pane(list).unwrap().vt_bar, Some(bar));
    let row = object(&mut store, ObjectType::Component, "a");
    store.set_parent(row, Some(list));
    assert!(store.return_list_item_to_pool(list, row));
    store.remove(row);
    assert!(store.take_list_item_from_pool(list, "ui://pa").is_none());
    store.remove(list);
    assert!(store.is_empty());
}
