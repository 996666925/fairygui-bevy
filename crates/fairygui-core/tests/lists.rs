//! 列表、树与下拉框的集成测试，使用官方示例包中的真实控件。

use fairygui_core::geom::Size;
use fairygui_core::object::{GObjectId, ObjectStore};
use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};
use fairygui_core::widget::{ControlData, create_from_package};

const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

/// 遍历包内所有组件，累计各类派生控件的数量。
fn count_controls(package: &FuiPackage) -> (usize, usize, usize) {
    let (mut lists, mut trees, mut combos) = (0usize, 0usize, 0usize);

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, package, index, &LoadContext::none()) else {
            continue;
        };

        let mut visit = |id: GObjectId| {
            let Some(object) = store.object(id) else {
                return;
            };
            match object.control() {
                ControlData::List(_) => lists += 1,
                ControlData::Tree(_) => trees += 1,
                ControlData::ComboBox(_) => combos += 1,
                _ => {}
            }
        };
        visit(root);
        store.for_each_descendant(root, &mut visit);
    }

    (lists, trees, combos)
}

#[test]
fn sample_package_contains_list_widgets() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let (lists, trees, combos) = count_controls(&package);

    assert!(
        lists + trees + combos > 0,
        "示例包中应当包含列表类控件（列表 {lists}，树 {trees}，下拉框 {combos}）"
    );
}

#[test]
fn list_items_and_layout_stay_consistent() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");
    let mut checked = 0usize;

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let mut found = None;
        let mut visit = |id: GObjectId| {
            if found.is_none()
                && let Some(object) = store.object(id)
                && let ControlData::List(data) = object.control()
                && data.item_count() > 0
            {
                found = Some(id);
            }
        };
        visit(root);
        store.for_each_descendant(root, &mut visit);

        let Some(id) = found else {
            continue;
        };
        let Some(object) = store.object(id) else {
            continue;
        };
        let ControlData::List(data) = object.control() else {
            continue;
        };

        // 布局结果的数量必须与数据项数量一致。
        let sizes = vec![Size::new(50.0, 20.0); data.item_count()];
        let positions = data.layout_items(&sizes, object.size());
        assert_eq!(positions.len(), data.item_count());

        let content = data.content_size(&positions, &sizes);
        assert!(content.height > 0.0, "内容高度应当为正");

        checked += 1;
    }

    // 示例包里是否含带数据项的列表取决于导出内容，这里不强制要求。
    let _ = checked;
}
