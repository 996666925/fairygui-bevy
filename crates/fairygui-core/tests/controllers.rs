//! 控制器解析的集成测试。
//!
//! 官方示例工程里的 Basics 包专门演示控制器用法，因此用它来验证这条链路。

use fairygui_core::GObjectId;
use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::create_from_package;

const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

/// 统计一棵对象树里所有容器的控制器总数。
fn count_controllers(store: &ObjectStore, root: GObjectId) -> usize {
    let mut total = 0usize;
    let mut add = |id: GObjectId| {
        if let Some(object) = store.object(id)
            && let Some(data) = object.control().component()
        {
            total += data.controllers.len();
        }
    };

    add(root);
    store.for_each_descendant(root, &mut add);
    total
}

#[test]
fn sample_package_contains_controllers() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    let mut total = 0usize;
    let mut with_controllers = 0usize;

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }

        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let count = count_controllers(&store, root);
        if count > 0 {
            with_controllers += 1;
            total += count;
        }
    }

    assert!(
        total > 0,
        "官方示例包中应当包含控制器（共检查 {with_controllers} 个含控制器的组件）"
    );
}

#[test]
fn controller_pages_are_read_with_names() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    let mut found = None;
    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }

        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };
        let Some(object) = store.object(root) else {
            continue;
        };
        let Some(data) = object.control().component() else {
            continue;
        };

        if let Some(controller) = data.controllers.first()
            && !controller.page_names.is_empty()
        {
            found = Some((
                controller.name.clone(),
                controller.page_names.clone(),
                controller.selected_index,
            ));
            break;
        }
    }

    let (name, pages, selected) = found.expect("应当能找到带页面的控制器");
    assert!(!name.is_empty(), "控制器应当有名称");
    assert!(pages.iter().all(|page| !page.is_empty()), "页面应当有名称");
    assert!(
        selected >= 0 && (selected as usize) < pages.len(),
        "首屏页面序号应当落在范围内，实际为 {selected} / {}",
        pages.len()
    );
}
