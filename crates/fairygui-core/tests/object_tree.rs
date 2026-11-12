//! 对象树构建的集成测试，使用官方示例工程导出的真实 `.fui` 文件。

use fairygui_core::object::ObjectStore;
use fairygui_core::package::{FuiPackage, LoadContext, ObjectType};
use fairygui_core::widget::{ControlData, create_from_package};

const MAIN_MENU: &[u8] = include_bytes!("fixtures/MainMenu.fui");
const BAG: &[u8] = include_bytes!("fixtures/Bag.fui");

#[test]
fn resource_dimensions_apply_before_optional_instance_size() {
    let (package, store, root) = build(
        "Basics",
        include_bytes!("fixtures/Basics.fui"),
        "Demo_Controller",
    );
    for name in ["n13", "n14", "n15"] {
        let member = store
            .object(store.child_by_name(root, name).unwrap())
            .unwrap();
        assert_eq!(member.size(), fairygui_core::geom::Size::new(90.0, 90.0));
        assert_eq!(member.source_size(), member.size());
        let item = package
            .item_index_by_id(member.package_item_id().unwrap())
            .unwrap();
        let mut direct_store = ObjectStore::new();
        let direct =
            create_from_package(&mut direct_store, &package, item, &LoadContext::none()).unwrap();
        assert_eq!(direct_store.object(direct).unwrap().size(), member.size());
    }
}

fn build(
    name: &str,
    data: &[u8],
    item_name: &str,
) -> (FuiPackage, ObjectStore, fairygui_core::GObjectId) {
    let package = FuiPackage::parse(name, data).expect("parse package");
    let mut store = ObjectStore::new();
    let index = package
        .item_index_by_name(item_name)
        .unwrap_or_else(|| panic!("item {item_name} should exist"));
    let root = create_from_package(&mut store, &package, index, &LoadContext::none())
        .expect("object tree should build");
    (package, store, root)
}

/// 描述里记了组编号的对象，在构建完成后应当真的绑定了分组控件。
///
/// 这一步曾经被整段跳过——编号被丢弃、成员从不登记，于是分组布局在真实包里
/// 一直是空转的。
#[test]
fn group_members_are_bound_to_their_group() {
    const FIXTURES: &[(&str, &[u8])] = &[
        ("Basics", include_bytes!("fixtures/Basics.fui")),
        ("Bag", include_bytes!("fixtures/Bag.fui")),
        ("MainMenu", include_bytes!("fixtures/MainMenu.fui")),
        ("Joystick", include_bytes!("fixtures/Joystick.fui")),
        ("Transition", include_bytes!("fixtures/Transition.fui")),
        ("Chat", include_bytes!("fixtures/Chat.fui")),
    ];

    let mut checked = 0usize;

    for (name, bytes) in FIXTURES {
        let Ok(package) = FuiPackage::parse(*name, bytes) else {
            continue;
        };

        for (index, item) in package.items().iter().enumerate() {
            if item.item_type != fairygui_core::package::PackageItemType::Component {
                continue;
            }

            let mut store = ObjectStore::new();
            let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
            else {
                continue;
            };

            let mut candidates = vec![root];
            store.for_each_descendant(root, &mut |id| candidates.push(id));

            for id in candidates {
                let Some(object) = store.object(id) else {
                    continue;
                };

                if object.group_index().is_some() {
                    checked += 1;
                    assert!(
                        object.group().is_some(),
                        "{name}: 记了组编号的对象应当绑定到分组控件"
                    );
                }

                // 分组必须与自己同在一个容器里，否则布局无从下手。
                if let Some(group) = object.group() {
                    assert_eq!(
                        store.object(group).and_then(|group| group.parent()),
                        object.parent(),
                        "{name}: 分组成员与分组应当在同一个容器里"
                    );
                }
            }
        }
    }

    println!("检查了 {checked} 个带组编号的对象");
}

#[test]
fn builds_root_with_declared_size() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");
    let object = store.object(root).expect("root object");

    assert_eq!(object.kind(), ObjectType::Component);
    assert_eq!((object.width(), object.height()), (1136.0, 640.0));
    assert_eq!(object.source_size().width, 1136.0);
    assert!(matches!(object.control(), ControlData::Component(_)));
}

