use super::*;
use crate::{
    geom::{Point, Size},
    package::ObjectType,
    widget::{ControlData, Controller},
};

fn parent(store: &mut ObjectStore, count: usize) -> GObjectId {
    let id = store.create(ObjectType::Component);
    store.objects[id]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(Controller {
            name: "pages".into(),
            page_ids: (0..count).map(|i| i.to_string()).collect(),
            page_names: (0..count).map(|i| format!("Page {i}")).collect(),
            selected_index: if count > 1 { 1 } else { 0 },
            ..Default::default()
        });
    id
}

fn pane(
    store: &mut ObjectStore,
    owner: GObjectId,
    direction: i8,
    page: bool,
    content: Size,
) -> GObjectId {
    let id = store.create(ObjectType::Component);
    store.set_parent(id, Some(owner));
    store.set_size(id, 100., 80.);
    let mut pane = ScrollPane::default();
    pane.scroll_type = ScrollType::from_code(direction);
    pane.page_mode = page;
    pane.set_view_size(Size::new(100., 80.));
    pane.set_content_size(content);
    store.objects[id]
        .control
        .component_mut()
        .unwrap()
        .scroll_pane = Some(pane);
    assert!(store.set_scroll_page_controller(id, Some((owner, 0))));
    id
}

fn selected(store: &ObjectStore, owner: GObjectId) -> i32 {
    store.objects[owner]
        .control
        .component()
        .unwrap()
        .controllers[0]
        .selected_index
}

#[test]
fn page_reads_feedback_and_controller_commands_match_420_native_cases() {
    let mut reads = 0;
    let mut drives = 0;
    for line in include_str!("../../tests/fixtures/scroll-pages-reference.txt").lines() {
        let Some((kind, rest)) = line.split_once(' ') else {
            continue;
        };
        if kind != "read" && kind != "drive" {
            continue;
        }
        let v: Vec<f32> = rest
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        let mut store = ObjectStore::new();
        let owner = parent(&mut store, if kind == "read" { v[2] as usize } else { 3 });
        let id = pane(
            &mut store,
            owner,
            v[0] as i8,
            v[1] != 0.,
            if kind == "read" {
                Size::new(500., 400.)
            } else {
                Size::new(v[2], v[3])
            },
        );
        if kind == "read" {
            if v[3] != 0. {
                store.changing_controllers.insert((owner, "pages".into()));
            }
            store.set_scroll_pos(id, v[4], v[5], false);
            store.update_scroll_page_controller(id);
            assert_eq!(store.scroll_current_page_x(id), Some(v[6] as i32), "{line}");
            assert_eq!(store.scroll_current_page_y(id), Some(v[7] as i32), "{line}");
            let controller = &store.objects[owner]
                .control
                .component()
                .unwrap()
                .controllers[0];
            assert_eq!(controller.selected_index, v[8] as i32, "{line}");
            assert_eq!(controller.previous_index, v[9] as i32, "{line}");
            assert!(
                !store.scroll_pane(id).unwrap().is_animating(),
                "feedback must not snap the source: {line}"
            );
            reads += 1;
        } else {
            store.objects[owner]
                .control
                .component_mut()
                .unwrap()
                .controllers[0]
                .selected_index = 0;
            store.set_controller_page_index(owner, "pages", v[4] as i32);
            let pane = store.scroll_pane(id).unwrap();
            assert_eq!(pane.pos(), Point::new(v[5], v[6]), "{line}");
            assert_eq!(pane.is_animating(), v[8] != 0., "{line}");
            drives += 1;
        }
    }
    assert_eq!((reads, drives), (324, 96));
}

#[test]
fn pane_feedback_moves_siblings_runs_actions_and_does_not_restart_its_own_tween() {
    let mut store = ObjectStore::new();
    let owner = parent(&mut store, 5);
    let source = pane(&mut store, owner, 0, true, Size::new(500., 80.));
    let sibling = pane(&mut store, owner, 1, true, Size::new(100., 400.));
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .controllers[0]
        .actions
        .push(crate::action::ControllerAction::ChangePage {
            from_pages: Vec::new(),
            to_pages: Vec::new(),
            object_id: String::new(),
            controller_name: "mirror".into(),
            target_page: "~1".into(),
        });
    let mut mirror = store.objects[owner]
        .control
        .component()
        .unwrap()
        .controllers[0]
        .clone();
    mirror.name = "mirror".into();
    mirror.actions.clear();
    store.objects[owner]
        .control
        .component_mut()
        .unwrap()
        .controllers
        .push(mirror);
    store.set_scroll_current_page_x(source, 2, true);
    assert_eq!(selected(&store, owner), 2);
    assert_eq!(store.scroll_pane(source).unwrap().pos().x, 200.);
    assert_eq!(store.scroll_pane(sibling).unwrap().pos().y, 160.);
    assert_eq!(
        store.objects[owner]
            .control
            .component()
            .unwrap()
            .controllers[1]
            .selected_index,
        2
    );
    store.advance_scrolls(0.1);
    let before = store.scroll_pane(source).unwrap().display_pos();
    store.set_scroll_current_page_x(source, 2, true);
    store.advance_scrolls(0.1);
    assert!(store.scroll_pane(source).unwrap().display_pos().x > before.x);
    store.set_scroll_pos(source, 150., 0., false);
    assert_eq!(
        selected(&store, owner),
        1,
        "an exact half remains on the preceding page"
    );
    assert_eq!(
        store.scroll_pane(source).unwrap().pos().x,
        150.,
        "source feedback must not page-snap itself"
    );
    assert!(!store.scroll_pane(source).unwrap().is_animating());
    assert_eq!(store.scroll_pane(sibling).unwrap().pos().y, 80.);
    store.set_scroll_pos(source, 150.25, 0., false);
    assert_eq!(selected(&store, owner), 2);
}

