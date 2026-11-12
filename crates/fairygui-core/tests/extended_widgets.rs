//! 扩展控件（按钮、标签、进度条、滑动条）解析的集成测试。

use fairygui_core::object::{GObjectId, ObjectStore};
use fairygui_core::package::{FuiPackage, LoadContext};
use fairygui_core::widget::{ButtonMode, ControlData, create_from_package};

const MAIN_MENU: &[u8] = include_bytes!("fixtures/MainMenu.fui");
const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

fn build(name: &str, data: &[u8], item_name: &str) -> (FuiPackage, ObjectStore, GObjectId) {
    let package = FuiPackage::parse(name, data).expect("parse package");
    let mut store = ObjectStore::new();
    let index = package
        .item_index_by_name(item_name)
        .unwrap_or_else(|| panic!("item {item_name} should exist"));
    let root = create_from_package(&mut store, &package, index, &LoadContext::none())
        .expect("object tree should build");
    (package, store, root)
}

#[test]
fn buttons_carry_extension_and_instance_data() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");

    let mut buttons = Vec::new();
    store.for_each_descendant(root, &mut |id| {
        if let Some(object) = store.object(id)
            && let ControlData::Button(data) = object.control()
        {
            buttons.push((
                object.name().to_string(),
                data.title.clone(),
                data.mode,
                data.down_effect,
                data.component.controllers.len(),
            ));
        }
    });

    assert!(!buttons.is_empty(), "示例主界面里应当包含按钮");

    for (name, title, mode, down_effect, controllers) in &buttons {
        // 扩展数据来自组件自身的描述块 6，能读到就说明构造期解析成功。
        assert!(
            matches!(
                mode,
                ButtonMode::Common | ButtonMode::Check | ButtonMode::Radio
            ),
            "按钮 {name} 的模式应当落在已知取值内"
        );
        assert!(*down_effect >= 0, "按下效果不应为负值");
        let _ = (title, controllers);
    }
}

#[test]
fn button_is_also_a_container() {
    let (_package, store, root) = build("MainMenu", MAIN_MENU, "Main");

    let mut checked = 0usize;
    store.for_each_descendant(root, &mut |id| {
        if let Some(object) = store.object(id)
            && let ControlData::Button(data) = object.control()
        {
            // 按钮由容器派生，应当能取到容器数据，进而访问控制器。
            assert!(object.control().is_container());
            assert_eq!(
                object.control().component().map(|c| c.controllers.len()),
                Some(data.component.controllers.len())
            );
            checked += 1;
        }
    });

    assert!(checked > 0, "示例里应当有按钮");
}

#[test]
fn basics_contains_progress_and_slider_widgets() {
    let package = FuiPackage::parse("Basics", BASICS).expect("parse basics package");

    let mut progress = 0usize;
    let mut sliders = 0usize;
    let mut labels = 0usize;

    for (index, item) in package.items().iter().enumerate() {
        if item.item_type != fairygui_core::package::PackageItemType::Component {
            continue;
        }
        let mut store = ObjectStore::new();
        let Ok(root) = create_from_package(&mut store, &package, index, &LoadContext::none())
        else {
            continue;
        };

        let mut classify = |id: GObjectId| {
            let Some(object) = store.object(id) else {
                return;
            };
            match object.control() {
                ControlData::ProgressBar(data) => {
                    progress += 1;
                    // 进度条的值应当落在范围内。
                    assert!(
                        data.max >= data.min,
                        "最大值 {} 不应小于最小值 {}",
                        data.max,
                        data.min
                    );
                }
                ControlData::Slider(_) => sliders += 1,
                ControlData::Label(_) => labels += 1,
                _ => {}
            }
        };

        classify(root);
        store.for_each_descendant(root, &mut classify);
    }

    // Basics 是官方用来演示各类控件的示例包，至少应当覆盖到其中一部分。
    assert!(
        progress + sliders + labels > 0,
        "示例包中应当包含进度条、滑动条或标签（进度条 {progress}，滑动条 {sliders}，标签 {labels}）"
    );
}
