//! 界面对象的基础数据与属性解析。

use crate::binary::{ByteBuffer, StringTable};
use crate::error::{Error, Result};
use crate::geom::{Point, Size};
use crate::package::ObjectType;

slotmap::new_key_type! {
    /// 对象句柄。
    ///
    /// 内部带版本号：对象被回收后，旧句柄不会指向新对象，比裸索引安全。
    pub struct GObjectId;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageTransformState {
    pub pivot_offset: Point,
    pub position: Point,
}

/// 混合模式。
///
/// 保留编辑器编号；FairyGUI-Laya 的导出表仅将字节 2 映射为 Add，
/// 其余模式可通过运行时 API 设置，未识别的 Laya 名称使用普通混合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    None,
    Add,
    Multiply,
    Screen,
    Erase,
    Mask,
    Below,
    Off,
    On,
    Custom1,
    Custom2,
    Custom3,
}

impl BlendMode {
    /// 由描述文件中的字节编号构造。
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::None,
            2 => Self::Add,
            3 => Self::Multiply,
            4 => Self::Screen,
            5 => Self::Erase,
            6 => Self::Mask,
            7 => Self::Below,
            8 => Self::Off,
            9 => Self::On,
            10 => Self::Custom1,
            11 => Self::Custom2,
            12 => Self::Custom3,
            _ => Self::Normal,
        }
    }
}

/// 单个界面对象。
///
/// 字段刻意与官方实现保持一致：同一套属性被所有控件复用，控件专有的数据
/// 由上层控件结构承载。属性对外只读，修改一律通过 [`crate::object::ObjectStore`]
/// 的方法，以保证脏标记不会漏掉。
#[derive(Debug, Clone)]
pub struct GObject {
    pub(crate) kind: ObjectType,
    pub(crate) name: String,
    /// 编辑器导出的容器内实例 ID，与包资源条目 ID 和竞技场句柄分开保存。
    pub(crate) instance_id: Option<String>,
    pub(crate) package_item_id: Option<String>,
    pub(crate) source_package_id: Option<String>,

    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) init_width: f32,
    pub(crate) init_height: f32,
    pub(crate) min_width: f32,
    pub(crate) max_width: f32,
    pub(crate) min_height: f32,
    pub(crate) max_height: f32,
    pub(crate) source_width: f32,
    pub(crate) source_height: f32,
    pub(crate) scale_x: f32,
    pub(crate) image_transform: Option<ImageTransformState>,
    pub(crate) scale_y: f32,
    pub(crate) skew_x: f32,
    pub(crate) skew_y: f32,
    pub(crate) pivot_x: f32,
    pub(crate) pivot_y: f32,
    pub(crate) pivot_as_anchor: bool,
    pub(crate) rotation: f32,
    pub(crate) alpha: f32,
    pub(crate) visible: bool,
    pub(crate) display_gear: Option<DisplayGearState>,
    pub(crate) display_gear2: Option<(bool, i32)>,
    pub(crate) touchable: bool,
    pub(crate) sorting_order: i32,
    pub(crate) grayed: bool,
    /// Native JS leaves _grayed undefined until its first assignment. Public
    /// reads still return false, but that first false assignment runs handlers.
    pub(crate) grayed_initialized: bool,
    pub(crate) gear_locked: bool,
    pub(crate) under_construct: bool,
    pub(crate) blend_mode: BlendMode,
    pub(crate) blend_mode_set: bool,
    pub(crate) cache_as_bitmap: bool,
    pub(crate) mask_parent: Option<GObjectId>,
    pub(crate) mask: Option<GObjectId>,
    /// Sprite.mouseThrough can differ from !opaque after a mask assignment.
    pub(crate) component_mouse_through: Option<bool>,
    pub(crate) component_hit_test_prior: bool,

    pub(crate) parent: Option<GObjectId>,
    pub(crate) children: Vec<GObjectId>,
    pub(crate) group: Option<GObjectId>,

    pub(crate) data: Option<String>,
    pub(crate) tooltips: Option<String>,

    /// 待同步的属性变更位，见 [`crate::object::dirty`]。
    pub(crate) dirty: u32,
    /// 控件专有数据。所有控件共有的属性放在本结构上，专有部分放在这里。
    pub(crate) control: crate::widget::ControlData,
    /// 描述里记下的组编号：父容器内的子对象序号，指向一个分组控件。
    ///
    /// 绑定要等父容器的子对象都建好之后再做，因此这里先存着。
    pub(crate) group_index: Option<i16>,
    /// 随控制器页面变化的联动属性。
    pub(crate) gears: Vec<crate::widget::Gear>,
    /// 与目标对象的位置、尺寸关联。
    pub(crate) relations: crate::widget::relation::Relations,
    /// 颜色滤镜参数。
    pub(crate) color_filter: Option<crate::color_matrix::ColorFilterParams>,
}