#[test]
fn root_has_children_placed_in_order() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");
    let object = store.object(root).expect("root object");

    assert!(!object.children().is_empty(), "主界面应当包含子对象");

    for &child in object.children() {
        let child_object = store.object(child).expect("child object");
        assert_eq!(
            child_object.parent(),
            Some(root),
            "子对象的父引用应当指回根对象"
        );
    }
}

#[test]
fn exported_instance_ids_are_separate_from_names_and_source_items() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");
    let button = store.child_by_name(root, "n1").expect("menu button n1");
    let object = store.object(button).unwrap();
    assert_eq!(object.instance_id(), Some("n1_rftu"));
    assert_eq!(object.package_item_id(), Some("rftu4"));
    assert_eq!(store.child_by_id(root, "n1_rftu"), Some(button));
    assert_eq!(store.child_by_id(root, "n1"), None);
    assert_eq!(store.child_by_id(root, "rftu4"), None);
    let title = store.child_by_name(button, "title").expect("button title");
    let title_id = store
        .object(title)
        .unwrap()
        .instance_id()
        .expect("title ID");
    assert_eq!(store.child_by_id(button, title_id), Some(title));
    assert_eq!(store.child_by_id(root, title_id), None);
}

#[test]
fn descendants_carry_their_own_control_data() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");

    let mut images = 0usize;
    let mut components = 0usize;
    let mut texts = 0usize;
    let mut others = 0usize;

    store.for_each_descendant(root, &mut |id| {
        let Some(object) = store.object(id) else {
            return;
        };
        match object.control() {
            ControlData::Image(_) => images += 1,
            ControlData::Component(_) => components += 1,
            ControlData::Text(_) => texts += 1,
            _ => others += 1,
        }
    });

    assert!(images + components + texts + others > 0, "应当遍历到子对象");
    assert!(images > 0, "主界面里应当有图片子对象");
}

#[test]
fn nested_components_are_expanded_recursively() {
    // Bag 包里的 BagWin 由多个子组件拼成，能验证递归构建。
    let (_package, store, root) = build("Bag", BAG, "BagWin");
    let object = store.object(root).expect("root object");

    assert_eq!(object.kind(), ObjectType::Component);
    assert!(!object.children().is_empty());

    // 至少存在一层「组件里再套组件」的结构。
    let nested = object.children().iter().any(|&child| {
        store
            .object(child)
            .map(|child_object| !child_object.children().is_empty())
            .unwrap_or(false)
    });
    assert!(nested, "应当存在嵌套组件");
}

#[test]
fn image_child_keeps_source_item_reference() {
    let (package, store, root) = build("MainMenu", MAIN_MENU, "Main");

    let mut found = None;
    store.for_each_descendant(root, &mut |id| {
        if found.is_some() {
            return;
        }
        if let Some(object) = store.object(id)
            && matches!(object.control(), ControlData::Image(_))
        {
            found = Some(id);
        }
    });

    let image = found.expect("应当找到图片子对象");
    let object = store.object(image).expect("image object");

    // 图片子对象应当记录它引用的包条目，渲染层要靠它找到图集切片。
    let item_id = object
        .package_item_id()
        .expect("图片子对象应当记录来源条目");
    assert!(
        package.item_by_id(item_id).is_some(),
        "来源条目应当存在于包内，实际为 {item_id}"
    );
}

#[test]
fn children_positions_are_read_from_description() {
    let (_package, store, root) = build("Bag", BAG, "BagWin");
    let object = store.object(root).expect("root object");

    // 描述里明确写过位置，至少应当有一个子对象不在 (0, 0)。
    let moved = object.children().iter().any(|&child| {
        store
            .object(child)
            .map(|child_object| child_object.position() != fairygui_core::geom::Point::ZERO)
            .unwrap_or(false)
    });
    assert!(moved, "子对象应当带有描述里写好的位置");
}

#[test]
fn store_removal_takes_the_whole_tree() {
    let (_package, mut store, root) = build("MainMenu", MAIN_MENU, "Main");
    let before = store.len();

    let removed = store.remove(root);

    assert_eq!(removed.len(), before, "移除根对象应当带走整棵树");
    assert!(store.is_empty());
    assert!(!store.contains(root));
}
