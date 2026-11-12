//! `bevy_ui` 适配层。
//!
//! 核心层与同步层都不直接接触 `bevy_ui` 的类型，转换全部集中在这里。
//! 升级 Bevy 时，改动范围被限制在本文件内。

use bevy::prelude::*;
use bevy::text::{LetterSpacing, LineHeight};

use fairygui_core::color::Color;
use fairygui_core::object::GObject;
use fairygui_core::package::AtlasSprite;

/// 依据对象属性构造布局节点。
///
/// FairyGUI 的对象是绝对定位的，因此这里固定使用绝对定位，
/// 并把位置与尺寸直接换算为像素。
pub fn node_for(object: &GObject) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(object.x()),
        top: Val::Px(object.y()),
        width: Val::Px(object.width()),
        height: Val::Px(object.height()),
        ..default()
    }
}

/// 依据对象属性构造二维变换。
///
/// 轴心的处理：FairyGUI 的轴心是旋转与缩放的基准点，而 Bevy 的节点原点在左上角，
/// 因此当轴心不在左上角时，需要用平移把基准点补偿回去。
pub fn transform_for(object: &GObject) -> UiTransform {
    let pivot = object.pivot();
    // Bevy UI 以节点中心为变换原点，而 FairyGUI 的轴心可以位于任意位置。
    // 平移必须是「轴心向量经过缩放/旋转前后的差值」：没有变换时应为零，
    // 否则默认轴心 (0, 0) 会把每个节点平移半个宽高。
    let pivot_from_center = Vec2::new(
        (pivot.x - 0.5) * object.width(),
        (pivot.y - 0.5) * object.height(),
    );
    let scaled = Vec2::new(
        pivot_from_center.x * object.scale_x(),
        pivot_from_center.y * object.scale_y(),
    );
    let angle = object.rotation().to_radians();
    let (sin, cos) = angle.sin_cos();
    let transformed = Vec2::new(
        scaled.x * cos - scaled.y * sin,
        scaled.x * sin + scaled.y * cos,
    );
    let mut offset = pivot_from_center - transformed;
    // Native handleXYChanged subtracts the logical pivot before adding its
    // transform compensation when x/y identify the anchor rather than bounds.
    if object.pivot_as_anchor() {
        offset -= Vec2::new(pivot.x * object.width(), pivot.y * object.height());
    }

    UiTransform {
        translation: Val2::px(offset.x, offset.y),
        scale: Vec2::new(object.scale_x(), object.scale_y()),
        rotation: Rot2::degrees(object.rotation()),
    }
}

/// 置灰时叠加的着色。
///
/// 旧版近似值，为源兼容保留。运行时使用 Laya 灰度矩阵。
pub const GRAYED_TINT: bevy::prelude::Color = bevy::prelude::Color::srgb(0.62, 0.62, 0.66);

/// 图形的渲染外观。
///
/// 矩形与椭圆都能用节点自身的形状表达：矩形就是节点本身，椭圆靠把四角圆角
/// 拉满得到（宽高相等时是正圆，不等时就是椭圆）。填充走背景色，描边走边框。
#[derive(Debug, Clone, Copy)]
pub struct GraphStyle {
    /// 填充色。不填充时是全透明的。
    pub fill: bevy::prelude::Color,
    /// 描边色。
    pub stroke: bevy::prelude::Color,
    /// 描边宽度（像素）。不描边时为零。
    pub stroke_width: f32,
    /// 四角圆角。
    pub radius: BorderRadius,
}

