//! 非容器控件的数据与描述解析。
//!
//! 每个控件的字段顺序都严格对应官方 `setup_beforeAdd` 的读取顺序。

use crate::binary::{ByteBuffer, StringTable};
use crate::color::Color;
use crate::error::Result;
use crate::object::GObject;
use crate::package::ObjectType;
use crate::widget::ControlData;

/// 填充方式。
///
/// 描述里的编号与官方一致：`0` 不填充、`1` 横向、`2` 纵向、
/// `3`/`4`/`5` 分别是 90 度、180 度与 360 度径向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillMethod {
    /// 不填充，整块显示。
    #[default]
    None,
    /// 从一侧向另一侧横向展开。
    Horizontal,
    /// 从一侧向另一侧纵向展开。
    Vertical,
    /// 四分之一圆。
    Radial90,
    /// 半圆。
    Radial180,
    /// 整圆。
    Radial360,
}

impl FillMethod {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Horizontal,
            2 => Self::Vertical,
            3 => Self::Radial90,
            4 => Self::Radial180,
            5 => Self::Radial360,
            _ => Self::None,
        }
    }

    /// 是否为径向填充。
    ///
    /// 径向通过 FillUtils 多边形裁剪绘制区域，不修改对象布局尺寸。
    pub const fn is_radial(self) -> bool {
        matches!(self, Self::Radial90 | Self::Radial180 | Self::Radial360)
    }
}

/// GImage 翻转方式，编号与导出格式一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum FlipType {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}
impl FlipType {
    pub const fn from_code(value: u8) -> Self {
        match value {
            1 => Self::Horizontal,
            2 => Self::Vertical,
            3 => Self::Both,
            _ => Self::None,
        }
    }
    pub const fn scale(self) -> (f32, f32) {
        (
            if matches!(self, Self::Horizontal | Self::Both) {
                -1.
            } else {
                1.
            },
            if matches!(self, Self::Vertical | Self::Both) {
                -1.
            } else {
                1.
            },
        )
    }
}

/// 图片控件的数据。
#[derive(Debug, Clone, Default)]
pub struct ImageData {
    /// 着色，为空表示不着色。
    pub color: Option<Color>,
    /// 翻转标志，按位组合。
    pub flip: FlipType,
    /// 填充方式，0 表示不填充。
    pub fill_method: u8,
    /// 填充起点。
    pub fill_origin: u8,
    /// 填充是否顺时针。
    pub fill_clockwise: bool,
    /// 填充比例。运行期保留 JS Number 精度；包内 Float32 解码后提升为 f64。
    pub fill_amount: f64,
}

impl ImageData {
    /// 填充方式。
    ///
    /// 与同名字段区分：这里把描述里的编号解释成枚举。
    pub fn fill_method(&self) -> FillMethod {
        FillMethod::from_code(self.fill_method)
    }

    /// 当前的填充比例，夹在 `0..=1`。
    pub fn fill_ratio(&self) -> f32 {
        self.fill_amount.clamp(0.0, 1.0) as f32
    }

    /// 横向填充是否从右侧开始。起点不同，留下的那一块贴的是不同的边。
    pub fn fills_from_right(&self) -> bool {
        self.fill_origin != 0 && self.fill_origin != 2
    }

    /// 纵向填充是否从底部开始。
    pub fn fills_from_bottom(&self) -> bool {
        self.fill_origin != 0 && self.fill_origin != 2
    }