impl GObject {
    /// 创建仅有默认属性的对象。
    pub fn new(kind: ObjectType) -> Self {
        Self {
            kind,
            name: String::new(),
            instance_id: None,
            package_item_id: None,
            source_package_id: None,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            init_width: 0.0,
            init_height: 0.0,
            min_width: 0.0,
            max_width: 0.0,
            min_height: 0.0,
            max_height: 0.0,
            source_width: 0.0,
            source_height: 0.0,
            scale_x: 1.0,
            image_transform: None,
            scale_y: 1.0,
            skew_x: 0.0,
            skew_y: 0.0,
            pivot_x: 0.0,
            pivot_y: 0.0,
            pivot_as_anchor: false,
            rotation: 0.0,
            alpha: 1.0,
            visible: true,
            display_gear: None,
            display_gear2: None,
            touchable: true,
            sorting_order: 0,
            grayed: false,
            grayed_initialized: false,
            gear_locked: false,
            under_construct: false,
            blend_mode: BlendMode::Normal,
            blend_mode_set: false,
            cache_as_bitmap: false,
            mask_parent: None,
            mask: None,
            component_mouse_through: None,
            component_hit_test_prior: false,
            parent: None,
            children: Vec::new(),
            group: None,
            data: None,
            tooltips: None,
            dirty: crate::object::dirty::ALL,
            control: crate::widget::ControlData::None,
            group_index: None,
            gears: Vec::new(),
            relations: crate::widget::relation::Relations::default(),
            color_filter: None,
        }
    }

    // ---- 标识 ----

    /// 对象种类。
    pub fn kind(&self) -> ObjectType {
        self.kind
    }

