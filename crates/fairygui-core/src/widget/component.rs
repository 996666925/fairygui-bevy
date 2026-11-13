//! 容器控件的数据与对象树构建。
//!
//! 组件描述自身也是一个「索引表 + 数据块」结构，官方用到的块为：
//!
//! | 块 | 内容 |
//! | --- | --- |
//! | 0 | 容器尺寸、边距、轴心、溢出方式 |
//! | 1 | 控制器 |
//! | 2 | 子对象 |
//! | 3 | 关联关系 |
//! | 4 | 整体属性（命中测试、遮罩、属性覆盖） |
//! | 5 | 过渡动画 |
//! | 6 | 扩展控件的专有数据 |
//! | 7 | 滚动设置 |

use tracing::debug;

use crate::error::{Error, Result};
use crate::geom::Margin;
use crate::object::{GObjectId, ObjectStore};
use crate::package::{FuiPackage, LoadContext, ObjectType, PackageItemType};
use crate::widget::{ControlData, OverflowType, leaf};

/// Paint ordering of a component's logical children. It never changes their
/// indices, controller bindings, relations or list selection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum ChildrenRenderOrder {
    #[default]
    Ascent = 0,
    Descent = 1,
    /// Paint the prefix forward, then the suffix backward, with the apex last.
    Arch = 2,
}
impl ChildrenRenderOrder {
    pub const fn from_code(code: i8) -> Self {
        match code {
            1 => Self::Descent,
            2 => Self::Arch,
            _ => Self::Ascent,
        }
    }
}

/// 命中测试区域。
#[derive(Debug, Clone)]
pub enum HitTestArea {
    /// ChildHitArea installed by setMask, retaining a stable shape identity.
    MaskChild { object: GObjectId, reversed: bool },
    /// 以另一个子对象的范围作为命中区域。
    Child {
        /// 子对象编号。
        child_index: usize,
    },
    /// 使用像素级命中数据。
    Pixel {
        /// 提供像素数据的图片条目编号。
        item_id: String,
        /// 横向偏移。
        offset_x: i32,
        /// 纵向偏移。
        offset_y: i32,
    },
}

/// 子对象的属性覆盖，由描述块 4 提供。
#[derive(Debug, Clone)]
pub struct ChildOverride {
    /// 目标子对象的路径。
    pub target: String,
    /// 属性编号。
    pub property_id: i32,
    /// 属性值。
    pub value: String,
}

/// 容器的数据。
#[derive(Debug, Clone, Default)]
pub struct ComponentData {
    pub children_render_order: ChildrenRenderOrder,
    /// Raw native apex, clamped to 0..=child count only when painting.
    pub apex_index: i32,
    /// 边距，用于内容区域的自动尺寸计算。
    pub margin: Margin,
    /// 溢出处理方式。
    pub overflow: OverflowType,
    /// 是否整体不参与命中测试。
    pub opaque: bool,
    /// 作为遮罩的子对象编号。
    pub mask_child: Option<usize>,
    /// Exported/runtime reversed mask flag.
    pub mask_reversed: bool,
    /// 命中测试区域。
    pub hit_test: Option<HitTestArea>,
    /// Instance block-4 page-controller index in the original parent. The store
    /// resolves it to an owner identity; reparenting does not rebind it.
    pub page_controller: Option<usize>,
    /// 子对象属性覆盖。
    pub overrides: Vec<ChildOverride>,
    /// 控制器。切换页面会驱动齿轮联动与列表分页。
    pub controllers: Vec<super::controller::Controller>,
    /// 滚动容器。只有可滚动时才有。
    pub scroll_pane: Option<crate::scroll::ScrollPane>,
    /// 过渡动画。
    pub transitions: Vec<crate::transition::Transition>,
}

impl ComponentData {
    /// 按名称取控制器。
    pub fn controller(&self, name: &str) -> Option<&super::controller::Controller> {
        self.controllers.iter().find(|item| item.name == name)
    }

    /// 按序号取控制器。
    pub fn controller_at(&self, index: usize) -> Option<&super::controller::Controller> {
        self.controllers.get(index)
    }

