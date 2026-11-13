//! 扩展控件（按钮、标签、进度条、滑动条）的数据与描述解析。
//!
//! 这些控件都继承自容器，因此各自持有 [`ComponentData`]。它们的描述分两部分：
//!
//! - **扩展数据**来自组件自身的描述块 6（图标、声音、显示模式等），在构造期读取；
//! - **实例数据**来自父容器描述里该子对象的块 6（标题、颜色、当前值等），在实例化时读取。
//!
//! 两者的首字节含义不同：前者是控件自己的某个枚举，后者是「对象类型」校验值。

use crate::binary::{ByteBuffer, StringTable};
use crate::color::Color;
use crate::error::Result;
use crate::package::ObjectType;
use crate::widget::component::ComponentData;

/// 按钮的显示状态。
///
/// 状态由名为 `button` 的控制器承载：切换状态就是切换该控制器的页面。
/// 编辑器里按钮的各状态页正是按这些名字命名的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ButtonState {
    /// 常态。
    #[default]
    Up,
    /// 按下。
    Down,
    /// 悬停。
    Over,
    /// 选中且悬停。
    SelectedOver,
    /// 禁用。
    Disabled,
    /// 选中且禁用。
    SelectedDisabled,
    /// 选中且按下。
    SelectedDown,
    /// 选中。
    SelectedUp,
}

impl ButtonState {
    /// 状态对应的控制器页面名。
    pub const fn page_name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Over => "over",
            Self::SelectedOver => "selectedOver",
            Self::Disabled => "disabled",
            Self::SelectedDisabled => "selectedDisabled",
            Self::SelectedDown => "selectedDown",
            Self::SelectedUp => "selectedUp",
        }
    }

    /// 承载按钮状态的控制器名。
    pub const fn controller_name() -> &'static str {
        "button"
    }
}

/// 按钮的交互模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonMode {
    /// 普通按钮：按下再松开即触发。
    #[default]
    Common,
    /// 复选按钮：点击后保持在选中状态。
    Check,
    /// 单选按钮：同组内只能选中一个。
    Radio,
}

impl ButtonMode {
    /// 由描述中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::Check,
            2 => Self::Radio,
            _ => Self::Common,
        }
    }
}

/// 按钮数据。
#[derive(Debug, Clone)]
pub struct ButtonData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 交互模式。
    pub mode: ButtonMode,
    /// 标题。
    pub title: String,
    /// 选中时的标题。
    pub selected_title: Option<String>,
    /// 图标地址。
    pub icon: Option<String>,
    /// 选中时的图标地址。
    pub selected_icon: Option<String>,
    /// 标题颜色。
    pub title_color: Option<Color>,
    /// 标题字号，0 表示沿用默认。
    pub title_font_size: i32,
    /// 关联的控制器编号，`-1` 表示未关联。
    pub related_controller: i32,
    /// 关联控制器中应当在选中时切换到的页面编号。
    pub related_page_id: Option<String>,
    /// 点击音效。
    pub sound: Option<String>,
    /// 点击音效音量。
    pub sound_volume_scale: f32,
    /// 按下效果类型。
    pub down_effect: i32,
    /// 按下效果强度。
    pub down_effect_value: f32,
    /// 是否处于选中状态。
    pub selected: bool,
    /// 是否允许点击改变选中状态。
    pub change_state_on_click: bool,
    /// 输入层最近提交的状态，用于选中状态变化时保留悬停外观。
    pub interaction_state: ButtonState,
    /// Pointer flags are separate: rolling out while held must not release effects.
    pub hovered: bool,
    pub pressed: bool,
    /// Guards multiplicative down-effect scale changes against repeated states.
    pub down_scaled: bool,
    /// Runtime-assigned linked popup/window; it is not an exported resource URL.
    pub linked_popup: Option<crate::object::GObjectId>,
}

impl Default for ButtonData {
    fn default() -> Self {
        Self {
            component: ComponentData::default(),
            mode: ButtonMode::Common,
            title: String::new(),
            selected_title: None,
            icon: None,
            selected_icon: None,
            title_color: None,
            title_font_size: 0,
            related_controller: -1,
            related_page_id: None,
            sound: None,
            sound_volume_scale: 1.0,
            down_effect: 0,
            down_effect_value: 0.8,
            selected: false,
            change_state_on_click: true,
            interaction_state: ButtonState::Up,
            hovered: false,
            pressed: false,
            down_scaled: false,
            linked_popup: None,
        }
    }
}

/// 标签数据。
#[derive(Debug, Clone, Default)]
pub struct LabelData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 标题。
    pub title: String,
    /// 图标地址。
    pub icon: Option<String>,
    /// 标题颜色。
    pub title_color: Option<Color>,
    /// 标题字号，0 表示沿用默认。
    pub title_font_size: i32,
}

/// 进度条数据。
#[derive(Debug, Clone)]
pub struct ProgressData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 当前值。
    pub value: i32,
    /// 最大值。
    pub max: i32,
    /// 最小值（格式 2 及以上才有）。
    pub min: i32,
    /// 标题显示方式。
    pub title_type: i32,
    /// 是否反向。
    pub reverse: bool,
}

impl Default for ProgressData {
    fn default() -> Self {
        Self {
            component: ComponentData::default(),
            value: 50,
            max: 100,
            min: 0,
            title_type: 0,
            reverse: false,
        }
    }
}