/// 由图形数据算出渲染外观。
pub fn graph_style(data: &fairygui_core::widget::GraphData) -> GraphStyle {
    use fairygui_core::widget::GraphType;

    let fill = if data.has_fill() {
        to_bevy_color(data.fill_color)
    } else {
        bevy::prelude::Color::NONE
    };

    let (stroke, stroke_width) = if data.has_stroke() {
        (to_bevy_color(data.line_color), data.line_size as f32)
    } else {
        (bevy::prelude::Color::NONE, 0.0)
    };

    let radius = if data.graph_kind() == GraphType::Ellipse {
        BorderRadius::MAX
    } else {
        // 圆角顺序与编辑器写入一致：左上、右上、左下、右下。
        let [top_left, top_right, bottom_left, bottom_right] = data.corner_radii();
        BorderRadius {
            top_left: Val::Px(top_left).into(),
            top_right: Val::Px(top_right).into(),
            bottom_left: Val::Px(bottom_left).into(),
            bottom_right: Val::Px(bottom_right).into(),
        }
    };

    GraphStyle {
        fill,
        stroke,
        stroke_width,
        radius,
    }
}

/// 把溢出方式转换为裁剪设置。
///
/// 可滚动的容器交给 `bevy_ui` 的滚动机制处理，这样裁剪、滚动位置与滚轮
/// 都能直接复用引擎的实现，而不必在核心层重造一套。
pub fn overflow_for(overflow: fairygui_core::widget::OverflowType) -> Overflow {
    use fairygui_core::widget::OverflowType;
    match overflow {
        OverflowType::Visible => Overflow::visible(),
        OverflowType::Hidden => Overflow::clip(),
        OverflowType::Scroll => Overflow::scroll(),
    }
}

/// 依据图集切片构造图片节点。
///
/// 切片直接以子区域的方式引用图集纹理，不切出子纹理。九宫格与平铺都在
/// 节点上声明，由渲染层处理。
pub fn image_for(
    sprite: &AtlasSprite,
    texture: Handle<Image>,
    scale9_grid: Option<fairygui_core::geom::Rect>,
    scale_by_tile: bool,
) -> ImageNode {
    use bevy::sprite::{BorderRect, TextureSlicer};
    use bevy::ui::widget::NodeImageMode;

    let mut node = ImageNode {
        image: texture,
        rect: Some(Rect::new(
            sprite.rect.x,
            sprite.rect.y,
            sprite.rect.x + sprite.rect.width,
            sprite.rect.y + sprite.rect.height,
        )),
        // FairyGUI 已经给出了对象尺寸；显式拉伸模式避免 Bevy 在异步纹理
        // 尚未完成上传时用透明占位图重新测量节点，导致图片区域不绘制。
        image_mode: NodeImageMode::Stretch,
        ..default()
    };

    if let Some(grid) = scale9_grid {
        // 描述里记的是切分区域的左上角与宽高，需要换算成四边宽度。
        let left = grid.x;
        let top = grid.y;
        let right = (sprite.original_size.x - (grid.x + grid.width)).max(0.0);
        let bottom = (sprite.original_size.y - (grid.y + grid.height)).max(0.0);

        node.image_mode = NodeImageMode::Sliced(TextureSlicer {
            // 分量顺序为「左、右、上、下」。
            border: BorderRect::from([left, right, top, bottom]),
            ..default()
        });
    } else if scale_by_tile {
        node.image_mode = NodeImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        };
    }

    node
}

/// 把核心层的颜色转换为 Bevy 的颜色。
pub fn to_bevy_color(color: Color) -> bevy::color::Color {
    bevy::color::Color::srgba(color.r, color.g, color.b, color.a)
}

/// 把两层着色相乘，用于叠加「图片自身颜色」与「颜色滤镜」。
///
/// Bevy 的 `Color` 之间没有乘法实现，这里按 sRGB 分量逐通道相乘。
pub fn multiply_tint(base: bevy::color::Color, overlay: bevy::color::Color) -> bevy::color::Color {
    let base = base.to_srgba();
    let overlay = overlay.to_srgba();

    bevy::color::Color::srgba(
        base.red * overlay.red,
        base.green * overlay.green,
        base.blue * overlay.blue,
        base.alpha * overlay.alpha,
    )
}

/// 把对象不透明度乘到颜色的 alpha 上。
///
/// 用「乘以基础色的 alpha」而不是「设为 alpha」：基础色本身可能带透明度
/// （例如半透明的图片），直接覆盖会把它丢掉。
pub fn apply_alpha(color: bevy::color::Color, alpha: f32) -> bevy::color::Color {
    let combined = (color.alpha() * alpha).clamp(0.0, 1.0);
    color.with_alpha(combined)
}