    /// 对象名。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 编辑器导出的实例 ID，只在所属容器内有意义。
    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }

    /// 来源的包条目编号。
    pub fn package_item_id(&self) -> Option<&str> {
        self.package_item_id.as_deref()
    }

    /// Package owning the object's resource and local URLs. Inline objects
    /// inherit the package containing their serialized instance data.
    pub fn source_package_id(&self) -> Option<&str> {
        self.source_package_id.as_deref()
    }

    // ---- 几何 ----

    /// 相对父对象的横坐标。
    pub fn x(&self) -> f32 {
        self.x
    }

    /// 相对父对象的纵坐标。
    pub fn y(&self) -> f32 {
        self.y
    }

    /// 相对父对象的位置。
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Nonzero orders form an ascending suffix after ordinary siblings.
    pub fn sorting_order(&self) -> i32 {
        self.sorting_order
    }

    /// 宽度。
    pub fn width(&self) -> f32 {
        self.width
    }

    /// 高度。
    pub fn height(&self) -> f32 {
        self.height
    }

    /// 尺寸。
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// 编辑器里设定的原始尺寸。
    pub fn init_size(&self) -> Size {
        Size::new(self.init_width, self.init_height)
    }

    /// 内容尺寸（容器未经拉伸时的尺寸）。
    pub fn source_size(&self) -> Size {
        Size::new(self.source_width, self.source_height)
    }

    /// 横向缩放。
    pub fn scale_x(&self) -> f32 {
        self.scale_x
    }

    /// 纵向缩放。
    pub fn scale_y(&self) -> f32 {
        self.scale_y
    }

    /// 横向斜切（角度）。
    pub fn skew_x(&self) -> f32 {
        self.skew_x
    }

    /// 纵向斜切（角度）。
    pub fn skew_y(&self) -> f32 {
        self.skew_y
    }

    /// 轴心，取值为尺寸的比例（0~1）。
    pub fn pivot(&self) -> Point {
        Point::new(self.pivot_x, self.pivot_y)
    }

    /// 轴心是否同时作为锚点（影响定位方式）。
    pub fn pivot_as_anchor(&self) -> bool {
        self.pivot_as_anchor
    }

    /// 旋转角度（度）。
    pub fn rotation(&self) -> f32 {
        self.rotation
    }

    /// 不透明度。
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    // ---- 显示 ----

    /// 是否可见。
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// 控制器的显示连接状态，包含尚未释放的显示锁。
    pub fn display_connected(&self) -> bool {
        let connected = self
            .display_gear
            .as_ref()
            .is_none_or(|gear| gear.visible || gear.locks > 0);
        match self.display_gear2 {
            Some((visible, 0)) => connected && visible,
            Some((visible, _)) => connected || visible,
            None => connected,
        }
    }

    /// 自身是否绘制。显式 visible 和控制器显示齿轮分别生效。
    pub fn display_visible(&self) -> bool {
        self.visible && self.display_connected()
    }

    /// 是否响应输入。
    pub fn touchable(&self) -> bool {
        self.touchable
    }

    /// 是否置灰。
    pub fn grayed(&self) -> bool {
        self.grayed
    }

    /// 混合模式。
    pub fn blend_mode(&self) -> BlendMode {
        self.blend_mode
    }

    /// An unset Sprite blend inherits its parent; explicit Normal overrides it.
    pub fn blend_mode_is_set(&self) -> bool {
        self.blend_mode_set
    }

    /// Whether Sprite.cacheAs requests an isolated bitmap subtree.
    pub fn cache_as_bitmap(&self) -> bool {
        self.cache_as_bitmap
    }

    /// Sprite currently using this object as its ordinary alpha mask.
    pub fn mask_parent(&self) -> Option<GObjectId> {
        self.mask_parent
    }

    /// Stable mask Sprite reference, independent of child ordering.
    pub fn mask(&self) -> Option<GObjectId> {
        self.mask
    }

    // ---- 树 ----

    /// 父对象。
    pub fn parent(&self) -> Option<GObjectId> {
        self.parent
    }

    /// 子对象列表。
    pub fn children(&self) -> &[GObjectId] {
        &self.children
    }

    /// Exported size restrictions. Zero means that axis has no upper bound.
    pub fn max_size(&self) -> Size {
        Size::new(self.max_width, self.max_height)
    }

    /// 所属的组。
    pub fn group(&self) -> Option<GObjectId> {
        self.group
    }

    // ---- 附加信息 ----

    /// 控件专有数据。
    pub fn control(&self) -> &crate::widget::ControlData {
        &self.control
    }

    /// 随控制器页面变化的联动属性。
    pub fn gears(&self) -> &[crate::widget::Gear] {
        &self.gears
    }

    /// 描述里记下的组编号，即父容器内那个分组控件的序号。
    ///
    /// 绑定完成之前它是唯一的线索，绑定之后仍保留以便排查。
    pub fn group_index(&self) -> Option<i16> {
        self.group_index
    }

    /// 颜色滤镜参数。
    pub fn color_filter(&self) -> Option<crate::color_matrix::ColorFilterParams> {
        self.color_filter
    }

    /// 编辑器里填写的自定义数据。
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }

    /// 提示文本。
    pub fn tooltips(&self) -> Option<&str> {
        self.tooltips.as_deref()
    }

    // ---- 描述解析 ----

    /// 读取所有控件共有的属性，对应官方的 `setup_beforeAdd`。
    ///
    /// 描述以 `beginPos` 为起点，自成一个小型的「索引表 + 数据块」结构。
    pub fn setup_before_add(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        table: &StringTable,
        begin_pos: usize,
    ) -> Result<()> {
        if !buffer.seek(begin_pos, 0)? {
            return Err(Error::MissingBlock { block: 0 });
        }

        // 跳过 type(1) + src(2) + pkgId(2)，这几项在创建对象时已经用过。
        buffer.skip(5);

        self.instance_id = buffer.read_s(table)?;
        self.name = buffer.read_s(table)?.unwrap_or_default();

        self.x = buffer.read_i32()? as f32;
        self.y = buffer.read_i32()? as f32;

        if buffer.read_bool()? {
            self.width = buffer.read_i32()? as f32;
            self.height = buffer.read_i32()? as f32;
            self.init_width = self.width;
            self.init_height = self.height;
        }

        if buffer.read_bool()? {
            self.min_width = buffer.read_i32()? as f32;
            self.max_width = buffer.read_i32()? as f32;
            self.min_height = buffer.read_i32()? as f32;
            self.max_height = buffer.read_i32()? as f32;
        }

        if buffer.read_bool()? {
            self.scale_x = buffer.read_f32()?;
            self.scale_y = buffer.read_f32()?;
        }

        if buffer.read_bool()? {
            self.skew_x = buffer.read_f32()?;
            self.skew_y = buffer.read_f32()?;
        }

        if buffer.read_bool()? {
            self.pivot_x = buffer.read_f32()?;
            self.pivot_y = buffer.read_f32()?;
            self.pivot_as_anchor = buffer.read_bool()?;
        }

        let alpha = buffer.read_f32()?;
        if alpha != 1.0 {
            self.alpha = alpha;
        }

        let rotation = buffer.read_f32()?;
        if rotation != 0.0 {
            self.rotation = rotation;
        }

        if !buffer.read_bool()? {
            self.visible = false;
        }
        if !buffer.read_bool()? {
            self.touchable = false;
        }
        if buffer.read_bool()? {
            self.grayed = true;
            self.grayed_initialized = true;
        }

        // FairyGUI-Laya's exported BlendMode table only maps byte 2 to lighter.
        // Unmapped bytes do not assign the Sprite property or cancel inheritance.
        if buffer.read_u8()? == 2 {
            self.blend_mode = BlendMode::Add;
            self.blend_mode_set = true;
        }

        let filter = buffer.read_i8()?;
        if filter == 1 {
            // 顺序是亮度、对比度、饱和度、色相。
            self.color_filter = Some(crate::color_matrix::ColorFilterParams {
                brightness: buffer.read_f32()?,
                contrast: buffer.read_f32()?,
                saturation: buffer.read_f32()?,
                hue: buffer.read_f32()?,
            });
        }

        if let Some(data) = buffer.read_s(table)? {
            self.data = Some(data);
        }

        self.dirty |= crate::object::dirty::ALL;
        Ok(())
    }

    /// 读取创建完成之后才能设置的属性，对应官方的 `setup_afterAdd`。
    ///
    /// 齿轮（Gear）部分依赖控制器与齿轮系统，由上层在齿轮模块落地后补齐。
    pub fn setup_after_add(
        &mut self,
        buffer: &mut ByteBuffer<'_>,
        table: &StringTable,
        begin_pos: usize,
    ) -> Result<()> {
        if buffer.seek(begin_pos, 1)? {
            if let Some(tooltips) = buffer.read_s(table)? {
                self.tooltips = Some(tooltips);
            }

            // 组编号：它是**父容器内的子对象序号**，指向一个分组控件。
            // 具体绑定要等父容器的子对象都建好之后再做，这里先记下序号。
            let group_index = buffer.read_i16()?;
            if group_index >= 0 {
                self.group_index = Some(group_index);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DisplayGearState {
    pub controller_index: i32,
    pub generation: u64,
    pub visible: bool,
    pub locks: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_object_uses_sane_defaults() {
        let object = GObject::new(ObjectType::Component);
        assert_eq!(object.kind(), ObjectType::Component);
        assert_eq!(object.scale_x(), 1.0);
        assert_eq!(object.scale_y(), 1.0);
        assert_eq!(object.alpha(), 1.0);
        assert!(object.visible());
        assert!(object.touchable());
        assert!(!object.grayed());
        assert_eq!(object.pivot(), Point::ZERO);
        assert!(object.children().is_empty());
        assert_eq!(object.parent(), None);
    }

    #[test]
    fn blend_mode_maps_editor_codes() {
        assert_eq!(BlendMode::from_code(0), BlendMode::Normal);
        assert_eq!(BlendMode::from_code(2), BlendMode::Add);
        assert_eq!(BlendMode::from_code(5), BlendMode::Erase);
        assert_eq!(BlendMode::from_code(200), BlendMode::Normal);
    }

    #[test]
    fn setup_before_add_reads_geometry_and_flags() {
        use crate::object::dirty;

        // 逐字段构造：skip(5) + id + name + xy + 尺寸 + 四个可选块 + alpha + 旋转 + 三个开关 + 混合 + 滤镜 + data
        let mut data = Vec::new();
        // 描述块自带索引表：1 个块、使用 4 字节偏移，块 0 紧跟在索引表之后。
        data.extend_from_slice(&[1, 0, 0, 0, 0, 6]);
        data.extend_from_slice(&[0, 0, 0, 0, 0]); // 被跳过的 5 字节
        data.extend_from_slice(&[0x00, 0x02]); // id：字符串表索引 2
        data.extend_from_slice(&[0xFF, 0xFE]); // name = null
        data.extend_from_slice(&100i32.to_be_bytes()); // x
        data.extend_from_slice(&(-20i32).to_be_bytes()); // y
        data.push(1); // 有尺寸
        data.extend_from_slice(&320i32.to_be_bytes());
        data.extend_from_slice(&240i32.to_be_bytes());
        data.push(0); // 无最小/最大尺寸
        data.push(1); // 有缩放
        data.extend_from_slice(&2.0f32.to_be_bytes());
        data.extend_from_slice(&3.0f32.to_be_bytes());
        data.push(0); // 无斜切
        data.push(1); // 有轴心
        data.extend_from_slice(&0.5f32.to_be_bytes());
        data.extend_from_slice(&0.5f32.to_be_bytes());
        data.push(1); // 轴心作为锚点
        data.extend_from_slice(&0.5f32.to_be_bytes()); // alpha
        data.extend_from_slice(&45.0f32.to_be_bytes()); // rotation
        data.push(1); // visible
        data.push(0); // touchable = false
        data.push(1); // grayed
        data.push(2); // blend = Add
        data.push(0); // 无滤镜
        data.extend_from_slice(&[0xFF, 0xFD]); // data = 空串

        let mut buffer = ByteBuffer::new(&data);
        let mut table = StringTable::new();
        table.push(None);
        table.push(Some("placeholder".to_string()));
        table.push(Some("id".to_string()));

        let mut object = GObject::new(ObjectType::Image);
        object
            .setup_before_add(&mut buffer, &table, 0)
            .expect("setup should succeed");

        // 描述里的编号是对象在容器内的编号，不会覆盖来源条目，因此这里仍为空。
        assert_eq!(object.package_item_id(), None);
        assert_eq!(object.instance_id(), Some("id"));
        assert_eq!(object.name(), "");
        assert_eq!(object.position(), Point::new(100.0, -20.0));
        assert_eq!(object.size(), Size::new(320.0, 240.0));
        assert_eq!(object.init_size(), Size::new(320.0, 240.0));
        assert_eq!((object.scale_x(), object.scale_y()), (2.0, 3.0));
        assert_eq!(object.pivot(), Point::new(0.5, 0.5));
        assert!(object.pivot_as_anchor());
        assert_eq!(object.alpha(), 0.5);
        assert_eq!(object.rotation(), 45.0);
        assert!(object.visible());
        assert!(!object.touchable());
        assert!(object.grayed());
        assert_eq!(object.blend_mode(), BlendMode::Add);
        assert_eq!(object.data(), Some(""));
        assert_eq!(object.dirty, dirty::ALL);
    }

    #[test]
    fn setup_before_add_keeps_defaults_when_optional_blocks_absent() {
        let mut data = Vec::new();
        // 描述块自带索引表：1 个块、使用 4 字节偏移，块 0 紧跟在索引表之后。
        data.extend_from_slice(&[1, 0, 0, 0, 0, 6]);
        data.extend_from_slice(&[0, 0, 0, 0, 0]);
        data.extend_from_slice(&[0xFF, 0xFE]); // id = null
        data.extend_from_slice(&[0xFF, 0xFE]); // name = null
        data.extend_from_slice(&0i32.to_be_bytes());
        data.extend_from_slice(&0i32.to_be_bytes());
        data.push(0); // 无尺寸
        data.push(0); // 无最小/最大
        data.push(0); // 无缩放
        data.push(0); // 无斜切
        data.push(0); // 无轴心
        data.extend_from_slice(&1.0f32.to_be_bytes());
        data.extend_from_slice(&0.0f32.to_be_bytes());
        data.push(0); // visible = false
        data.push(1); // touchable
        data.push(0); // grayed
        data.push(0); // blend
        data.push(0); // filter
        data.extend_from_slice(&[0xFF, 0xFE]); // data = null

        let mut buffer = ByteBuffer::new(&data);
        let mut object = GObject::new(ObjectType::Component);
        object
            .setup_before_add(&mut buffer, &StringTable::new(), 0)
            .expect("setup should succeed");

        assert_eq!(object.scale_x(), 1.0, "缺省缩放应保持 1");
        assert_eq!(object.alpha(), 1.0, "缺省不透明度应保持 1");
        assert!(!object.visible());
        assert!(object.data().is_none());
    }
}