    /// 按填充参数算出实际应当显示的区域（相对控件左上角）。
    ///
    /// 横纵填充按比例收缩，并依据起点决定贴哪一边——从右往左填充与从左往右
    /// 填充，留下的那块位置正好相反，这一点很容易漏。
    ///
    /// 径向返回完整画布范围，精确裁剪轮廓由 `fill::fill_polygon` 提供。
    /// 这是绘制区域的辅助查询，不能用它缩小对象的布局或输入矩形。
    pub fn fill_rect(&self, size: crate::geom::Size) -> crate::geom::Rect {
        if self.fill_method() == FillMethod::None {
            return crate::geom::Rect::new(0.0, 0.0, size.width, size.height);
        }
        let ratio = self.fill_ratio();

        if ratio <= 0.0 {
            return crate::geom::Rect::new(0.0, 0.0, 0.0, 0.0);
        }

        match self.fill_method() {
            FillMethod::Horizontal => {
                let width = size.width * ratio;
                let x = if self.fills_from_right() {
                    size.width - width
                } else {
                    0.0
                };
                crate::geom::Rect::new(x, 0.0, width, size.height)
            }
            FillMethod::Vertical => {
                let height = size.height * ratio;
                let y = if self.fills_from_bottom() {
                    size.height - height
                } else {
                    0.0
                };
                crate::geom::Rect::new(0.0, y, size.width, height)
            }
            // 径向裁剪需要完整多边形，而非一个矩形。
            _ => crate::geom::Rect::new(0.0, 0.0, size.width, size.height),
        }
    }

    /// 按填充参数算出实际应当显示的尺寸。
    ///
    /// 只需要尺寸、不关心位置时用它；需要完整区域请用 [`Self::fill_rect`]。
    pub fn fill_size(&self, size: crate::geom::Size) -> crate::geom::Size {
        match self.fill_method() {
            FillMethod::None
            | FillMethod::Radial90
            | FillMethod::Radial180
            | FillMethod::Radial360 => size,
            FillMethod::Horizontal => {
                crate::geom::Size::new(size.width * self.fill_ratio(), size.height)
            }
            FillMethod::Vertical => {
                crate::geom::Size::new(size.width, size.height * self.fill_ratio())
            }
        }
    }
}

/// 序列帧控件的数据。
#[derive(Debug, Clone, Default)]
pub struct MovieClipData {
    /// 运行期播放速度覆盖，为空时为 1。
    pub time_scale: Option<f32>,
    /// 着色，为空表示不着色。
    pub color: Option<Color>,
    /// 当前帧。
    pub frame: i32,
    /// 是否正在播放。
    pub playing: bool,
}

/// 装载器的数据。
#[derive(Debug, Clone)]
pub struct LoaderData {
    /// 运行期序列帧速度覆盖，为空时为 1。
    pub time_scale: Option<f32>,
    /// 资源地址。
    pub url: Option<String>,
    /// 水平对齐。
    pub align: i32,
    /// 垂直对齐。
    pub vertical_align: i32,
    /// 填充方式。
    pub fill: i32,
    /// 是否只缩小不放大。
    pub shrink_only: bool,
    /// 是否按内容自动尺寸。
    pub auto_size: bool,
    /// 加载失败时是否显示提示图。
    pub show_error_sign: bool,
    /// 序列帧是否播放。
    pub playing: bool,
    /// 序列帧当前帧。
    pub frame: i32,
    /// 着色。
    pub color: Option<Color>,
    /// 填充方式。
    pub fill_method: u8,
    /// 填充起点。
    pub fill_origin: u8,
    /// 填充是否顺时针。
    pub fill_clockwise: bool,
    /// 填充比例。运行期保留 JS Number 精度；包内 Float32 解码后提升为 f64。
    pub fill_amount: f64,
    /// 是否跟随源图尺寸（格式 7 及以上才有）。
    pub use_resize: bool,
}

impl Default for LoaderData {
    fn default() -> Self {
        Self {
            time_scale: None,
            url: None,
            align: 0,
            vertical_align: 0,
            fill: 0,
            shrink_only: false,
            auto_size: false,
            show_error_sign: true,
            playing: false,
            frame: 0,
            color: None,
            fill_method: 0,
            fill_origin: 0,
            fill_clockwise: false,
            fill_amount: 0.0,
            use_resize: false,
        }
    }
}