#[test]
fn binding_is_inert_retains_owner_across_reparenting_and_cleans_up_on_disposal() {
    let mut store = ObjectStore::new();
    let owner = parent(&mut store, 3);
    let other = parent(&mut store, 3);
    let id = pane(&mut store, owner, 0, true, Size::new(500., 80.));
    assert_eq!(selected(&store, owner), 1);
    assert_eq!(store.scroll_pane(id).unwrap().pos(), Point::ZERO);
    assert!(!store.set_scroll_page_controller(id, Some((owner, 50))));
    assert_eq!(store.scroll_page_controller(id), Some((owner, 0)));
    store.set_parent(id, Some(other));
    store.set_controller_page_index(owner, "pages", 2);
    store.set_controller_page_index(other, "pages", 2);
    assert_eq!(store.scroll_pane(id).unwrap().pos(), Point::ZERO);
    store.set_scroll_current_page_x(id, 1, false);
    assert_eq!(selected(&store, owner), 1);
    assert_eq!(selected(&store, other), 2);
    store.remove(owner);
    assert_eq!(store.scroll_page_controller(id), None);
    assert!(store.set_scroll_page_controller(id, Some((other, 0))));
    assert!(store.set_scroll_page_controller(id, None));
    assert!(!store.set_scroll_page_controller(id, None));
    store.remove(id);
    assert!(store.scroll_page_controllers.is_empty());
}

#[test]
fn wheel_grip_drag_and_content_resize_update_bound_controller_without_selecting_rows() {
    let mut store = ObjectStore::new();
    let owner = parent(&mut store, 3);
    let id = pane(&mut store, owner, 1, true, Size::new(100., 400.));
    store.scroll_mouse_wheel(id, 1.);
    assert_eq!(selected(&store, owner), 1);
    store.scroll_mouse_wheel(id, 1.);
    assert_eq!(selected(&store, owner), 2);
    store.set_scroll_perc(id, true, 0.);
    assert_eq!(selected(&store, owner), 0);
    store.begin_scroll_drag(id);
    store.drag_scroll_by(id, 0., -100., 0.1);
    assert_eq!(selected(&store, owner), 1);
    store.end_scroll_drag(id);
    store.update_scroll_content_size(id, Size::new(100., 80.));
    assert_eq!(selected(&store, owner), 0);
    assert!(matches!(
        store.objects[id].control,
        ControlData::Component(_)
    ));
}

#[test]
fn exported_bag_list_binds_parent_controller_after_materialized_ranges_are_ready() {
    use crate::{FuiPackage, package::LoadContext, widget::create_from_package};
    let package = FuiPackage::parse("Bag", include_bytes!("../../tests/fixtures/Bag.fui")).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("BagWin").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let list = store.child_by_name(root, "list").unwrap();
    assert_eq!(
        store.objects[list]
            .control
            .component()
            .unwrap()
            .page_controller,
        Some(0)
    );
    assert_eq!(store.scroll_page_controller(list), Some((root, 0)));
    assert!(store.scroll_pane(list).unwrap().page_mode);
    store.objects[root]
        .control
        .component_mut()
        .unwrap()
        .controllers[0]
        .selected_index = 1;
    store.layout_scroll_bars(list);
    let view = store.scroll_pane(list).unwrap().view_size();
    store.update_scroll_content_size(list, Size::new(view.width * 3., view.height * 3.));
    assert_eq!(
        selected(&store, root),
        1,
        "pre-materialization range updates must preserve home page"
    );
    store.initialize_scroll_page_controllers(root);
    let pos = store.scroll_pane(list).unwrap().pos();
    assert_eq!(pos.x, view.width);
    assert_eq!(pos.y, 0.);
    store.set_scroll_current_page_x(list, 2, false);
    assert_eq!(selected(&store, root), 2);
    store.initialize_scroll_page_controllers(root);
    assert_eq!(store.scroll_pane(list).unwrap().pos().x, view.width * 2.);
}
