use super::*;
use crate::package::ObjectType;

#[test]
fn folding_reflows_visible_rows_without_changing_logical_children() {
    let mut store = ObjectStore::new();
    let list = store.create(ObjectType::List);
    store.set_size(list, 100., 80.);
    let first = store.create(ObjectType::Component);
    let hidden = store.create(ObjectType::Component);
    let last = store.create(ObjectType::Component);
    for row in [first, hidden, last] {
        store.set_size(row, 30., 20.);
        store.set_parent(row, Some(list));
    }
    store.set_visible(hidden, false);
    store.layout_list(list);
    assert_eq!(store.object(last).unwrap().y(), 40.);
    let initial = store.list_interaction_settings(list).unwrap();
    assert!(initial.scroll_item_to_view_on_click);
    assert!(!initial.fold_invisible_items);
    assert!(store.set_list_interaction_settings(
        list,
        ListInteractionSettings {
            scroll_item_to_view_on_click: false,
            fold_invisible_items: true,
        }
    ));
    assert_eq!(store.object(last).unwrap().y(), 20.);
    assert_eq!(
        store.object(list).unwrap().children(),
        &[first, hidden, last]
    );
    assert_eq!(
        store.list_interaction_settings(list),
        Some(ListInteractionSettings {
            scroll_item_to_view_on_click: false,
            fold_invisible_items: true,
        })
    );
}