impl LoaderData {
    /// Layout the untrimmed content canvas using FairyGUI's six fill modes.
    /// Images and movies are sized directly; use_resize only affects components.
    pub fn content_rect(
        &self,
        source: crate::geom::Size,
        viewport: crate::geom::Size,
    ) -> crate::geom::Rect {
        use crate::geom::Rect;
        if source.is_zero() {
            return Rect::ZERO;
        }
        let (mut width, mut height) = (source.width, source.height);
        if self.auto_size {
            return Rect::new(0.0, 0.0, width, height);
        }
        if self.fill != 0 {
            let (mut sx, mut sy) = (viewport.width / width, viewport.height / height);
            match self.fill {
                1 => {
                    sx = sx.min(sy);
                    sy = sx;
                }
                2 => sx = sy,
                3 => sy = sx,
                5 => {
                    sx = sx.max(sy);
                    sy = sx;
                }
                _ => {}
            }
            if self.shrink_only {
                sx = sx.min(1.0);
                sy = sy.min(1.0);
            }
            width *= sx;
            height *= sy;
        }
        let x = match self.align {
            1 => ((viewport.width - width) / 2.0).floor(),
            2 => viewport.width - width,
            _ => 0.0,
        };
        let y = match self.vertical_align {
            1 => ((viewport.height - height) / 2.0).floor(),
            2 => viewport.height - height,
            _ => 0.0,
        };
        Rect::new(x, y, width, height)
    }
}

#[cfg(test)]
mod loader_layout_tests {
    use super::*;
    use crate::geom::{Rect, Size};

    #[test]
    fn fill_modes_keep_original_size_fit_match_dimensions_stretch_and_cover() {
        let source = Size::new(100.0, 50.0);
        let viewport = Size::new(300.0, 200.0);
        for (fill, expected) in [
            (0, (100.0, 50.0)),
            (1, (300.0, 150.0)),
            (2, (400.0, 200.0)),
            (3, (300.0, 150.0)),
            (4, (300.0, 200.0)),
            (5, (400.0, 200.0)),
        ] {
            let data = LoaderData {
                fill,
                ..Default::default()
            };
            let rect = data.content_rect(source, viewport);
            assert_eq!(
                rect.size(),
                Size::new(expected.0, expected.1),
                "fill {fill}"
            );
        }
    }

    #[test]
    fn alignment_uses_floor_for_centering_and_allows_cover_overflow() {
        let mut data = LoaderData {
            fill: 0,
            align: 1,
            vertical_align: 1,
            ..Default::default()
        };
        assert_eq!(
            data.content_rect(Size::new(100.0, 50.0), Size::new(301.0, 201.0)),
            Rect::new(100.0, 75.0, 100.0, 50.0)
        );
        data.fill = 5;
        assert_eq!(
            data.content_rect(Size::new(100.0, 50.0), Size::new(300.0, 200.0)),
            Rect::new(-50.0, 0.0, 400.0, 200.0)
        );
        data.align = 2;
        assert_eq!(
            data.content_rect(Size::new(100.0, 50.0), Size::new(300.0, 200.0))
                .x,
            -100.0
        );
    }

    #[test]
    fn shrink_only_caps_enlargement_and_auto_size_uses_source_dimensions() {
        let mut data = LoaderData {
            fill: 4,
            shrink_only: true,
            align: 2,
            vertical_align: 2,
            ..Default::default()
        };
        assert_eq!(
            data.content_rect(Size::new(100.0, 50.0), Size::new(300.0, 200.0)),
            Rect::new(200.0, 150.0, 100.0, 50.0)
        );
        data.auto_size = true;
        assert_eq!(
            data.content_rect(Size::new(100.0, 50.0), Size::new(300.0, 200.0)),
            Rect::new(0.0, 0.0, 100.0, 50.0)
        );
        assert_eq!(
            data.content_rect(Size::ZERO, Size::new(300.0, 200.0)),
            Rect::ZERO
        );
    }
}