/// 滑动条数据。
#[derive(Debug, Clone)]
pub struct SliderData {
    /// 容器部分的数据。
    pub component: ComponentData,
    /// 当前值。
    pub value: f64,
    /// 最大值。
    pub max: f64,
    /// 最小值（格式 2 及以上才有）。
    pub min: f64,
    /// 是否整数值。
    pub whole_numbers: bool,
    /// 是否反向。
    pub reverse: bool,
    pub title_type: i32,
    pub change_on_click: bool,
    pub can_drag: bool,
}

impl Default for SliderData {
    fn default() -> Self {
        Self {
            component: ComponentData::default(),
            value: 50.,
            max: 100.,
            min: 0.,
            whole_numbers: false,
            reverse: false,
            title_type: 0,
            change_on_click: true,
            can_drag: true,
        }
    }
}

/// 读取按钮的扩展数据（组件描述块 6）。
pub(crate) fn read_button_extension(
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
) -> Result<(ButtonMode, Option<String>, f32, i32, f32)> {
    if !buffer.seek(0, 6)? {
        return Ok((ButtonMode::Common, None, 1.0, 0, 0.8));
    }

    let mode = ButtonMode::from_code(buffer.read_i8()?);
    let sound = buffer.read_s(table)?;
    let sound_volume_scale = buffer.read_f32()?;
    let down_effect = i32::from(buffer.read_i8()?);
    let down_effect_value = buffer.read_f32()?;

    Ok((
        mode,
        sound,
        sound_volume_scale,
        down_effect,
        down_effect_value,
    ))
}

/// 读取按钮的实例数据（父容器中该子对象的块 6）。
pub(crate) fn setup_button(
    data: &mut ButtonData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    expected_type: ObjectType,
) -> Result<()> {
    if !buffer.seek(begin_pos, 6)? {
        return Ok(());
    }

    // 首字节是对象类型校验值，用于确认实例数据与条目匹配。
    let declared = buffer.read_i8()?;
    if ObjectType::from_code(declared as u8) != expected_type {
        return Ok(());
    }

    if let Some(title) = buffer.read_s(table)? {
        data.title = title;
    }
    data.selected_title = buffer.read_s(table)?;
    data.icon = buffer.read_s(table)?;
    data.selected_icon = buffer.read_s(table)?;

    if buffer.read_bool()? {
        data.title_color = Some(read_color(buffer)?);
    }

    let font_size = buffer.read_i32()?;
    if font_size != 0 {
        data.title_font_size = font_size;
    }

    data.related_controller = i32::from(buffer.read_i16()?);

    data.related_page_id = buffer.read_s(table)?;
    if let Some(sound) = buffer.read_s(table)? {
        data.sound = Some(sound);
    }

    if buffer.read_bool()? {
        data.sound_volume_scale = buffer.read_f32()?;
    }

    data.selected = buffer.read_bool()?;
    Ok(())
}

/// 读取标签的实例数据。
pub(crate) fn setup_label(
    data: &mut LabelData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    expected_type: ObjectType,
) -> Result<()> {
    if !buffer.seek(begin_pos, 6)? {
        return Ok(());
    }

    let declared = buffer.read_i8()?;
    if ObjectType::from_code(declared as u8) != expected_type {
        return Ok(());
    }

    if let Some(title) = buffer.read_s(table)? {
        data.title = title;
    }
    data.icon = buffer.read_s(table)?;

    if buffer.read_bool()? {
        data.title_color = Some(read_color(buffer)?);
    }

    let font_size = buffer.read_i32()?;
    if font_size != 0 {
        data.title_font_size = font_size;
    }

    // 标签内部如果是输入框，还会带一段输入框属性；否则是一段固定长度的保留区。
    if buffer.read_bool()? {
        if expected_type == ObjectType::InputText {
            let _prompt = buffer.read_s(table)?;
            let _restrict = buffer.read_s(table)?;
            let _max_length = buffer.read_i32()?;
            let _keyboard_type = buffer.read_i32()?;
            let _password = buffer.read_bool()?;
        } else {
            buffer.skip(13);
        }
    }

    Ok(())
}

/// 读取进度条的实例数据。
pub(crate) fn setup_progress(
    data: &mut ProgressData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    expected_type: ObjectType,
) -> Result<()> {
    if !buffer.seek(begin_pos, 6)? {
        return Ok(());
    }

    let declared = buffer.read_i8()?;
    if ObjectType::from_code(declared as u8) != expected_type {
        return Ok(());
    }

    data.value = buffer.read_i32()?;
    data.max = buffer.read_i32()?;
    if buffer.version() >= 2 {
        data.min = buffer.read_i32()?;
    }

    let _ = table;
    Ok(())
}

/// 读取滑动条的实例数据。
pub(crate) fn setup_slider(
    data: &mut SliderData,
    buffer: &mut ByteBuffer<'_>,
    table: &StringTable,
    begin_pos: usize,
    expected_type: ObjectType,
) -> Result<()> {
    if !buffer.seek(begin_pos, 6)? {
        return Ok(());
    }

    let declared = buffer.read_i8()?;
    if ObjectType::from_code(declared as u8) != expected_type {
        return Ok(());
    }

    data.value = f64::from(buffer.read_i32()?);
    data.max = f64::from(buffer.read_i32()?);
    if buffer.version() >= 2 {
        data.min = f64::from(buffer.read_i32()?);
    }

    let _ = table;
    Ok(())
}

fn read_color(buffer: &mut ByteBuffer<'_>) -> Result<Color> {
    let r = buffer.read_u8()?;
    let g = buffer.read_u8()?;
    let b = buffer.read_u8()?;
    let a = buffer.read_u8()?;
    Ok(Color::from_rgba8(r, g, b, a))
}
