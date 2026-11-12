//! 包解析的集成测试，使用官方示例工程导出的真实 `.fui` 文件。
//!
//! 夹具来源与许可见 `tests/fixtures/README.md`。

use fairygui_core::package::{FuiPackage, LoadContext, PackageItemType};

const JOYSTICK: &[u8] = include_bytes!("fixtures/Joystick.fui");
const MAIN_MENU: &[u8] = include_bytes!("fixtures/MainMenu.fui");
const BAG: &[u8] = include_bytes!("fixtures/Bag.fui");
const TRANSITION: &[u8] = include_bytes!("fixtures/Transition.fui");

fn atlas_of(package: &FuiPackage) -> &fairygui_core::package::PackageItem {
    package
        .items()
        .iter()
        .find(|item| item.item_type == PackageItemType::Atlas)
        .expect("package should contain an atlas item")
}

#[test]
fn parses_minimal_package() {
    let package = FuiPackage::parse("Joystick", JOYSTICK).expect("parse joystick package");

    assert_eq!(package.name(), "Joystick");
    assert!(package.version() >= 2, "包格式版本应当在 ver2 分支之上");
    assert!(package.item_count() > 0);
    assert!(package.dependencies().is_empty());
    assert!(package.branches().is_empty());
}

#[test]
fn parses_header_and_items() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main menu package");

    assert_eq!(package.name(), "MainMenu");
    assert_eq!(package.id(), "58oxr1hs");
    assert_eq!(package.item_count(), 9);
    assert_eq!(
        package.data().len(),
        MAIN_MENU.len(),
        "未压缩包的体量应与输入一致"
    );

    // 条目类型分布：组件、图片与一个图集
    let components = package
        .items()
        .iter()
        .filter(|item| item.item_type == PackageItemType::Component)
        .count();
    let images = package
        .items()
        .iter()
        .filter(|item| item.item_type == PackageItemType::Image)
        .count();
    assert_eq!(components, 3);
    assert_eq!(images, 5);
}

#[test]
fn atlas_file_name_gets_package_prefix() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main menu package");
    // 图集与声音的文件名要带上「包名前缀 + 下划线」
    assert_eq!(
        atlas_of(&package).file.as_deref(),
        Some("MainMenu_atlas0.png")
    );
}

#[test]
fn image_item_maps_to_atlas_rect() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main menu package");

    let index = package
        .item_index_by_name("CloseNormal")
        .expect("CloseNormal should exist");
    let item = package.item(index).expect("item by index");
    assert_eq!(item.item_type, PackageItemType::Image);
    assert_eq!(item.object_type, fairygui_core::package::ObjectType::Image);
    assert_eq!((item.width, item.height), (40.0, 40.0));

    let sprite = package
        .sprite_for(index)
        .expect("image should have a sprite");
    assert_eq!(sprite.rect.x, 0.0);
    assert_eq!(sprite.rect.y, 81.0);
    assert_eq!((sprite.rect.width, sprite.rect.height), (40.0, 40.0));
    assert!(!sprite.rotated);

    // 图集切片指向包内唯一的图集条目
    let atlas_index = package
        .items()
        .iter()
        .position(|candidate| candidate.item_type == PackageItemType::Atlas)
        .expect("atlas index");
    assert_eq!(sprite.atlas_item, atlas_index);
}

#[test]
fn scale9_grid_is_parsed() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main menu package");

    let item = package.item_by_name("b7_png").expect("b7_png should exist");
    let grid = item.scale9_grid.as_ref().expect("b7_png should be scale9");
    assert!(
        grid.width > 0.0 && grid.height > 0.0,
        "九宫格区域应当有效，实际为 {grid:?}"
    );
    assert!(!item.scale_by_tile);
}

#[test]
fn sprite_offset_is_read_for_newer_format() {
    let package = FuiPackage::parse("Transition", TRANSITION).expect("parse transition package");

    // 该包中图片的条目尺寸与切片尺寸不同，说明 offset/originalSize 字段被正确读取。
    let index = package.item_index_by_name("h0").expect("h0 should exist");
    let item = package.item(index).expect("item by index");
    let sprite = package.sprite_for(index).expect("h0 should have a sprite");

    assert_eq!((item.width, item.height), (50.0, 50.0));
    assert_ne!(
        (sprite.rect.width, sprite.rect.height),
        (item.width, item.height),
        "切片尺寸应当与原图尺寸不同，才说明独立字段被读取"
    );
    assert!(sprite.original_size.x > 0.0 && sprite.original_size.y > 0.0);
}

#[test]
fn component_item_keeps_raw_layout_data() {
    let package = FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse main menu package");

    let item = package.item_by_name("Main").expect("Main should exist");
    assert_eq!(item.item_type, PackageItemType::Component);
    assert_eq!(
        item.object_type,
        fairygui_core::package::ObjectType::Component
    );

    let raw = package
        .raw_data(item)
        .expect("component should expose raw layout data");
    assert!(raw.total_len() > 0);
    assert_eq!(raw.version(), package.version(), "子缓冲区应继承版本号");
}

#[test]
fn movie_clip_frames_resolve_to_atlas() {
    let package = FuiPackage::parse("Bag", BAG).expect("parse bag package");

    let index = package
        .item_index_by_name("quan")
        .expect("quan should exist");
    assert_eq!(
        package.item(index).expect("item").item_type,
        PackageItemType::MovieClip
    );

    let clip = package
        .load_movie_clip(index, &LoadContext::none())
        .expect("load movie clip");

    assert_eq!(clip.frames.len(), 12);
    assert_eq!(clip.frame_count(), 12);
    assert_eq!(clip.interval, 166);
    assert!(!clip.swing);
    assert_eq!(clip.repeat_delay, 0);

    // 每帧都应当读到它引用的条目编号
    for (frame_index, frame) in clip.frames.iter().enumerate() {
        assert!(
            frame.sprite_id.is_some(),
            "frame {frame_index} should reference an item"
        );
    }

    for frame in &clip.frames {
        let id = frame.sprite_id.as_deref().unwrap();
        assert!(package.item_index_by_id(id).is_none());
        let sprite = package.sprite_by_id(id).expect("帧切片独立于包条目");
        let texture = frame.texture.as_ref().expect("每帧必须有纹理");
        assert_eq!(texture.rect, sprite.rect);
        assert_eq!(
            package.item(texture.atlas_item).unwrap().item_type,
            PackageItemType::Atlas
        );
    }
}

#[test]
fn large_package_parses_all_items() {
    let package = FuiPackage::parse("Transition", TRANSITION).expect("parse transition package");
    assert_eq!(package.item_count(), 56);

    // 精灵表与条目表一一对应，不应出现映射失败
    let mapped = (0..package.item_count())
        .filter(|&index| package.sprite_for(index).is_some())
        .count();
    let images = package
        .items()
        .iter()
        .filter(|item| item.item_type == PackageItemType::Image)
        .count();
    assert_eq!(mapped, images, "每个图片条目都应映射到一个图集切片");
}

#[test]
fn rejects_non_fairygui_data() {
    let err = FuiPackage::parse("bogus", b"NOTFGUI________________").unwrap_err();
    assert!(
        matches!(err, fairygui_core::Error::InvalidMagic { .. }),
        "应当报告魔数错误，实际为 {err}"
    );
}