/// 矢量图形的数据。
#[derive(Debug, Clone, Default)]
pub struct GraphData {
    /// 图形种类，0 表示空。
    pub graph_type: i32,
    /// 线宽。
    pub line_size: i32,
    /// 线色。
    pub line_color: Color,
    /// 填充色。
    pub fill_color: Color,
    /// 圆角半径，四个角。
    pub corner_radius: Option<[f32; 4]>,
    /// 多边形顶点坐标，依次为 x、y。
    pub polygon_points: Option<Vec<f32>>,
    /// 正多边形的边数。
    pub sides: i32,
    /// 起始角度。
    pub start_angle: f32,
    /// 多边形各边到中心的距离。
    pub distances: Option<Vec<f32>>,
}

impl PartialEq for GraphData {
    fn eq(&self, other: &Self) -> bool {
        // NaN distances are a supported native spelling of the default 1.
        // Retained snapshots must compare equal without rebuilding every frame.
        fn number(a: f32, b: f32) -> bool {
            a == b || a.is_nan() && b.is_nan()
        }
        fn values(a: Option<&[f32]>, b: Option<&[f32]>) -> bool {
            match (a, b) {
                (Some(a), Some(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(&a, &b)| number(a, b))
                }
                (None, None) => true,
                _ => false,
            }
        }
        self.graph_type == other.graph_type
            && self.line_size == other.line_size
            && self.line_color == other.line_color
            && self.fill_color == other.fill_color
            && values(
                self.corner_radius.as_ref().map(|v| v.as_slice()),
                other.corner_radius.as_ref().map(|v| v.as_slice()),
            )
            && values(
                self.polygon_points.as_deref(),
                other.polygon_points.as_deref(),
            )
            && self.sides == other.sides
            && number(self.start_angle, other.start_angle)
            && values(self.distances.as_deref(), other.distances.as_deref())
    }
}

/// 图形种类。取值与描述里写入的编号一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphType {
    /// 空，不绘制任何东西。
    #[default]
    Empty,
    /// 矩形。带圆角时是圆角矩形。
    Rect,
    /// 原版名为 Ellipse，实际绘制半径为宽度一半的圆。
    Ellipse,
    /// 多边形。
    Polygon,
    /// 正多边形。
    RegularPolygon,
}

impl GraphType {
    /// 由描述中的编号构造。认不出的编号按空处理。
    pub const fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Rect,
            2 => Self::Ellipse,
            3 => Self::Polygon,
            4 => Self::RegularPolygon,
            _ => Self::Empty,
        }
    }

    /// 是否能用节点自身的形状表达。
    ///
    /// 矩形与椭圆可以：前者就是节点本身，后者靠把四角圆角拉满得到。
    /// 多边形需要真正的矢量绘制，不在其中。
    pub const fn is_basic_shape(self) -> bool {
        matches!(self, Self::Rect | Self::Ellipse)
    }
}

impl GraphData {
    /// 图形种类。
    pub fn graph_kind(&self) -> GraphType {
        GraphType::from_code(self.graph_type)
    }

    /// 是否需要填充。
    pub fn has_fill(&self) -> bool {
        self.fill_color.a > 0.0
    }

    /// 是否需要描边。线宽为零或线色全透明时都不描。
    pub fn has_stroke(&self) -> bool {
        self.line_size > 0 && self.line_color.a > 0.0
    }

    /// 四角圆角半径。没有圆角数据时全为零。
    ///
    /// 顺序与描述一致：左上、右上、左下、右下。
    pub fn corner_radii(&self) -> [f32; 4] {
        self.corner_radius.unwrap_or([0.0; 4])
    }
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn graph_types_map_from_codes() {
        assert_eq!(GraphType::from_code(0), GraphType::Empty);
        assert_eq!(GraphType::from_code(1), GraphType::Rect);
        assert_eq!(GraphType::from_code(2), GraphType::Ellipse);
        assert_eq!(
            GraphType::from_code(9),
            GraphType::Empty,
            "认不出的编号按空处理，而不是当成某种图形"
        );
    }