    /// 控制器名称对应的序号。
    pub fn controller_index_of(&self, name: &str) -> Option<usize> {
        self.controllers.iter().position(|item| item.name == name)
    }

    /// 按名称可变地取控制器。
    pub fn controller_mut(&mut self, name: &str) -> Option<&mut super::controller::Controller> {
        self.controllers.iter_mut().find(|item| item.name == name)
    }
}

/// 依据包条目创建对象，并把描述填充进去。
///
/// 这是运行时的入口：从包里的一个组件条目出发，构造出完整的对象树。
pub fn create_from_package(
    store: &mut ObjectStore,
    package: &FuiPackage,
    item_index: usize,
    ctx: &LoadContext,
) -> Result<GObjectId> {
    create_with_context(
        store,
        package,
        item_index,
        ctx,
        &mut Construction {
            registry: None,
            active: Vec::new(),
            branch: package
                .branches()
                .get(ctx.branch_index as usize)
                .cloned()
                .unwrap_or_default(),
        },
    )
}

/// Build resources across registered dependency packages. Each package maps the
/// current branch by name, since branch indices are package-local.
pub fn create_from_package_with_registry(
    store: &mut ObjectStore,
    package: &FuiPackage,
    item_index: usize,
    ctx: &LoadContext,
    registry: &crate::package::PackageRegistry,
) -> Result<GObjectId> {
    create_with_context(
        store,
        package,
        item_index,
        ctx,
        &mut Construction {
            registry: Some(registry),
            active: Vec::new(),
            branch: package
                .branches()
                .get(ctx.branch_index as usize)
                .cloned()
                .unwrap_or_default(),
        },
    )
}

struct Construction<'a> {
    registry: Option<&'a crate::package::PackageRegistry>,
    active: Vec<(String, usize)>,
    // Retain the original branch name even through an intermediate package
    // without that branch. Branch indices are never transferable across packages.
    branch: String,
}

fn create_with_context(
    store: &mut ObjectStore,
    package: &FuiPackage,
    item_index: usize,
    ctx: &LoadContext,
    construction: &mut Construction<'_>,
) -> Result<GObjectId> {
    let index = package.resolve_branch(item_index, ctx);
    let item = package
        .item(index)
        .ok_or_else(|| Error::ItemNotFound(format!("#{item_index}")))?;

    let identity = (package.id().to_owned(), index);
    if construction.active.contains(&identity) {
        return Err(Error::invalid_data(
            0,
            format!("cyclic component reference: {} / {}", package.id(), item.id),
        ));
    }
    construction.active.push(identity);
    let object = store.create(item.object_type);
    store.object_mut(object).unwrap().source_package_id = Some(package.id().into());
    store.set_source(object, item.object_type, Some(item.id.clone()));
    store.set_init_size(object, item.width, item.height);
    store.set_source_size(object, item.width, item.height);
    store.set_size(object, item.width, item.height);

    let result = if item.item_type == PackageItemType::Component {
        store.object_mut(object).unwrap().under_construct = true;
        build_component(store, package, index, ctx, object, construction)
    } else {
        Ok(())
    };
    construction.active.pop();
    if let Err(error) = result {
        store.remove(object);
        return Err(error);
    }
    Ok(object)
}