/// 依据文本属性构造字体设置。
pub fn text_font_for(
    object: &GObject,
    data: &fairygui_core::widget::TextFieldData,
    font: Option<&Handle<Font>>,
) -> TextFont {
    let _ = object;
    text_font_for_span(data, font, None, false, false)
}

/// Build the font of a rich-text span, combining field-wide and UBB styles.
pub fn text_font_for_span(
    data: &fairygui_core::widget::TextFieldData,
    font: Option<&Handle<Font>>,
    size: Option<i32>,
    bold: bool,
    italic: bool,
) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size.unwrap_or(data.font_size).max(1) as f32),
        // 引擎默认字体不含中日韩字形，注册过 `GuiFont` 时改用它。
        font: font
            .cloned()
            .map(bevy::text::FontSource::Handle)
            .unwrap_or_default(),
        weight: if data.bold || bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if data.italic || italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..default()
    }
}

/// FairyGUI's leading is extra space added to the font size.
pub fn text_line_height_for(
    data: &fairygui_core::widget::TextFieldData,
    size: Option<i32>,
) -> LineHeight {
    LineHeight::Px((size.unwrap_or(data.font_size) + data.leading).max(1) as f32)
}

/// Convert exported logical-pixel letter spacing to Bevy's text component.
pub fn text_letter_spacing_for(data: &fairygui_core::widget::TextFieldData) -> LetterSpacing {
    LetterSpacing::Px(data.letter_spacing as f32)
}

/// 依据文本属性构造布局设置。
pub fn text_layout_for(data: &fairygui_core::widget::TextFieldData) -> TextLayout {
    let justify = match data.align {
        1 => Justify::Center,
        2 => Justify::Right,
        _ => Justify::Left,
    };
    // AutoSize=Both lets the line grow to its intrinsic width. Other multiline
    // fields must wrap inside the exported width; WordOrCharacter also handles
    // CJK and words that are wider than the whole field.
    let linebreak = if data.single_line || data.auto_size == 1 {
        LineBreak::NoWrap
    } else {
        LineBreak::WordOrCharacter
    };
    TextLayout::new(justify, linebreak)
}