    #[test]
    fn only_basic_shapes_are_expressible_by_a_node() {
        assert!(GraphType::Rect.is_basic_shape());
        assert!(GraphType::Ellipse.is_basic_shape());
        assert!(
            !GraphType::Polygon.is_basic_shape(),
            "多边形需要真正的矢量绘制"
        );
        assert!(!GraphType::RegularPolygon.is_basic_shape());
        assert!(!GraphType::Empty.is_basic_shape());
    }

    #[test]
    fn fill_and_stroke_are_detected() {
        let mut graph = GraphData::default();
        assert!(!graph.has_fill());
        assert!(!graph.has_stroke());

        graph.fill_color = Color::from_rgba8(255, 0, 0, 255);
        assert!(graph.has_fill());

        // 有线色但线宽为零时仍然不描边——画不出来。
        graph.line_color = Color::from_rgba8(0, 0, 0, 255);
        assert!(!graph.has_stroke());

        graph.line_size = 2;
        assert!(graph.has_stroke());
    }

    #[test]
    fn fill_rect_respects_the_origin() {
        let size = crate::geom::Size::new(100.0, 50.0);

        // 从左往右填充一半：留下的那块贴左边。
        let mut image = ImageData {
            fill_method: FillMethod::Horizontal as u8,
            fill_origin: 0,
            fill_amount: 0.5,
            ..Default::default()
        };

        let rect = image.fill_rect(size);
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.width, 50.0);

        // 从右往左：同样是一半，但贴的是右边。
        image.fill_origin = 1;
        let rect = image.fill_rect(size);
        assert_eq!(rect.x, 50.0, "从右侧填充时留下的那块应当贴右边");
        assert_eq!(rect.width, 50.0);
    }

    #[test]
    fn ordinary_images_keep_their_full_rect_without_fill_settings() {
        let rect = ImageData::default().fill_rect(crate::geom::Size::new(100.0, 50.0));
        assert_eq!((rect.width, rect.height), (100.0, 50.0));
    }

    #[test]
    fn zero_ratio_yields_an_empty_rect() {
        let size = crate::geom::Size::new(100.0, 50.0);

        // 横向、纵向都该如此：比例归零就没有东西可显示。
        for method in [1u8, 2u8] {
            let image = ImageData {
                fill_method: method,
                fill_amount: 0.0,
                ..Default::default()
            };
            let rect = image.fill_rect(size);
            assert_eq!(
                (rect.width, rect.height),
                (0.0, 0.0),
                "比例为零时不该显示任何东西"
            );
        }
    }

    #[test]
    fn corner_radii_default_to_zero() {
        let mut graph = GraphData::default();
        assert_eq!(graph.corner_radii(), [0.0; 4], "没有圆角数据就是直角");

        graph.corner_radius = Some([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(graph.corner_radii(), [1.0, 2.0, 3.0, 4.0]);
    }
}

/// 分组的数据。
#[derive(Debug, Clone, Default)]
pub struct GroupData {
    /// 排列方式。
    pub layout: i32,
    /// 行间距。
    pub line_gap: i32,
    /// 列间距。
    pub column_gap: i32,
    /// 是否排除不可见对象（格式 2 及以上）。
    pub exclude_invisibles: bool,
    /// 是否禁用自动尺寸（格式 2 及以上）。
    pub auto_size_disabled: bool,
    /// 布局参考的主对象编号（格式 2 及以上）。
    pub main_grid_index: i32,
}

/// 静态文本的数据。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFieldData {
    /// 字体名，为空表示使用默认字体。
    pub font: Option<String>,
    /// 字号。
    pub font_size: i32,
    /// 文字颜色。
    pub color: Color,
    /// 水平对齐，0 左 1 中 2 右。
    pub align: i32,
    /// 垂直对齐，0 上 1 中 2 下。
    pub vertical_align: i32,
    /// 行距。
    pub leading: i32,
    /// 字间距。
    pub letter_spacing: i32,
    /// 是否解析 UBB 标记。
    pub ubb_enabled: bool,
    /// Native Text.html. RichText objects enable it independently of UBB.
    pub html_enabled: bool,
    /// 自动尺寸策略。
    pub auto_size: i32,
    /// 下划线。
    pub underline: bool,
    /// 斜体。
    pub italic: bool,
    /// 粗体。
    pub bold: bool,
    /// 是否单行。
    pub single_line: bool,
    /// 描边颜色。
    pub stroke_color: Option<Color>,
    /// 描边宽度。
    pub stroke: f32,
    /// 是否包含模板变量。
    pub template_vars: bool,
    /// 文本内容，在描述块 6 中读取。
    pub text: String,
}