/// 读取组件描述，填充到已经创建好的对象上。
fn build_component(
    store: &mut ObjectStore,
    package: &FuiPackage,
    item_index: usize,
    ctx: &LoadContext,
    object: GObjectId,
    construction: &mut Construction<'_>,
) -> Result<()> {
    let item = package
        .item(item_index)
        .ok_or_else(|| Error::ItemNotFound(format!("#{item_index}")))?;

    let mut buffer = package
        .raw_data(item)
        .ok_or_else(|| Error::invalid_data(0, "component item has no raw data"))?;

    let mut component = ComponentData::default();

    // ---- 块 0：容器自身的属性 ----
    if !buffer.seek(0, 0)? {
        return Err(Error::MissingBlock { block: 0 });
    }

    let source_width = buffer.read_i32()? as f32;
    let source_height = buffer.read_i32()? as f32;
    store.set_source_size(object, source_width, source_height);
    store.set_init_size(object, source_width, source_height);
    store.set_size(object, source_width, source_height);

    if buffer.read_bool()? {
        let min_width = buffer.read_i32()? as f32;
        let max_width = buffer.read_i32()? as f32;
        let min_height = buffer.read_i32()? as f32;
        let max_height = buffer.read_i32()? as f32;
        if let Some(target) = store.object_mut(object) {
            target.min_width = min_width;
            target.max_width = max_width;
            target.min_height = min_height;
            target.max_height = max_height;
        }
    }

    if buffer.read_bool()? {
        let pivot_x = buffer.read_f32()?;
        let pivot_y = buffer.read_f32()?;
        let as_anchor = buffer.read_bool()?;
        store.set_pivot(object, pivot_x, pivot_y, as_anchor);
    }

    if buffer.read_bool()? {
        component.margin.top = buffer.read_i32()? as f32;
        component.margin.bottom = buffer.read_i32()? as f32;
        component.margin.left = buffer.read_i32()? as f32;
        component.margin.right = buffer.read_i32()? as f32;
    }

    let overflow = OverflowType::from_code(buffer.read_i8()?);
    component.overflow = overflow;

    // 可滚动时，滚动配置放在块 7；读完要回到原位置继续。
    if overflow == OverflowType::Scroll {
        let saved = buffer.position();
        if buffer.seek(0, 7)? {
            let mut pane = crate::scroll::ScrollPane::default();
            pane.setup_with_defaults(&mut buffer, package.string_table(), ctx.scroll_defaults)?;
            pane.set_view_size(crate::geom::Size::new(source_width, source_height));
            component.scroll_pane = Some(pane);
        }
        buffer.set_position(saved);
    }

    if buffer.read_bool()? {
        // 裁剪的软化范围，渲染层暂不支持。
        buffer.skip(8);
    }

    // ---- 块 1：控制器 ----
    let env = super::controller::PackageEnv {
        branch: package
            .branches()
            .get(ctx.branch_index.max(0) as usize)
            .cloned()
            .unwrap_or_default(),
        vars: std::collections::HashMap::new(),
    };

    if buffer.seek(0, 1)? {
        let count = buffer.read_i16()?.max(0) as usize;
        for _ in 0..count {
            let next_pos = i32::from(buffer.read_i16()?);
            let end = buffer.resolve_relative(next_pos)?;

            let mut controller = super::controller::Controller::default();
            controller.setup(&mut buffer, package.string_table(), &env)?;
            component.controllers.push(controller);

            buffer.set_position(end);
        }
    }

    // ---- 块 2：子对象 ----
    if !buffer.seek(0, 2)? {
        return Err(Error::MissingBlock { block: 2 });
    }

    let child_count = buffer.read_i16()?.max(0) as usize;
    let mut children = Vec::with_capacity(child_count);

    for _ in 0..child_count {
        let data_len = buffer.read_i16()?.max(0) as usize;
        let child_pos = buffer.position();

        // 子对象描述开头是「种类 + 源条目编号 + 源包编号」，共 5 字节。
        if !buffer.seek(child_pos, 0)? {
            return Err(Error::MissingBlock { block: 0 });
        }
        let kind_code = buffer.read_i8()?;
        let source_id = buffer.read_s(package.string_table())?;
        let source_package_id = buffer.read_s(package.string_table())?;
        let registry = construction.registry;
        let source_package = match source_package_id.as_deref() {
            None => Some(package),
            Some(id) if id == package.id() => Some(package),
            Some(id) => registry
                .and_then(|registry| registry.get_by_id(id))
                .map(|p| p.as_ref()),
        };
        // An explicit foreign package must never fall back to an equal local
        // item ID. Native GComponent uses a plain typed object when absent.
        let source = source_id.as_deref().and_then(|id| {
            let owner = source_package?;
            Some((owner, owner.item_index_by_id(id)?))
        });
        let child = if let Some((owner, source_index)) = source {
            let owner_ctx = LoadContext {
                branch_index: owner.branch_index_of(&construction.branch),
                ..*ctx
            };
            create_with_context(store, owner, source_index, &owner_ctx, construction)?
        } else {
            let child = store.create(ObjectType::from_code(kind_code as u8));
            store.object_mut(child).unwrap().source_package_id = Some(package.id().into());
            child
        };
        let kind = store.object(child).unwrap().kind();
        // Attach before parsing instance data so a later parse failure can
        // reclaim the whole partially constructed subtree transactionally.
        store.set_parent(child, Some(object));
        store.object_mut(child).unwrap().under_construct = true;

        // 只写了类型的子对象没有自己的描述数据，补一份对应控件的默认数据。
        let needs_default = store
            .object(child)
            .is_some_and(|object| matches!(object.control(), ControlData::None));
        if needs_default && let Some(target) = store.object_mut(child) {
            target.control = ControlData::for_kind(kind);
        }

        let built_size = store.object(child).map(|o| o.size()).unwrap_or_default();
        let built_grayed = store.object(child).unwrap().grayed();
        // 先读与父节点无关的通用属性。
        {
            let table = package.string_table();
            let child_object = store
                .object_mut(child)
                .ok_or_else(|| Error::invalid_data(child_pos, "child object vanished"))?;
            child_object.setup_before_add(&mut buffer, table, child_pos)?;
            leaf::setup_before_add(child_object, &mut buffer, table, child_pos)?;
        }

        if !built_grayed && store.object(child).unwrap().grayed() {
            store.initialize_grayed(child);
        }

        // Instance dimensions override the source component dimensions. Apply
        // that change through the store so backgrounds/text follow relations.
        let instance_size = store.object(child).unwrap().size();
        if instance_size != built_size && !store.object(child).unwrap().children().is_empty() {
            if let Some(target) = store.object_mut(child) {
                target.width = built_size.width;
                target.height = built_size.height;
            }
            store.set_size(child, instance_size.width, instance_size.height);
        }

        store.set_parent(child, Some(object));

        // 再读依赖父节点的部分：齿轮要访问父容器的控制器，按钮要绑定上级控制器。
        {
            let table = package.string_table();
            let child_object = store
                .object_mut(child)
                .ok_or_else(|| Error::invalid_data(child_pos, "child object vanished"))?;

            child_object.setup_after_add(&mut buffer, table, child_pos)?;
        }
        // Native GearBase.init snapshots the pre-extension values before a
        // text/button/label instance applies its block-6 properties.
        super::gear::setup_gears(store, child, &mut buffer, package.string_table(), child_pos)?;
        {
            let table = package.string_table();
            let child_object = store.object_mut(child).unwrap();
            leaf::setup_after_add(child_object, &mut buffer, table, child_pos)?;

            // 子对象自身的关系定义（块 3）。此时全部子对象都已创建，
            // 描述里按编号引用的兄弟对象才能正确解析，所以放到这一遍来读。
            if buffer.seek(child_pos, 3)? {
                child_object.relations.setup(&mut buffer, table, false)?;
            }

            // 由容器派生的控件还要在实例数据里读扩展部分。
            let kind = child_object.kind();
            match &mut child_object.control {
                ControlData::Button(data) => {
                    super::extend::setup_button(data, &mut buffer, table, child_pos, kind)?;
                }
                ControlData::Label(data) => {
                    super::extend::setup_label(data, &mut buffer, table, child_pos, kind)?;
                }
                ControlData::ProgressBar(data) => {
                    super::extend::setup_progress(data, &mut buffer, table, child_pos, kind)?;
                }
                ControlData::Slider(data) => {
                    super::extend::setup_slider(data, &mut buffer, table, child_pos, kind)?;
                }
                ControlData::List(data) => {
                    super::list::read_list(
                        data,
                        &mut buffer,
                        table,
                        child_pos,
                        true,
                        ctx.scroll_defaults,
                    )?;
                }
                ControlData::Tree(data) => {
                    super::tree::read_tree(
                        data,
                        &mut buffer,
                        table,
                        child_pos,
                        ctx.scroll_defaults,
                    )?;
                }
                ControlData::ComboBox(data) => {
                    super::combo::read_combo(data, &mut buffer, table, child_pos)?;
                }
                _ => {}
            }
        }

        // GComponent.setup_afterAdd's first block-4 value binds a pane to a
        // parent controller. List setup must precede this: inline lists create
        // their pane while reading their own instance block 5.
        if store.object(child).unwrap().control().component().is_some()
            && buffer.seek(child_pos, 4)?
        {
            let page_controller = buffer.read_i16()?;
            let data = store
                .object_mut(child)
                .unwrap()
                .control
                .component_mut()
                .unwrap();
            if data.scroll_pane.is_some() {
                data.page_controller = usize::try_from(page_controller).ok();
            }
        }

        store.object_mut(child).unwrap().under_construct = false;
        store.initialize_button(child);
        store.initialize_progress(child);
        store.initialize_slider(child);
        if matches!(
            store.object(child).map(|object| object.control()),
            Some(ControlData::ComboBox(_))
        ) {
            store.apply_title_of(child);
        }
        store.mark_subtree(child, crate::object::dirty::ALL);
        children.push(child);

        buffer.set_position(child_pos + data_len);
    }

    // ---- 组编号绑定 ----
    //
    // 描述里的组编号是同一容器内的子对象序号，此时子对象都已建好，可以绑定了。
    // 少了这一步，分组布局就无从知道谁是它的成员，`layout_all_groups` 会空转。
    for &child in &children {
        let Some(index) = store.object(child).and_then(|object| object.group_index) else {
            continue;
        };
        let Some(group) = children.get(index.max(0) as usize).copied() else {
            continue;
        };

        // 编号指向的必须真的是个分组控件，否则宁可当作没有。
        let is_group = matches!(
            store.object(group).map(|object| object.control()),
            Some(ControlData::Group(_))
        );
        if is_group {
            store.set_group(child, Some(group));
        }
    }

    // ---- 块 3：容器自身的关系 ----
    if buffer.seek(0, 3)?
        && let Some(target) = store.object_mut(object)
    {
        target
            .relations
            .setup(&mut buffer, package.string_table(), true)?;
    }

    // ---- 块 5：过渡动画 ----
    if buffer.seek(0, 5)? {
        let count = buffer.read_i16()?.max(0) as usize;
        for _ in 0..count {
            let next_pos = i32::from(buffer.read_i16()?);
            let end = buffer.resolve_relative(next_pos)?;

            let mut transition = crate::transition::Transition::default();
            transition.setup(&mut buffer, package.string_table())?;
            transition.bind_targets(store, object);
            component.transitions.push(transition);

            buffer.set_position(end);
        }
    }

    // ---- 块 4：整体属性 ----
    if buffer.seek(0, 4)? {
        buffer.skip(2); // 自定义数据长度，暂未使用
        component.opaque = buffer.read_bool()?;

        let mask_child = buffer.read_i16()?;
        component.mask_child = if mask_child >= 0 {
            Some(mask_child as usize)
        } else {
            None
        };

        if component.mask_child.is_some() {
            component.mask_reversed = buffer.read_bool()?;
        }
        let hit_test_id = buffer.read_s(package.string_table())?;
        let hit_offset_x = buffer.read_i32()?;
        let hit_offset_y = buffer.read_i32()?;
        component.hit_test = match hit_test_id {
            Some(item_id) => Some(HitTestArea::Pixel {
                item_id,
                offset_x: hit_offset_x,
                offset_y: hit_offset_y,
            }),
            None if hit_offset_x != 0 && hit_offset_y != -1 => Some(HitTestArea::Child {
                child_index: hit_offset_y.max(0) as usize,
            }),
            None => None,
        };
    }

    debug!(
        component = item.name.as_str(),
        children = children.len(),
        "component constructed"
    );

    // 由容器派生的控件还要在自己的描述块 6 里读扩展数据。
    let mut control = match item.object_type {
        ObjectType::Button => ControlData::Button(Box::new(super::extend::ButtonData {
            component,
            ..Default::default()
        })),
        ObjectType::Label => ControlData::Label(Box::new(super::extend::LabelData {
            component,
            ..Default::default()
        })),
        ObjectType::ProgressBar => {
            ControlData::ProgressBar(Box::new(super::extend::ProgressData {
                component,
                ..Default::default()
            }))
        }
        ObjectType::Slider => ControlData::Slider(Box::new(super::extend::SliderData {
            component,
            ..Default::default()
        })),
        ObjectType::List => ControlData::List(Box::new(super::list::ListData {
            component,
            ..Default::default()
        })),
        ObjectType::Tree => ControlData::Tree(Box::new(super::tree::TreeData {
            list: super::list::ListData {
                component,
                ..Default::default()
            },
            ..Default::default()
        })),
        ObjectType::ComboBox => ControlData::ComboBox(Box::new(super::combo::ComboBoxData {
            component,
            ..Default::default()
        })),
        ObjectType::ScrollBar => ControlData::ScrollBar(Box::new(crate::scroll::ScrollBarData {
            component,
            ..Default::default()
        })),
        _ => ControlData::Component(Box::new(component)),
    };

    if let ControlData::Button(data) = &mut control {
        let (mode, sound, volume, down_effect, down_effect_value) =
            super::extend::read_button_extension(&mut buffer, package.string_table())?;
        data.mode = mode;
        data.sound = sound;
        data.sound_volume_scale = volume;
        data.down_effect = down_effect;
        data.down_effect_value = down_effect_value;
    }

    if let ControlData::ComboBox(data) = &mut control {
        super::combo::read_combo_extension(data, &mut buffer, package.string_table())?;
    }
    if let ControlData::ProgressBar(data) = &mut control
        && buffer.seek(0, 6)?
    {
        data.title_type = i32::from(buffer.read_u8()?);
        data.reverse = buffer.read_bool()?;
    }
    if let ControlData::Slider(data) = &mut control
        && buffer.seek(0, 6)?
    {
        data.title_type = i32::from(buffer.read_u8()?);
        data.reverse = buffer.read_bool()?;
        if buffer.version() >= 2 {
            data.whole_numbers = buffer.read_bool()?;
            data.change_on_click = buffer.read_bool()?;
        }
    }

    if let ControlData::ScrollBar(data) = &mut control {
        data.fixed_grip_size =
            crate::scroll::bar::read_scroll_bar_extension(&mut buffer, package.string_table())?;
    }

    if matches!(&control, ControlData::Button(data) if data.down_effect == 2) {
        let anchor = store
            .object(object)
            .is_some_and(|object| object.pivot_as_anchor());
        store.set_pivot(object, 0.5, 0.5, anchor);
    }
    if let Some(target) = store.object_mut(object) {
        target.control = control;
    }

    if let Some(data) = store.object(object).and_then(|o| o.control().component()) {
        let (index, reversed, hit_test) =
            (data.mask_child, data.mask_reversed, data.hit_test.clone());
        if let Some(index) = index {
            let mask = *children.get(index).ok_or_else(|| {
                Error::invalid_data(0, "component mask child index out of bounds")
            })?;
            store.set_component_mask(object, Some(mask), reversed);
            // Explicit exported hit areas are assigned after the mask setter.
            if hit_test.is_some() {
                store
                    .object_mut(object)
                    .unwrap()
                    .control
                    .component_mut()
                    .unwrap()
                    .hit_test = hit_test;
            }
        }
    }

    store.bind_exported_scroll_page_controllers(object);
    store.initialize_controllers(object);
    store.object_mut(object).unwrap().under_construct = false;
    store.initialize_resource_title(object);
    // Resource-root buttons also run GButton's construction-time UP effects;
    // they do not pass through the parent component's child initialization loop.
    store.initialize_button(object);
    store.initialize_progress(object);
    store.initialize_slider(object);

    Ok(())
}
