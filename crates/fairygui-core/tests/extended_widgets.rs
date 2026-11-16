//! 扩展控件（按钮、标签、进度条、滑动条）解析的集成测试。

use fairygui_core::object::{GObjectId, ObjectStore};
use fairygui_core::package::{FuiPackage, LoadContext};
use fairygui_core::widget::{ButtonMode, ControlData, create_from_package};

const MAIN_MENU: &[u8] = include_bytes!("fixtures/MainMenu.fui");
const BASICS: &[u8] = include_bytes!("fixtures/Basics.fui");

#[test]
fn button_instance_styles_reach_nested_titles_before_initial_controller_application() {
    let (package, mut store, root) = build(
        "ButtonInstance",
        include_bytes!("fixtures/ButtonInstance.fui"),
        "Main",
    );
    for (name, base, visible, selected, font, color) in [
        ("common", "COMMON", "COMMON", false, 24, [51, 153, 204]),
        ("check", "CHECK OFF", "CHECK ON", true, 26, [204, 102, 51]),
        (
            "nested",
            "NESTED OFF",
            "NESTED ON",
            true,
            28,
            [102, 204, 153],
        ),
        ("preserved", "Base", "Base", false, 16, [18, 52, 86]),
        (
            "bound",
            "BOUND BASE",
            "BOUND BASE",
            false,
            22,
            [204, 153, 51],
        ),
        ("radio", "RADIO OFF", "RADIO ON", true, 24, [153, 102, 204]),
    ] {
        let button = store.child_by_name(root, name).unwrap();
        let mut title = store.child_by_name(button, "title").unwrap();
        if name == "nested" {
            title = store.child_by_name(title, "title").unwrap();
        }
        assert_eq!(store.object_text(button), Some(base), "{name}: base title");
        assert_eq!(
            store.object_text(title),
            Some(visible),
            "{name}: visible title"
        );
        assert_eq!(
            store.button_selected(button),
            Some(selected),
            "{name}: selection"
        );
        let field = store.object_text_field(button).unwrap();
        assert_eq!(field.font_size, font, "{name}: font");
        assert_eq!(
            field.color,
            fairygui_core::color::Color::from_rgba8(color[0], color[1], color[2], 255),
            "{name}: color"
        );
    }
    let controller = store
        .object(root)
        .unwrap()
        .control()
        .component()
        .unwrap()
        .controller("pages")
        .unwrap();
    assert_eq!(
        controller.selected_index, 0,
        "instance selection must not switch the parent's initial page"
    );
    let bound = store.child_by_name(root, "bound").unwrap();
    assert_eq!(store.button_related_controller(bound), Some("pages"));
    assert_eq!(
        store.child_index(bound),
        Some(4),
        "construction must not reorder radio buttons"
    );
    store.set_button_selected(bound, true);
    assert_eq!(
        store
            .object(root)
            .unwrap()
            .control()
            .component()
            .unwrap()
            .controller("pages")
            .unwrap()
            .selected_index,
        1
    );
    assert_eq!(
        store.object_text(store.child_by_name(bound, "title").unwrap()),
        Some("BOUND SELECTED")
    );
    let template = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("Nested").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    assert_eq!(store.object_text(template), Some("Base"));
    assert_eq!(store.object_text_field(template).unwrap().font_size, 16);
}

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

#[test]
fn label_input_instance_overrides_reach_nested_titles_and_do_not_modify_templates() {
    let (package, mut store, root) = build(
        "LabelInput",
        include_bytes!("fixtures/LabelInput.fui"),
        "Main",
    );
    for (name, prompt, restrict, max, password, font, color, value) in [
        (
            "direct",
            "Enter four digits",
            "0-9",
            4,
            false,
            24,
            [51, 102, 153],
            "",
        ),
        (
            "nested",
            "Password digits",
            "0-9",
            6,
            true,
            22,
            [153, 51, 102],
            "",
        ),
        (
            "preserved",
            "BaseHint",
            "A-Z",
            8,
            false,
            16,
            [18, 52, 86],
            "BASE",
        ),
        ("cleared", "", "", -1, false, 20, [0, 0, 0], ""),
    ] {
        let label = store.child_by_name(root, name).unwrap();
        let input = store.input_field_of(label).unwrap();
        let ControlData::TextInput(data) = store.object(input).unwrap().control() else {
            panic!()
        };
        assert_eq!(data.prompt_text.as_deref(), Some(prompt), "{name}");
        assert_eq!(data.restrict.as_deref(), Some(restrict), "{name}");
        assert_eq!(
            (data.max_length, data.password, data.text_field.font_size),
            (max, password, font),
            "{name}"
        );
        assert_eq!(
            data.text_field.color,
            fairygui_core::color::Color::from_rgba8(color[0], color[1], color[2], 255)
        );
        assert_eq!(store.object_text(label), Some(value), "{name}");
        store.set_input_prompt(label, Some("Runtime hint"));
        store.set_input_password(label, !password);
        store.apply_titles();
        let ControlData::TextInput(data) = store.object(input).unwrap().control() else {
            panic!()
        };
        assert_eq!(data.prompt_text.as_deref(), Some("Runtime hint"));
        assert_eq!(data.password, !password);
    }
    let template = create_from_package(
        &mut store,
        &package,
        package.item_index_by_name("DirectInput").unwrap(),
        &LoadContext::none(),
    )
    .unwrap();
    let input = store.input_field_of(template).unwrap();
    let ControlData::TextInput(data) = store.object(input).unwrap().control() else {
        panic!()
    };
    assert_eq!(data.prompt_text.as_deref(), Some("BaseHint"));
    assert_eq!(data.restrict.as_deref(), Some("A-Z"));
    assert_eq!(
        (data.max_length, data.password, data.text_field.font_size),
        (8, false, 16)
    );
    assert_eq!(store.object_text(template), Some("BASE"));
}