/// Align the measured text block inside its FairyGUI rectangle. With NoWrap,
/// Bevy's Justify only aligns lines inside the block, not inside the UI node.
/// Padding offsets the content without moving the object or its hit rectangle.
/// TextLayoutInfo.size is already logical pixels (unlike ComputedNode.size).
pub fn align_text(
    runtime: NonSend<crate::runtime::GuiRuntime>,
    mut query: Query<(
        &crate::runtime::GObjectRef,
        &bevy::text::TextLayoutInfo,
        &TextLayout,
        &mut Node,
    )>,
) {
    for (reference, layout, text_layout, mut node) in &mut query {
        let Some(object) = runtime.store().object(reference.0) else {
            continue;
        };
        let Some(data) = object.control().as_text_field() else {
            continue;
        };
        if layout.size.y <= 0.0 {
            continue;
        }
        let available = (Vec2::new(object.width(), object.height()) - layout.size).max(Vec2::ZERO);
        let factor = |alignment| match alignment {
            1 => 0.5,
            2 => 1.0,
            _ => 0.0,
        };
        let padding = UiRect {
            // Wrapped text is already justified by Parley inside its width.
            // Feeding that measured remainder back as padding changes the next
            // frame's wrapping constraint and can trip Parley's line breaker.
            left: Val::Px(if text_layout.linebreak == LineBreak::NoWrap {
                available.x * factor(data.align)
            } else {
                0.0
            }),
            top: Val::Px(available.y * factor(data.vertical_align)),
            right: Val::ZERO,
            bottom: Val::ZERO,
        };
        if node.padding != padding {
            node.padding = padding;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fairygui_core::object::ObjectStore;
    use fairygui_core::package::ObjectType;
    use fairygui_core::widget::TextFieldData;

    #[test]
    fn pivot_anchor_projection_matches_original_display_position_methods() {
        let mut state = None;
        let mut cases = 0;
        let mut points = 0;
        for line in include_str!("../../fairygui-core/tests/fixtures/pivot-reference.txt").lines() {
            let p: Vec<_> = line.split_whitespace().collect();
            match p.first().copied() {
                Some("case") => {
                    let v: Vec<f32> = p[1..].iter().map(|v| v.parse().unwrap()).collect();
                    let mut store = ObjectStore::new();
                    let id = store.create(ObjectType::Component);
                    store.set_size(id, v[6], v[7]);
                    store.set_xy(id, v[8], v[9]);
                    store.set_pivot(id, v[1], v[2], v[0] != 0.);
                    store.set_rotation(id, v[3]);
                    store.set_scale(id, v[4], v[5]);
                    state = Some((store, id));
                    cases += 1;
                }
                Some("point") => {
                    let v: Vec<f32> = p[1..].iter().map(|v| v.parse().unwrap()).collect();
                    let (store, id) = state.as_ref().unwrap();
                    let o = store.object(*id).unwrap();
                    let t = transform_for(o);
                    let (Val::Px(tx), Val::Px(ty)) = (t.translation.x, t.translation.y) else {
                        panic!("pixel translation");
                    };
                    let center = Vec2::new(o.x() + o.width() * 0.5, o.y() + o.height() * 0.5);
                    let actual = center
                        + Vec2::new(tx, ty)
                        + t.rotation
                            * (t.scale
                                * (Vec2::new(v[0], v[1]) - Vec2::new(o.width(), o.height()) * 0.5));
                    assert!(
                        actual.abs_diff_eq(Vec2::new(v[2], v[3]), 0.0001),
                        "{line}: {actual:?}"
                    );
                    points += 1;
                }
                _ => (),
            }
        }
        assert_eq!((cases, points), (72, 216));
    }

    #[test]
    fn identity_transform_does_not_shift_default_pivot() {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Component);
        store.set_size(id, 100.0, 40.0);

        let transform = transform_for(store.object(id).expect("object"));
        assert_eq!(transform.translation, bevy::ui::Val2::px(0.0, 0.0));
    }

    #[test]
    fn scaled_transform_keeps_a_top_left_pivot_in_place() {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Component);
        store.set_size(id, 100.0, 40.0);
        store.set_scale(id, 2.0, 2.0);

        let transform = transform_for(store.object(id).expect("object"));
        assert_eq!(transform.translation, bevy::ui::Val2::px(50.0, 20.0));
    }

    #[test]
    fn multiline_fixed_width_text_wraps_but_intrinsic_and_single_line_text_do_not() {
        let mut data = TextFieldData::default();
        assert_eq!(text_layout_for(&data).linebreak, LineBreak::WordOrCharacter);

        data.auto_size = 1;
        assert_eq!(text_layout_for(&data).linebreak, LineBreak::NoWrap);

        data.auto_size = 0;
        data.single_line = true;
        assert_eq!(text_layout_for(&data).linebreak, LineBreak::NoWrap);
    }

    #[test]
    fn text_style_and_spacing_are_mapped_to_bevy_components() {
        let mut store = ObjectStore::new();
        let id = store.create(ObjectType::Text);
        let object = store.object(id).unwrap();
        let data = TextFieldData {
            font_size: 22,
            leading: 12,
            letter_spacing: 3,
            bold: true,
            italic: true,
            ..Default::default()
        };

        let font = text_font_for(object, &data, None);
        assert_eq!(font.font_size, FontSize::Px(22.0));
        assert_eq!(font.weight, FontWeight::BOLD);
        assert_eq!(font.style, FontStyle::Italic);
        assert_eq!(text_line_height_for(&data, None), LineHeight::Px(34.0));
        assert_eq!(text_letter_spacing_for(&data), LetterSpacing::Px(3.0));
    }
}