/// 输入框的数据。
#[derive(Debug, Clone)]
pub struct TextInputData {
    /// 静态文本部分的属性。
    pub text_field: TextFieldData,
    /// 占位文本。
    pub prompt_text: Option<String>,
    /// 输入限制。
    pub restrict: Option<String>,
    /// 最大字符数。
    pub max_length: i32,
    /// 键盘类型。
    pub keyboard_type: i32,
    /// 是否密码框。
    pub password: bool,
    /// Native Input.editable; independent of visibility/touchability.
    pub editable: bool,
    /// Native Input prompt color. UBB remove mode does not execute color handlers.
    pub prompt_color: Color,
}

impl Default for TextInputData {
    fn default() -> Self {
        Self {
            text_field: TextFieldData::default(),
            prompt_text: None,
            restrict: None,
            max_length: 0,
            keyboard_type: 0,
            password: false,
            editable: true,
            prompt_color: Color::from_rgba8(169, 169, 169, 255),
        }
    }
}

/// 读取颜色：四个 8 位分量，顺序为红、绿、蓝、透明度。
fn read_color(buffer: &mut ByteBuffer<'_>) -> Result<Color> {
    let r = buffer.read_u8()?;
    let g = buffer.read_u8()?;
    let b = buffer.read_u8()?;
    let a = buffer.read_u8()?;
    Ok(Color::from_rgba8(r, g, b, a))
}

