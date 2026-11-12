//! 齿轮联动的集成测试。
//!
//! 官方示例工程的 Basics 包在同一批组件里用到了控制器与齿轮，
//! 因此可以在真实数据上验证「切页 → 属性变化」这条链路。

use fairygui_core::object::{GObjectId, ObjectStore};
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::create_from_package;

const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

#[test]
fn menu_button_background_survives_every_pointer_state_and_matches_instance_size() {
    use fairygui_core::widget::{ButtonState, GearKind};
    let package = FuiPackage::parse("MainMenu", include_bytes!("fixtures/MainMenu.fui")).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Main").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let button = store.find_by_name(root, "n1").unwrap();
    let size = store.object(button).unwrap().size();
    for state in [
        ButtonState::Up,
        ButtonState::Over,
        ButtonState::Down,
        ButtonState::Over,
        ButtonState::Up,
    ] {
        store.set_button_state(button, state);
        let page = store
            .object(button)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("button")
            .unwrap()
            .selected_page_id()
            .unwrap()
            .to_string();
        let mut visible_backgrounds = 0;
        for &child in store.object(button).unwrap().children() {
            let object = store.object(child).unwrap();
            for gear in object
                .gears()
                .iter()
                .filter(|g| g.kind == GearKind::Display)
            {
                assert_eq!(
                    object.display_visible(),
                    gear.pages.is_empty() || gear.pages.contains(&page),
                    "state {state:?}, child {}",
                    object.name()
                );
            }
            if matches!(
                object.control(),
                fairygui_core::widget::ControlData::Image(_)
            ) && object.display_visible()
            {
                visible_backgrounds += 1;
                assert_eq!(object.size(), size, "background must follow resized button");
            }
        }
        assert!(
            visible_backgrounds > 0,
            "state {state:?} lost every background"
        );
    }
}

#[test]
fn parent_page_changes_leave_nested_button_controller_gears_alone() {
    let package = FuiPackage::parse("Basics", BASICS).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Main").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let button = store.find_by_name(root, "btn_Graph").unwrap();
    store.set_button_state(button, fairygui_core::widget::ButtonState::Over);
    let before: Vec<_> = store
        .object(button)
        .unwrap()
        .children()
        .iter()
        .map(|id| (*id, snapshot(&store, *id)))
        .collect();
    assert!(store.set_controller_page_index(root, "c1", 1));
    store.advance_gear_tweens(1.0);
    for (id, expected) in before {
        assert_eq!(
            snapshot(&store, id),
            expected,
            "parent controller must not operate nested button gears"
        );
    }
}

#[test]
fn official_controller_demo_moves_group_members_along_the_group_gear_tween() {
    let package = FuiPackage::parse("Basics", BASICS).unwrap();
    let mut store = ObjectStore::new();
    let root = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Demo_Controller").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let group = store.child_by_name(root, "n16").unwrap();
    let member = store.child_by_name(root, "n13").unwrap();
    let initial = store.position_of(member).unwrap();
    let group_x = store.object(group).unwrap().x();
    store.set_controller_page_index(root, "c2", 1);
    store.advance_gear_tweens(0.1);
    assert_eq!(
        store.object(member).unwrap().x() - initial.x,
        store.object(group).unwrap().x() - group_x
    );
    store.advance_gear_tweens(1.0);
    assert_eq!(store.object(group).unwrap().x(), 661.0);
    assert_eq!(
        store.object(member).unwrap().x(),
        initial.x + 661.0 - group_x
    );
}

/// 记录一个对象可被齿轮影响的属性。
fn snapshot(store: &ObjectStore, id: GObjectId) -> Option<(bool, f32, f32, f32, f32)> {
    let object = store.object(id)?;
    Some((
        object.display_visible(),
        object.position().x,
        object.position().y,
        object.width(),
        object.height(),
    ))
}

/// 在子树中找出第一个挂了齿轮的对象。
fn first_with_gears(store: &ObjectStore, root: GObjectId) -> Option<GObjectId> {
    let mut found = None;
    let mut visit = |id: GObjectId| {
        if found.is_none()
            && let Some(object) = store.object(id)
            && !object.gears().is_empty()
        {
            found = Some(id);
        }
    };
    if let Some(object) = store.object(root)
        && !object.gears().is_empty()
    {
        return Some(root);
    }
    store.for_each_descendant(root, &mut visit);
    found
}

#[test]
fn sample_package_contains_gears() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    let mut gear_count = 0usize;
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let mut count = |id: GObjectId| {
            if let Some(object) = store.object(id) {
                gear_count += object.gears().len();
            }
        };
        count(root);
        store.for_each_descendant(root, &mut count);
    }

    assert!(gear_count > 0, "示例包中应当包含齿轮");
}

#[test]
fn switching_controller_page_changes_gear_driven_properties() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    let mut applied = 0usize;

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        // 只处理「恰好一个控制器、且至少两页」的容器，便于确定切换目标。
        let Some((controller_name, target_page)) = (|| {
            let object = store.object(root)?;
            let data = object.control().component()?;
            if data.controllers.len() != 1 {
                return None;
            }
            let controller = &data.controllers[0];
            if controller.page_count() < 2 || controller.name.is_empty() {
                return None;
            }
            Some((controller.name.clone(), controller.page_names[1].clone()))
        })() else {
            continue;
        };

        let Some(target) = first_with_gears(&store, root) else {
            continue;
        };
        let before = snapshot(&store, target);

        if !store.set_controller_page(root, &controller_name, &target_page) {
            continue;
        }

        let after = snapshot(&store, target);
        if before != after {
            applied += 1;
        }
    }

    assert!(applied > 0, "应当至少有一次切页通过齿轮改变了对象的属性");
}

#[test]
fn switching_to_same_page_reports_no_change() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let Some((controller_name, current_page)) = (|| {
            let object = store.object(root)?;
            let data = object.control().component()?;
            let controller = data.controllers.first()?;
            if controller.name.is_empty() {
                return None;
            }
            let page = controller.selected_page_name()?.to_string();
            Some((controller.name.clone(), page))
        })() else {
            continue;
        };

        // 切到当前页面不应被当成变化。
        assert!(
            !store.set_controller_page(root, &controller_name, &current_page),
            "切到同一页面不应报告变化"
        );
        return;
    }
}
