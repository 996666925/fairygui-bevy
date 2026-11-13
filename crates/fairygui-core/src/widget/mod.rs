//! 控件专有数据与描述解析。
//!
//! 所有控件共用 [`crate::object::GObject`] 里的通用属性，专有部分放在这里。
//! 容器以及由容器派生出来的控件（按钮、标签、进度条、滑动条、列表、树、下拉框、
//! 滚动条）都携带 [`ComponentData`]。
//!
//! 描述解析严格按官方 `setup_beforeAdd` / `setup_afterAdd` 的读取顺序复刻，
//! 因为描述是紧凑的字节流，任何字段顺序或宽度错误都会让后续字段全部错位。

mod combo;
mod component;
mod controller;
mod extend;
mod gear;
mod leaf;
mod list;
mod virtual_layout;
pub use virtual_layout::{VirtualGeometry, VirtualLayout};
pub(crate) mod relation;
mod tree;

pub use combo::{ComboBoxData, ComboPopup, popup_position};
pub use component::{
    ChildOverride, ChildrenRenderOrder, ComponentData, HitTestArea, create_from_package,
    create_from_package_with_registry,
};
pub use controller::{Controller, PackageEnv};
pub use extend::{ButtonData, ButtonMode, ButtonState, LabelData, ProgressData, SliderData};
pub use gear::{Gear, GearKind, GearTweenConfig, GearValue};
pub use leaf::{
    FillMethod, FlipType, GraphData, GraphType, GroupData, ImageData, LoaderData, MovieClipData,
    TextFieldData, TextInputData,
};
pub use list::{ListData, ListItemData, ListLayoutType, ListSelectionMode};
pub use tree::{TreeData, TreeNodeData};

/// 溢出处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowType {
    /// 不裁剪。
    #[default]
    Visible,
    /// 裁剪超出部分。
    Hidden,
    /// 裁剪并允许滚动。
    Scroll,
}

impl OverflowType {
    /// 由描述文件中的字节编号构造。
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::Hidden,
            2 => Self::Scroll,
            _ => Self::Visible,
        }
    }
}

/// 控件的专有数据。
#[derive(Debug, Clone, Default)]
pub enum ControlData {
    /// 没有专有数据。
    #[default]
    None,
    /// 图片。
    Image(ImageData),
    /// 装载器。
    Loader(LoaderData),
    /// 序列帧。
    MovieClip(MovieClipData),
    /// 矢量图形。
    Graph(GraphData),
    /// 分组。
    Group(GroupData),
    /// 静态文本。
    Text(TextFieldData),
    /// 输入框。
    TextInput(TextInputData),
    /// 普通容器。
    Component(Box<ComponentData>),
    /// 按钮。
    Button(Box<ButtonData>),
    /// 标签。
    Label(Box<LabelData>),
    /// 进度条。
    ProgressBar(Box<ProgressData>),
    /// 滑动条。
    Slider(Box<SliderData>),
    /// 列表。
    List(Box<ListData>),
    /// 树。
    Tree(Box<TreeData>),
    /// 下拉框。
    ComboBox(Box<ComboBoxData>),
    /// 滚动条。
    ScrollBar(Box<crate::scroll::ScrollBarData>),
}

impl ControlData {
    /// 种类名，用于日志与调试输出。
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Image(_) => "image",
            Self::Loader(_) => "loader",
            Self::MovieClip(_) => "movieClip",
            Self::Graph(_) => "graph",
            Self::Group(_) => "group",
            Self::Text(_) => "text",
            Self::TextInput(_) => "textInput",
            Self::Component(_) => "component",
            Self::Button(_) => "button",
            Self::Label(_) => "label",
            Self::ProgressBar(_) => "progressBar",
            Self::Slider(_) => "slider",
            Self::List(_) => "list",
            Self::Tree(_) => "tree",
            Self::ComboBox(_) => "comboBox",
            Self::ScrollBar(_) => "scrollBar",
        }
    }

    /// 取容器数据。
    ///
    /// 由容器派生的控件同样能取到。
    pub fn component(&self) -> Option<&ComponentData> {
        match self {
            Self::Component(data) => Some(data),
            Self::Button(data) => Some(&data.component),
            Self::Label(data) => Some(&data.component),
            Self::ProgressBar(data) => Some(&data.component),
            Self::Slider(data) => Some(&data.component),
            Self::List(data) => Some(&data.component),
            Self::Tree(data) => Some(&data.list.component),
            Self::ComboBox(data) => Some(&data.component),
            Self::ScrollBar(data) => Some(&data.component),
            _ => None,
        }
    }

    /// 可变地取容器数据。
    pub fn component_mut(&mut self) -> Option<&mut ComponentData> {
        match self {
            Self::Component(data) => Some(data),
            Self::Button(data) => Some(&mut data.component),
            Self::Label(data) => Some(&mut data.component),
            Self::ProgressBar(data) => Some(&mut data.component),
            Self::Slider(data) => Some(&mut data.component),
            Self::List(data) => Some(&mut data.component),
            Self::Tree(data) => Some(&mut data.list.component),
            Self::ComboBox(data) => Some(&mut data.component),
            Self::ScrollBar(data) => Some(&mut data.component),
            _ => None,
        }
    }

    /// 是否为容器（含由容器派生的控件）。
    pub fn is_container(&self) -> bool {
        self.component().is_some()
    }

    /// 取图片数据，非图片返回 `None`。
    pub fn as_image(&self) -> Option<&ImageData> {
        match self {
            Self::Image(data) => Some(data),
            _ => None,
        }
    }

    /// 取文本数据。
    ///
    /// 静态文本与输入框都能取到；输入框返回的是它内嵌的静态文本部分。
    pub fn as_text_field(&self) -> Option<&TextFieldData> {
        match self {
            Self::Text(data) => Some(data),
            Self::TextInput(data) => Some(&data.text_field),
            _ => None,
        }
    }

    /// 按对象类型给出默认的控件数据。
    ///
    /// 描述里的子对象有两种来源：引用包内条目，或者只写类型。后者没有自己的描述
    /// 数据（例如把一张图片直接扩展成按钮），但依然需要一份对应控件的默认数据，
    /// 否则它会被当成没有专有数据的对象，容器相关的行为也就无从谈起。
    pub fn for_kind(kind: crate::package::ObjectType) -> Self {
        use crate::package::ObjectType;
        match kind {
            ObjectType::Image => Self::Image(ImageData::default()),
            ObjectType::Loader => Self::Loader(LoaderData::default()),
            ObjectType::MovieClip => Self::MovieClip(MovieClipData::default()),
            ObjectType::Graph => Self::Graph(GraphData::default()),
            ObjectType::Group => Self::Group(GroupData::default()),
            ObjectType::Text => Self::Text(TextFieldData::default()),
            ObjectType::RichText => Self::Text(TextFieldData {
                html_enabled: true,
                ..Default::default()
            }),
            ObjectType::InputText => Self::TextInput(TextInputData::default()),
            ObjectType::Button => Self::Button(Box::default()),
            ObjectType::Label => Self::Label(Box::default()),
            ObjectType::ProgressBar => Self::ProgressBar(Box::default()),
            ObjectType::Slider => Self::Slider(Box::default()),
            ObjectType::List => Self::List(Box::default()),
            ObjectType::Tree => Self::Tree(Box::default()),
            ObjectType::ComboBox => Self::ComboBox(Box::default()),
            ObjectType::ScrollBar => Self::ScrollBar(Box::default()),
            ObjectType::Component => Self::Component(Box::default()),
            // 3D 装载器与未知类型没有专有数据。
            _ => Self::None,
        }
    }
}