/// 读取控件在描述块 5 中的专有属性。
pub(crate) fn setup_before_add(
    object: &mut GObject,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<()> {
    let kind = object.kind();
    let version = buffer.version();

    let control = match kind {
        ObjectType::Image => ControlData::Image(read_image(buffer, begin_pos)?),
        ObjectType::MovieClip => ControlData::MovieClip(read_movie_clip(buffer, begin_pos)?),
        ObjectType::Loader => ControlData::Loader(read_loader(buffer, table, begin_pos, version)?),
        ObjectType::Graph => ControlData::Graph(read_graph(buffer, begin_pos)?),
        ObjectType::Group => ControlData::Group(read_group(buffer, begin_pos, version)?),
        ObjectType::Text | ObjectType::RichText => {
            let mut text = read_text_field(buffer, table, begin_pos)?;
            text.html_enabled = kind == ObjectType::RichText;
            ControlData::Text(text)
        }
        ObjectType::InputText => {
            let text_field = read_text_field(buffer, table, begin_pos)?;
            let mut input = read_text_input(buffer, table, begin_pos)?;
            input.text_field = text_field;
            ControlData::TextInput(input)
        }
        // 其余类型不在这里处理。容器类控件的专有数据来自组件自身的描述，
        // 若在这里回落到「无数据」，会把它们已经设置好的数据覆盖掉。
        _ => return Ok(()),
    };

    object.control = control;
    if let ControlData::Image(data) = &object.control
        && data.flip != FlipType::None
    {
        (object.scale_x, object.scale_y) = data.flip.scale();
        object.initialize_image_transform();
    }
    Ok(())
}

/// 读取控件在描述块 6/9 中的专有属性。
pub(crate) fn setup_after_add(
    object: &mut GObject,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<()> {
    let kind = object.kind();

    if matches!(
        kind,
        ObjectType::Text | ObjectType::RichText | ObjectType::InputText
    ) && buffer.seek(begin_pos, 6)?
        && let Some(text) = buffer.read_s(table)?
    {
        match &mut object.control {
            ControlData::Text(data) => data.text = text,
            ControlData::TextInput(data) => data.text_field.text = text,
            _ => {}
        }
    }

    Ok(())
}

fn read_image(buffer: &mut ByteBuffer<'_>, begin_pos: usize) -> Result<ImageData> {
    let mut data = ImageData::default();
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    if buffer.read_bool()? {
        data.color = Some(read_color(buffer)?);
    }
    data.flip = FlipType::from_code(buffer.read_u8()?);
    data.fill_method = buffer.read_u8()?;
    if data.fill_method != 0 {
        data.fill_origin = buffer.read_u8()?;
        data.fill_clockwise = buffer.read_bool()?;
        data.fill_amount = f64::from(buffer.read_f32()?);
    }

    Ok(data)
}

#[cfg(test)]
mod image_setup_tests {
    use super::*;
    #[test]
    fn exported_flip_overrides_serialized_scale_but_none_preserves_it() {
        for code in 0..4 {
            // Six short-offset blocks; only block 5 has content at byte 14.
            let mut bytes = vec![6, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 14];
            bytes.extend([0, code, 0]);
            let mut object = GObject::new(ObjectType::Image);
            object.width = 100.;
            object.height = 60.;
            object.x = 30.;
            object.y = 40.;
            object.scale_x = 2.;
            object.scale_y = 0.5;
            setup_before_add(
                &mut object,
                &mut ByteBuffer::new(&bytes),
                &StringTable::new(),
                0,
            )
            .unwrap();
            let expected = FlipType::from_code(code);
            assert_eq!(object.control().as_image().unwrap().flip, expected);
            let scale = if code == 0 {
                (2., 0.5)
            } else {
                expected.scale()
            };
            assert_eq!((object.scale_x(), object.scale_y()), scale);
            if code != 0 {
                assert_eq!(
                    object.image_display_position(),
                    Some(crate::Point::new(
                        30. + if scale.0 == -1. { 100. } else { 0. },
                        40. + if scale.1 == -1. { 60. } else { 0. }
                    ))
                );
            }
        }
    }
}

fn read_movie_clip(buffer: &mut ByteBuffer<'_>, begin_pos: usize) -> Result<MovieClipData> {
    let mut data = MovieClipData::default();
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    if buffer.read_bool()? {
        data.color = Some(read_color(buffer)?);
    }
    buffer.read_u8()?; // flip，官方读后丢弃
    data.frame = buffer.read_i32()?;
    data.playing = buffer.read_bool()?;

    Ok(data)
}

fn read_loader(
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    version: i32,
) -> Result<LoaderData> {
    let mut data = LoaderData::default();
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    data.url = buffer.read_s(table)?;
    data.align = i32::from(buffer.read_i8()?);
    data.vertical_align = i32::from(buffer.read_i8()?);
    data.fill = i32::from(buffer.read_i8()?);
    data.shrink_only = buffer.read_bool()?;
    data.auto_size = buffer.read_bool()?;
    data.show_error_sign = buffer.read_bool()?;
    data.playing = buffer.read_bool()?;
    data.frame = buffer.read_i32()?;
    if buffer.read_bool()? {
        data.color = Some(read_color(buffer)?);
    }
    data.fill_method = buffer.read_u8()?;
    if data.fill_method != 0 {
        data.fill_origin = buffer.read_u8()?;
        data.fill_clockwise = buffer.read_bool()?;
        data.fill_amount = f64::from(buffer.read_f32()?);
    }
    if version >= 7 {
        data.use_resize = buffer.read_bool()?;
    }

    Ok(data)
}

fn read_graph(buffer: &mut ByteBuffer<'_>, begin_pos: usize) -> Result<GraphData> {
    let mut data = GraphData::default();
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    data.graph_type = i32::from(buffer.read_i8()?);
    if data.graph_type == 0 {
        return Ok(data);
    }

    data.line_size = buffer.read_i32()?;
    data.line_color = read_color(buffer)?;
    data.fill_color = read_color(buffer)?;

    if buffer.read_bool()? {
        let mut radius = [0.0f32; 4];
        for value in &mut radius {
            *value = buffer.read_f32()?;
        }
        data.corner_radius = Some(radius);
    }

    if data.graph_type == 3 {
        let count = buffer.read_i16()?;
        let mut points = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count {
            points.push(buffer.read_f32()?);
        }
        data.polygon_points = Some(points);
    } else if data.graph_type == 4 {
        data.sides = i32::from(buffer.read_i16()?);
        data.start_angle = buffer.read_f32()?;
        let count = buffer.read_i16()?;
        if count > 0 {
            let mut distances = Vec::with_capacity(count as usize);
            for _ in 0..count {
                distances.push(buffer.read_f32()?);
            }
            data.distances = Some(distances);
        }
    }

    Ok(data)
}

fn read_group(buffer: &mut ByteBuffer<'_>, begin_pos: usize, version: i32) -> Result<GroupData> {
    let mut data = GroupData::default();
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    data.layout = i32::from(buffer.read_i8()?);
    data.line_gap = buffer.read_i32()?;
    data.column_gap = buffer.read_i32()?;

    if version >= 2 {
        data.exclude_invisibles = buffer.read_bool()?;
        data.auto_size_disabled = buffer.read_bool()?;
        data.main_grid_index = i32::from(buffer.read_i16()?);
    }

    Ok(data)
}

fn read_text_field(
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<TextFieldData> {
    let mut data = TextFieldData {
        color: Color::WHITE,
        ..Default::default()
    };
    if !buffer.seek(begin_pos, 5)? {
        return Ok(data);
    }

    data.font = buffer.read_s(table)?;
    data.font_size = i32::from(buffer.read_i16()?);
    data.color = read_color(buffer)?;
    data.align = i32::from(buffer.read_i8()?);
    data.vertical_align = i32::from(buffer.read_i8()?);
    data.leading = i32::from(buffer.read_i16()?);
    data.letter_spacing = i32::from(buffer.read_i16()?);
    data.ubb_enabled = buffer.read_bool()?;
    data.auto_size = i32::from(buffer.read_i8()?);
    data.underline = buffer.read_bool()?;
    data.italic = buffer.read_bool()?;
    data.bold = buffer.read_bool()?;
    data.single_line = buffer.read_bool()?;

    if buffer.read_bool()? {
        data.stroke_color = Some(read_color(buffer)?);
        // 官方把宽度存为「实际宽度 - 1」。
        data.stroke = buffer.read_f32()? + 1.0;
    }

    if buffer.read_bool()? {
        // 阴影：颜色 4 字节 + 偏移两个浮点 + 模糊两个浮点。
        buffer.skip(12);
    }

    if buffer.read_bool()? {
        data.template_vars = true;
    }

    Ok(data)
}

fn read_text_input(
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
) -> Result<TextInputData> {
    let mut data = TextInputData::default();
    if !buffer.seek(begin_pos, 4)? {
        return Ok(data);
    }

    data.prompt_text = buffer.read_s(table)?;
    data.restrict = buffer.read_s(table)?;

    let max_length = buffer.read_i32()?;
    if max_length != 0 {
        data.max_length = max_length;
    }

    let keyboard_type = buffer.read_i32()?;
    if keyboard_type != 0 {
        data.keyboard_type = keyboard_type;
    }

    if buffer.read_bool()? {
        data.password = true;
    }

    Ok(data)
}
