# fairygui-bevy

[FairyGUI](https://www.fairygui.com/) 运行时的 Bevy 实现，移植蓝本为官方
[FairyGUI-layabox](https://github.com/fairygui/FairyGUI-layabox)（MIT License）。

可以在 Bevy 项目中直接加载 FairyGUI 编辑器导出的 `.fui` 包，复现由编辑器设计的
界面、动效与交互逻辑。

## 工程结构

Cargo workspace，按「平台无关核心」与「Bevy 适配层」分层：

| crate | 说明 |
| --- | --- |
| `crates/fairygui-core` | 平台无关运行时：字节流读取、`.fui` 包解析、对象模型、全部控件与行为。**不依赖 Bevy**。 |
| `crates/fairygui-bevy` | Bevy 集成层：资产加载、对象树到实体树的生成与增量同步、输入桥接。所有易变的 `bevy_ui` API 收敛在 `backend` 模块。 |
| `crates/fairygui-bevy-macros` | `GuiExtension` 派生宏：按资源地址自动绑定应用组件与子控件引用。 |
| `crates/fairygui-demo` | 可运行示例。 |

依赖方向固定为 `fairygui-bevy` → `fairygui-core`，核心层永不反向依赖 Bevy。

输入与帧调度的设计见 [架构说明](docs/architecture.md)。应用可通过
`fairygui_bevy::prelude::*` 导入常用类型，将事件处理系统放入
`GuiSystems::Respond`，保证本帧点击在本帧同步到视图。`GuiEvent` 以 Bevy
entity observer 事件形式发布；`GuiInputSettings` 配置拖动阈值与输入窗口，
`GuiInputState` 提供悬停、指针捕获与游戏输入仲裁状态。

弹出菜单可用 `show_popup_menu_items` 配置稳定名称、动作编号、图标、勾选状态和
分隔项模板；业务系统读取 `GuiMenuAction`，消息在点击 100 ms 后、菜单关闭后发布。
支持按名称动态编辑与增删、保留对象复用，详见 [菜单生命周期](docs/popup-lifecycle.md)。

业务系统也可用 `GuiPopupCommand` 消息管理弹出层，无需借用运行时：

```rust,ignore
use fairygui_bevy::core::popup::MenuItem;

commands.write_message(GuiPopupCommand::ShowMenu {
    source: page_entity,
    menu: GuiPopupMenu::new("PopupMenu", "PopupMenuItem", vec![
        MenuItem::action("save", "保存", 1),
    ]),
    position: Vec2::new(100., 200.),
});
```

`source` 接受根拥有者或控件视图实体，模板名在其来源包内解析；模板也可以使用
已登记包的 `ui://` 地址。`Show` 显示已有的根/控件；`Hide` 收起该弹出层及其上层，
`HideAll` 收起全部弹出层，隐藏后保留对象和视图。运行时保留一个菜单配置；
`ReopenMenu` 保留勾选与动态编辑，`DisposeMenu` 回收菜单及其视图。
`GuiPopupState` 资源提供从底到顶的控件实体 `stack()` 和当前菜单 `menu()`，
在输入事件发布前与 Sync 结束时更新，业务可据此查询控件或挂载 observer。

位置采用舞台逻辑像素，视口使用 `GuiInputSettings` 选择的窗口；无窗口应用默认
1280×720，已有舞台则沿用其尺寸。等待 `.fui` 的根拥有者可提前接收显示请求，
资产就绪后执行；根被替换、移除或销毁会取消等待请求。`HideAll` 同时取消全部
等待显示，`Hide` 取消对应目标的等待显示，`DisposeMenu` 取消等待中的菜单显示。
对象属性在 Sync 中同步，随后处理 `GuiPopupCommand`；
同一类消息保持顺序。

`ShowAnchored { popup, anchor, direction }` 根据锚点控件当帧的逻辑坐标放置已有
弹出层。`GuiPopupDirection::Auto` 在下方空间不足时翻到上方，`Up` / `Down`
指定方向；保留原版的横向溢出调整和向上放置时的 1 像素间隔。
锚点在显示请求执行时必须已创建；弹出根本身等待资产的规则与 `Show` 一致。

下拉框及菜单的组件、列表项和分隔项模板可使用其他已登记包的 `ui://` 地址，
嵌套子组件也按声明的源包构造并使用各自图集；依赖通过 `register_package` 登记。

核心层的内部分工：

```
binary/        大端读取器、索引表定位、两段式字符串解码
package/       六块索引表解析、条目、位图字体、序列帧、运行时注册表
object/        GObject、竞技场与对象树、脏标记、事件队列
widget/        各控件的数据与描述解析（叶子控件、容器、扩展控件、列表/树/下拉框）
scroll/        滚动容器与滚动条
transition/    过渡动画、通用补间、三十一种缓动、运动路径
text/          原版 UBB 转换、HTML/XML 样式与实体解析
geom  error                            几何与错误类型
color/  color_matrix/                  颜色类型与颜色矩阵
action  pool  root  window  popup      控制器动作、对象池、根节点、窗口、弹出菜单
drag_drop  translation  async_build    拖拽、多语言、分帧构建
```

## 环境要求

- Rust 1.98（见 `rust-toolchain.toml`）
- Bevy 0.20.1（2026-11-14 再次核实为最新稳定版，MSRV 为 Rust 1.97.1）

## 快速开始

1. 用 FairyGUI 编辑器打开 UI 工程，发布时选择 **`.fui` 二进制格式**。
2. 把导出的 `.fui` 与图集放进 `crates/fairygui-demo/assets/ui/`（详见该目录 `README.md`）。
3. 运行：

```bash
cargo run -p fairygui-demo
```

Windows 下 demo 默认使用 DX12，避免部分 Vulkan 驱动的交换链异常；仍可通过
`WGPU_BACKEND` 环境变量显式选择后端。交互回归可运行
`cargo run -p fairygui-demo -- --smoke-test`：打开真实窗口，自动点击全部 15 个
演示入口与 Basics 的 20 个子页面并返回，失败时以非零状态退出。
截图保存在工作目录的 `target/fairygui-demo-*.png`。

常规界面用 ECS 实体表达所有权。`FairyGuiPlugin` 会等资产加载完成后创建根界面，
并在组件移除或实体销毁时回收对象树和 Bevy 视图：

```rust,ignore
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component)]
struct Menu;

fn spawn_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((Menu, FairyGuiRoot::new(assets.load("ui/MainMenu.fui"), "Main")));
}
```

创建时可直接设置尺寸和位置，资产就绪后会在首次布局与视图生成前应用：

```rust,ignore
commands.spawn(
    FairyGuiRoot::new(assets.load("ui/Chat.fui"), "Main")
        .with_size(Vec2::new(800., 600.))
        .with_position(Vec2::new(100., 50.)),
);
```

尺寸和位置使用逻辑像素。之后也可修改根组件上的 `size` / `position`
（类型均为 `Option<Vec2>`），保留当前实体并更新关联布局；未改变的配置不会覆盖
命令或动画的结果。默认 `None` 保留导出值，已创建后改回 `None` 保留当前值。
拥有者上的属性组件在创建配置之后应用。当前实际几何请读取 `GuiTransform`。

需要根界面跟随窗口尺寸时，直接使用：

```rust,ignore
commands.spawn(
    FairyGuiRoot::new(assets.load("ui/Chat.fui"), "Main")
        .with_window_size()
        .with_position(Vec2::ZERO),
);
```

首次布局即使用窗口的逻辑尺寸，之后窗口缩放自动更新，保留根及子控件实体。
默认跟随主窗口；设置 `GuiInputSettings.window` 后跟随指定窗口。位置独立设置，
子控件通过编辑器配置的 relations 随根尺寸调整。启用时窗口尺寸优先于 `size`；
没有可用窗口时保留当前尺寸（创建时使用 `size` 或导出值）。修改
`follow_window_size = false` 可停止跟随，并恢复配置的 `size`（为 `None` 则保留当前尺寸）。

动态创建的组件可通过父实体直接挂进现有 FairyGUI 容器：

```rust,ignore
let child = commands.spawn(
    FairyGuiRoot::new(assets.load("ui/Basics.fui"), "Slider_HZ")
        .with_parent(container_entity)
        .with_position(Vec2::new(20., 60.))
        .with_size(Vec2::new(240., 30.)),
).id();
```

`parent` 接受 `FairyGuiRoot` / `FairyGuiWindow` 拥有者或带 `GObjectRef` 的容器视图。
位置在父节点局部坐标中；父变换、透明度、裁剪、舞台状态与输入规则自然作用于子树，
事件沿同一逻辑层级冒泡。父资产或父视图尚未就绪时等待，期间不会生成独立可见界面。
`GuiRootMountState`（或 `gui.root_mount(owner)`）提供等待资产、等待父目标、已挂载、
主动撤下、无效父节点、循环挂载与创建失败状态；叶对象不能作父节点。

修改 `FairyGuiRoot.parent` 可保留现有模型、视图及业务组件换父，设为 `None` 可提升为
独立界面；`active=false` 撤下子树，恢复后挂回配置的父节点。删除子拥有者或移除根组件
只回收该子树；删除父界面会一并回收仍挂载的子树，留下的子拥有者等待父目标。
父拥有者替换模板后，子实例会重建并重新挂载；直接引用旧父视图时需改为新的视图实体。
若要保留子实例，应在删除旧父的同帧先修改子根的 `parent` 或将其设为 inactive。
这不会自动给根拥有者添加 Bevy `ChildOf`；拥有者的业务生命周期由应用管理。
`--smoke-root-mount` 验证实际窗口中的局部坐标、换父、裁剪、隐藏恢复、冒泡与销毁。

运行时会在所有者实体上添加 `GuiRootView`，指向根视图实体。
`GuiQuery` 只读取 ECS 组件，可按名称或路径找到控件实体并直接挂载业务组件和 observer：

```rust,ignore
#[derive(Component)]
struct Bound;

fn bind_menu(
    mut commands: Commands,
    gui: GuiQuery,
    menus: Query<Entity, With<Menu>>,
    bound: Query<(), With<Bound>>,
) {
    for root in &menus {
        if let Some(button) = gui.child_by_path(root, "n1")
            && !bound.contains(button)
        {
            commands.entity(button).insert(Bound).observe(on_menu_button);
        }
    }
}

fn on_menu_button(event: On<GuiEvent>, mut texts: Query<&mut GuiText>) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click {
        if let Ok(mut text) = texts.get_mut(event.entity) { text.set("Clicked"); }
    }
}

// 新根就绪后立即绑定，并在同帧布局前应用业务配置。
app.add_systems(Update, bind_menu.in_set(GuiSystems::Initialize));
```

`child_by_name` 查找直属子节点，`find_by_name` 按 FairyGUI 逻辑顺序递归查找，
`child_by_path` 按 `/` 分隔的子节点名查找；都接受根拥有者或控件视图实体。

`gui.is_child_in_view(parent, child)` 查询直属控件是否与原生视口相交，也可通过
`GuiObjectInfo::children_in_view()` 获取有序结果。它使用逻辑几何、未缩放尺寸和
滚动面板当前显示偏移，包含边缘接触；不受控件显隐、舞台连接或祖先裁剪影响。
没有溢出的滚动轴不参与判断；普通非裁剪组件的子控件都在视口内。无效或非直属
目标返回 `None`。快照在 PostLayout 前与 Sync 末尾刷新，适合在本帧布局后选择动画行。
控件上的只读 `GuiObjectInfo` 提供名称、类型、父节点及有序子节点，排除渲染辅助实体，
隐藏或暂离舞台的控件仍可查询。数据在 `GuiSystems::Sync` 更新；Respond 阶段读取
上一帧的快照，Initialize 可查询本帧新建的根及其逻辑控件。高级模型操作可通过
`GuiRootInstance` 获取底层对象 ID 和 root handle。

`GuiTransform` 提供控件的逻辑位置、未缩放尺寸、缩放、轴心、旋转与斜切；
可用 `Query<&GuiTransform>` 查询视图实体，或用 `gui.transform(root_or_view)` 读取。
坐标相对 FairyGUI 逻辑父节点，旋转和斜切以度为单位，不包含滚动偏移、轴心补偿
或祖先变换，也不依赖 Bevy 布局是否已完成。例如根界面居中只需查询 ECS：

```rust,ignore
fn center_root(transforms: &mut Query<&mut GuiTransform>, view: Entity, viewport: Vec2) {
    if let Ok(mut transform) = transforms.get_mut(view) {
        transform.position = (viewport - transform.size) * 0.5;
    }
}
```

几何状态在 Sync 末尾更新，包含本帧修改、控制器、分组和动画引起的变化，
未变化时不会触发 `Changed<GuiTransform>`。通过 `set_pivot { pivot, as_anchor }`
可同时配置比例轴心与锚点定位。`GuiFont` 资源也在根构建前同步，首帧和后续动态
界面会直接使用配置的字体，无需业务系统手工调用 `GuiRuntime::set_font`。

控件层级通过可变组件 `GuiSortingOrder(i32)` 设置。`0` 表示普通子节点，
非零优先级按升序排在普通节点之后；负值归零，相同优先级的新增或变更节点
排在已有同优先级节点之后。它只影响同一逻辑父节点下的顺序。

```rust,ignore
fn prioritize(mut priorities: Query<&mut GuiSortingOrder>, button: Entity) {
    if let Ok(mut priority) = priorities.get_mut(button) {
        priority.0 = 10;
    }
}

fn reorder(mut gui: GuiMut, button: Entity, sibling: Entity) {
    if let Some(mut object) = gui.get_mut(button) {
        object.set_child_index(0);
        object.swap_with(sibling);
    }
}
```

`set_child_index` 将普通控件移动到指定索引，超出普通节点区域会收拢到该区域末尾；
有非零优先级的控件忽略此操作。`set_child_index_before` 的索引以移动前的子节点序列为准，
向后移动时先减一。`swap_with` 按原版的两次索引设置执行，并遵守双方的排序限制。
`gui.child_index(entity)`、`gui.sorting_order(entity)` 和 `GuiObjectInfo::children()`
提供同步后的逻辑层级。绘制顺序和鼠标命中同时更新，保留已有实体及 observer；
列表选中项随对象保留，并更新其索引。`GuiSortingOrder` 可随等待资源的根拥有者一起挂载。
`--smoke-child-order` 使用重叠按钮验证优先级、重排、交换和真实点击目标。

容器的绘制顺序通过 `GuiRenderOrder` 修改，与逻辑层级分开：

```rust,ignore
fn render_order(mut containers: Query<&mut GuiRenderOrder>, view: Entity) {
    if let Ok(mut order) = containers.get_mut(view) {
        order.order = ChildrenRenderOrder::Arch;
        order.apex_index = 2;
    }
}
```

`Ascent` 按逻辑顺序绘制，`Descent` 倒序绘制；`Arch` 先正序绘制 apex 之前的节点，
再倒序绘制剩余节点，使 apex 最后绘制。原始 apex 保留在组件中，绘制时限制在
`0..=子节点数`；负值等同倒序，超出末尾等同正序。隐藏或无显示对象的子节点仍占
逻辑索引，apex 以该索引为准。绘制设置不会修改 `GuiObjectInfo::children()`、
关联、控制器目标或列表选中索引；Bevy 的局部层级及实际点击目标同步变化。

普通容器、按钮、列表、树及其他组件派生控件共用这套设置。`GuiRenderOrder` 可随
根拥有者一起声明初始值；即时操作也可用 `GuiObjectMut::set_children_render_order`
和 `set_apex_index`。虚拟列表在拱形模式下继续按视口中心自动选择 apex，并同步回
组件；未修改的属性不会覆盖该结果。滚动条始终位于内容上方，刷新头尾保留在内容
下方。`--smoke-render-order` 验证三种模式、隐藏 apex、边界 apex 和真实点击目标。

新建视图的绑定和初始配置放入 `GuiSystems::Initialize`：此时根视图、逻辑子控件和
几何/文本快照已就绪，列表与树的数据尚未准备。此阶段挂载的 `GuiListContent`、
`GuiVirtualList`、`GuiTreeContent` 会在同帧生成行，随后 `GuiSystems::PostLayout`
可读取完成布局的行快照并修改组件。demo 的菜单、关闭按钮、演示页和动态子页通过
Initialize 完成初始定位与绑定，首次绘制就使用业务配置。普通文本在 Bevy 完成本帧测量后
同步逻辑与物理内边距，首批字形就按正确位置绘制；`--smoke-first-frame` 在每个
视图首次完成布局的帧检查位置和文本对齐，并实际打开 Basics 的 Button 页面、
ListEffect、LoopList 和 TreeView，检查首帧内容、图标、缩放及动画状态。

`GuiSpace`（`gui.space(entity)`）提供原生逻辑坐标转换，也可直接调用
`gui.local_to_global`、`global_to_local` 及对应的 `_rect` 方法。转换包含祖先变换、
锚点轴心、边距、当前滚动显示偏移和虚拟列表对齐，不依赖 Bevy 布局或窗口 DPI。
矩形遵循原版两角转换，镜像/旋转可能保留负宽高；不可逆变换返回 `None`。
快照随逻辑几何在 PostLayout 前和 Sync 末尾刷新，未变化时保留变更标记。

`GuiText::value()` 或 `gui.text(entity)` 读取 FairyGUI 原始文本：按钮返回基础标题，
标签和下拉框转发到标题子控件；输入框返回已提交内容，不含提示文字、密码掩码、
IME 预编辑或翻译/UBB 渲染结果。无文本属性的控件没有 `GuiText`。
`GuiDisplay`（也可用 `gui.display(entity)`）分别提供显式显隐、控制器/分组显示状态、
舞台连接状态、本地/祖先累积透明度、置灰和输入开关。`display_visible()` 是本地
显示状态，不包含祖先显隐、裁剪或遮挡；隐藏和离开舞台也是独立状态。

文本和显示状态会在发布输入事件前、Sync 结束时各刷新一次，因此 observer 能读取
本帧输入刚提交的值，再修改组件在同帧更新其他控件。没有变化时保留 ECS 变更标记。
`GuiDisplay::touchable` 修改输入开关。富文本链接消息 `GuiLink.entity`
直接提供原始目标的控件实体，可据此查询、修改或匹配业务组件。

列表和树可在控件实体上直接监听 `GuiListItemClick`，无需扫描全局消息或读取选择：

```rust,ignore
commands.entity(list).observe(on_item);

fn on_item(event: On<GuiListItemClick>, gui: GuiQuery) {
    info!("item {}: {:?}", event.index, gui.text(event.item));
}
```

`entity` 是列表/树视图，`item` 是实际点击的行实体，`index` 是点击时的逻辑数据索引，
循环列表也不暴露内部记录索引。事件在选择处理后发布一次，不冒泡；禁用选择或
点击取消选中也会发布，保留 Shift/Ctrl、指针位置和触摸 ID。也可通过
`MessageReader<GuiListItemClick>` 消费同一通知。虚拟行可能在后续帧复用，保存数据
身份时使用本次 `index`，不要从稍后的行状态反推。

`GuiLoaderState` 是 Loader 视图上的只读组件，也可通过 `gui.loader(entity)` 查询
`Empty`、`Loading`、`Ready` 或具体失败原因。URL 改变时不会继续暴露旧 URL 的失败
结果；状态在 Publish 前、PostLayout 前和 Sync 末尾投影，未变化时保留变更标记。

冒泡的 `GuiEvent.entity` 表示当前接收实体，`target` 是原始目标的视图实体（没有
对应视图时为 `None`）。可沿 `GuiObjectInfo::parent()` 判断点击是否来自某个子控件，
无需借用运行时解析原生对象 ID。

冒泡使用 Bevy 的同一次事件传播，业务 observer 可停止后续祖先投递：

```rust,ignore
fn on_child(mut event: On<GuiEvent>) {
    if event.event.kind == fairygui_bevy::core::object::EventKind::Click {
        event.propagate(false);
    }
}
```

当前实体上的其他 observer 仍会执行；载荷修改会被后续祖先看到。每次派发独立，
相同事件重复触发也互不影响。路径在输入时按 FairyGUI 逻辑树保存，保留 grip/箭头的
原生截断位置，之后换父节点或渲染辅助节点不会改变这条路径；没有视图的接收者跳过。
这是对通知传播的控制，不会撤销输入阶段已完成的按钮/列表选择，也不会取消另行发布的
`GuiListItemClick`、`GuiTextInput` 或 `GuiLink`。按钮选择、列表选择和控件值在发布通知前
刷新，observer 对组件的修改仍在同帧 Sync 生效。
`GuiEvent::new(entity, event)` 可构造只投递给一个实体的直接通知。
`--smoke-observer-events` 验证实际复选框点击、停止/继续冒泡、共享载荷和同帧文字更新。

`GuiInputState::hovered()`、`captured()`、`captured_touch(id)` 和
`hovered_touch(id)` 都返回控件视图 `Entity`，可直接用于 ECS 查询。`captures()`
提供左右鼠标键及各触点的独立捕获快照，顺序为左键、右键、触摸 ID 升序；
`capture(GuiPointerId::Mouse(MouseButton::Left))` 或 `GuiPointerId::Touch(id)`
可查询指定指针的来源窗口、目标、当前命中实体、起始/当前位置、拖动及点击抑制状态。
坐标单位为窗口逻辑像素；`suppresses_click()` 表示拖动阈值或业务拖拽取消了点击，
抬起时的命中条件仍独立检查。状态在 Input 后、PostLayout 前及 Sync 末尾刷新；
隐藏、模态阻挡或销毁的目标会从公开快照中排除，未变化时保留资源变更标记。
`wants_pointer_input()` / `wants_keyboard_input()` 可用于游戏输入仲裁，即使逻辑
捕获目标暂时还没有对应视图，GUI 仍会保留对该指针的占用。
切换输入窗口会结束旧窗口的捕获，抬起通知保留旧窗口的最后位置，不会把新窗口的
坐标作为旧控件的拖动输入。

嵌套滚动在指针超过本地轴的拖动阈值后选择面板。横纵方向竞争时按主要位移方向
处理，同方向优先最近面板；取得拖动权后保持到松手。鼠标和触摸使用相同规则。
`GuiScrollState::is_dragging()` 提供当前状态；
`GuiInputCommand::CancelScrollDrag(entity)` 取消该面板当前和等待中的指针拖动，
不产生惯性、吸附或下拉释放，并保留已发起的程序滚动动画。

ScrollPane 示例通过 `GuiVirtualList` 配置 1000 行，竖向拖动列表、横向滑出每行的
Stick/Delete 按钮。虚拟行重新绑定数据时通过 `GuiListItem::set_scroll_position`
重置横向位置；点按钮会更新右侧文字，在其他位置按下会收回已展开行，并取消
展开行与外层列表的拖动。`--smoke-nested-scroll` 验证实际拖动、按钮点击、回收重绑与销毁。

输入框通过 `GuiInputCommand::Focus(entity)` 获取焦点，`Blur` 释放 GUI 输入焦点；
`GuiInputState::focused_entity()` 返回实际输入框视图实体。`GuiTextInput.entity`
也指向该输入框。可直接在输入实体上 `.observe(keep_editing)`，响应 Enter 并保留编辑会话：

```rust,ignore
fn keep_editing(
    input: On<GuiTextInput>,
    mut commands: Commands,
) {
    if input.event.kind == fairygui_bevy::core::object::EventKind::Enter {
        commands.write_message(GuiInputCommand::PreventSubmit(input.entity));
    }
}
```

输入命令在 Sync 完成视图、编辑策略和舞台同步后、焦点协调前执行；Respond 和
PostLayout 均可发送。Focus/PreventSubmit 接受已就绪的根/窗口拥有者和控件视图，
复合控件转发到输入标题；资产未就绪或目标不存在时丢弃请求。隐藏、离台、模态
遮挡等仍遵循统一输入策略。Blur 保留其他 Bevy 控件的焦点，同帧焦点请求以最后一个为准。

`GuiTextInput` 的目标 observer 在 Publish 阶段触发，不冒泡；`value` 为已提交的
原始文本，密码框也不会用遮罩字符替代。需要集中记录时仍可通过消息读取。

Chat 示例通过页面组件拥有消息和表情面板，Send 按钮、输入框与表情列表各自
绑定 observer。支持按钮/回车发送、首条回复与后续随机回复、100 条历史上限、
左右行模板和富文本表情；发送前处于底部才继续跟随滚动。表情面板在按钮上方
显示，选中表情追加标签并保留面板，点击外部关闭；页面销毁时一并回收。
`--smoke-chat` 验证实际菜单、键盘、按钮、表情选择、历史滚动及关闭重开。

拖放也由实体驱动。在源控件上添加 `GuiDragSource`，目标上添加 `GuiDropTarget`
并绑定 observer，即可在鼠标或触摸超过拖动阈值后搬运数据：

```rust,ignore
commands.entity(source).insert(GuiDragSource {
    icon: Some("ui://package/item".into()),
    data: Some("inventory:42".into()),
});
commands.entity(receiver).insert(GuiDropTarget).observe(on_drop);

fn on_drop(event: On<GuiDrop>) {
    // event.entity 是接收者，event.source 是源控件实体。
    info!("drop {:?} from {:?}", event.data, event.source);
}
```

源和目标组件添加在控件视图上；释放时最近的目标祖先接收一次 `GuiDrop`。
可选图标使用原生 100×100、中心轴心、不接收输入的 Loader，结束或取消后回收。
`GuiDragState` 资源提供源、临时图标、当前目标、数据和所属触摸 ID；
`GuiDragCommand::Start` 支持业务主动启动已捕获指针的拖放，`Cancel` 取消当前拖放。
源销毁、隐藏、离台或被模态界面阻挡时自动取消；取消后需重新按下才会自动开始。
`GuiIcon` / `gui.icon(entity)` 读取原生图标地址，`set_icon` 修改图标，空字符串清除。
Basics 的 b→c 拖放使用这套接口，`--smoke-drag-drop` 可运行真实窗口专项回归。

移动控件本身使用 `GuiDraggable`；自由拖动不创建代理，边界可指定舞台矩形或另一个
控件实体的区域：

```rust,ignore
commands.entity(free).insert(GuiDraggable::default());
commands.entity(bounded).insert(GuiDraggable {
    bounds: Some(GuiDragBounds::Control(bounds_entity)),
});
```

`GuiDragBounds::Stage(rect)` 使用固定舞台逻辑坐标；`Control(entity)` 随边界控件的
位置和尺寸更新。`GuiObjectDragState` 提供拖动中和所属触摸 ID；在控件上观察
`GuiEvent` 的 `DragStart`、`DragMove`、`DragEnd` 响应生命周期。超过阈值时记录
起点，从后续位移开始移动，位置遵循原版整数取整，超大控件固定到边界起点。
移除组件或失去输入资格会结束拖动；同一控件只允许一个指针操作。Basics 的 A
自由拖动、D 边界拖动也通过这些组件绑定。

滚动容器上的 `GuiScrollSettings`（或 `gui.scroll_settings(entity)`）提供可写的滚轮开关、
步长、条目吸附、触摸拖动、回弹与减速率。直接修改组件字段，插件会在处理输入前同步。
需要影响当帧输入的业务系统应放在 `GuiSystems::Input` 之前：

```rust,ignore
fn configure_scroll(mut panes: Query<&mut GuiScrollSettings>) {
    for mut pane in &mut panes {
        pane.mouse_wheel_enabled = true;
        pane.scroll_step = 30.;
        pane.touch_effect = true;
    }
}
```

只修改需要调整的字段可保留导出配置及原生运行时的其他修改。`scroll_step = 0` 恢复配置默认值，
负数反转滚轮方向，行滚轮移动两倍步长；减速率必须是 0～1 之间的有限值（不含端点）。
配置修改不主动移动视口或重启补间；位置与播放状态仍从只读 `GuiScrollState` 查询。
也可在 `FairyGuiRoot` 加载前附加设置组件，或使用直接方法 `set_scroll_settings`。

滚动容器上的只读 `GuiScrollState`（或 `gui.scroll(entity)`）提供逻辑目标位置、
动画当前显示位置、视口/内容尺寸、最大位置、两种位置对应的进度比例及当前页。
它在输入事件发布前和 Sync 结束时更新，没有滚动容器的对象不带该组件。
`scroll_to { position, animated }` 按原生分页/条目对齐规则滚动；
`set_scroll_percentage { percentage, animated }` 将比例限制在 0～1，并在执行时读取
实际范围，因此到顶部/底部无需业务代码提前计算尺寸：

```rust,ignore
gui_edit.edit(list_entity, |object: &mut GuiObjectMut| object.set_scroll_percentage(Vec2::Y, false));
```

`scroll_to_view { rect, animated, set_first }` 把内容坐标中的矩形滚入视口；
`set_first` 为真时优先对齐矩形起点，最终位置仍受分页、条目吸附和边界约束。
原有 `set_scroll_position` 直接设置
位置，允许选择是否越界；动画是否进行中可通过 `is_animating()` 查询。

`GuiTransitions` 提供控件上全部过渡的 ECS 快照；`gui.transition(entity, "t0")`
可读取播放中、暂停、反向、时间、导出时长、速度及播放编号。状态在 Publish 前和
Sync 末尾更新。`play_transition_with_options { name, options }` 支持延迟、区间、
重复次数和反向播放，与核心 `TransitionPlayOptions` 使用同一规则。

自然播放结束通过 `GuiTransitionComplete { entity, transition, play_generation }`
通知，包含所有重复及等待中的嵌套任务；同帧开始并结束的短过渡也会通知。
停止、重播替换、离开舞台和销毁不会伪造自然完成。业务可比较当前状态的
`play_generation()`，避免把旧一次播放的完成消息用于新一次播放。
消息在 Sync 后发布，Respond 系统下一帧读取；放在 Sync 后的业务系统可同帧读取。
`GuiTransitionHook` 使用 `entity` 与 `hook` 字段，`GuiTransitionCallback` 使用
`entity` 与 `callback` 字段，也在 Sync 后发布。demo 通过完成消息回收临时界面。

普通属性直接修改 ECS 组件：`GuiText`、`GuiIcon`、`GuiTransform`、`GuiDisplay`、
`GuiTextFormat`、`GuiTooltips`、`GuiData`、`GuiButtonSelected`、`GuiSliderValue`、`GuiProgressValue` 以及滑动条、进度条、下拉框的完整状态。
将业务系统放在 `GuiSystems::Respond`，插件在推进动画前应用修改，并在本帧 Sync 更新 Bevy UI。

列表和树的 `GuiListLayout` 可直接修改布局类型、行列数、间距、对齐与自动拉伸。
修改会重新布局普通列表及虚拟列表，并同步滚动内容范围；不合法的计数或虚拟几何会保留
原布局。对齐量作用在列表内容容器上，行项目坐标仍保持原生局部坐标。
`GuiListSettings` 控制点击列表项时滚入视口，以及不可见行是否折叠布局；折叠只改变位置和内容范围，
不改变项目顺序、索引或选择状态。
直接修改行的 `GuiDisplay.visible`、`GuiTransform.size` 或列表自身尺寸，也会在当帧
刷新普通列表和树的布局、自动行宽与滚动内容范围，无需再调用布局方法。

```rust,ignore
fn update_label(mut labels: Query<&mut GuiText>, target: Entity) {
    if let Ok(mut text) = labels.get_mut(target) {
        text.set("Hello");
    }
}

fn move_button(mut transforms: Query<&mut GuiTransform>, target: Entity) {
    if let Ok(mut transform) = transforms.get_mut(target) {
        transform.position = Vec2::new(20., 30.);
    }
}
```

这些属性保留上次模型值，只写入业务实际修改的字段，输入、关联、控制器和动画的结果
会同步回组件。未变化时保留组件变更标记。修改控件的视图实体；根创建前的位置、尺寸
用 `FairyGuiRoot` 的构造方法设置，其余初始属性可随拥有者一起挂载，资产就绪后应用。
移除属性组件会恢复原生投影；移除拥有者会一起清理对应状态。

文字样式直接修改 `GuiTextFormat`，按钮、标签和下拉框会转发到其标题文字：

```rust,ignore
fn style_text(mut formats: Query<&mut GuiTextFormat>, target: Entity) {
    if let Ok(mut format) = formats.get_mut(target) {
        format.font_size = 32;
        format.color = Color::srgb(0.2, 0.6, 1.0);
        format.bold = true;
        format.stroke = 2.0;
        format.stroke_color = Color::BLACK;
    }
}
```

组件同时提供字体、水平/垂直对齐、行距、字距、粗体/斜体/下划线、单行、自动尺寸、
UBB 和 HTML 开关。`font = None` 使用默认字体，也支持系统字体族、外部字体资产和
`ui://` 位图字体；颜色接受 Bevy `Color`，转换到 sRGB 后写入模型。灰态等显示效果
不改变这里的原始颜色，文本内容仍通过 `GuiText` 修改。未编辑的样式字段跟随原生
控制器和过渡变化；颜色按原版回写当前页的颜色齿轮，字号修改不回写字号齿轮。
格式变化在同帧文本测量之前生效，自动尺寸继续参与关联与列表布局。

`GuiQuery::text_format` 可读取根拥有者或控件的格式。初始格式可随根拥有者挂载，
其所有字段都会作为初始配置；仅修改导出格式的少数字段时，在 Initialize 查询并编辑。
即时操作可用 `GuiObjectMut::text_format` / `set_text_format`。无文字标题的控件不带
该组件。`--smoke-text-format` 验证格式修改、普通/富文本/位图输入切换和真实按钮点击。

图形绘制可通过 `GuiGraph` 修改，位置与尺寸继续使用 `GuiTransform`：

```rust,ignore
fn configure_graph(mut graphs: Query<&mut GuiGraph>, target: Entity) {
    if let Ok(mut graph) = graphs.get_mut(target) {
        graph.kind = GraphType::RegularPolygon;
        graph.sides = 5;
        graph.start_angle = -90.;
        graph.distances = vec![1., 0.5, 1.2];
        graph.fill_color = Color::srgb(0.2, 0.7, 1.);
        graph.line_size = 3;
        graph.line_color = Color::BLACK;
    }
}
```

支持 Empty、Rect、Ellipse、Polygon 和 RegularPolygon；按原版，Ellipse 实际是以宽度
一半为半径、位于 `(width/2, height/2)` 的圆，绘制和命中可以超出声明矩形。任意多边形
使用 `polygon_points: Vec<Vec2>`，缩放尺寸不改写这些局部坐标；正多边形使用较小尺寸
生成顶点，缺少或为 NaN 的距离系数取 1。未修改字段跟随原生变化。

`corner_radius` 顺序为左上、右上、左下、右下；`None` 使用普通矩形命中，提供圆角数组
时沿用原版 DrawPath 不参与 HitArea 的行为。命中不取决于填充或描边透明度。
Empty 不绘制也不命中。非有限坐标、角度、圆角或无限距离会恢复上次有效字段，其他
修改仍可生效。有效 NaN 距离保留，不会造成逐帧组件改写或几何重建。

`GuiColor` 也可改填充色，并按原版回写颜色齿轮；`GuiGraph.fill_color` 使用绘制参数
路径，不回写图形自身的颜色齿轮。同帧同时修改两者时 `GuiGraph` 的显式填充色优先。
可用 `GuiQuery::graph` 查询配置，或用 `GuiObjectMut::set_graph` 即时修改。
`--smoke-graph-properties` 验证形状切换、原版圆的外扩绘制/命中、轮廓穿透和回收。

按钮配置通过 `GuiButtonSettings` 修改：

```rust,ignore
fn configure_button(mut buttons: Query<&mut GuiButtonSettings>, target: Entity) {
    if let Ok(mut button) = buttons.get_mut(target) {
        button.mode = ButtonMode::Check;
        button.change_state_on_click = true;
        button.selected_title = Some("已选中".into());
        button.selected_icon = Some("ui://MyPackage/selectedIcon".into());
        button.down_effect = 2;
        button.down_effect_value = 0.8;
    }
}
```

`ButtonMode` 提供 Common、Check、Radio。选中值使用 `GuiButtonSelected`，基础标题和
图标使用 `GuiText` / `GuiIcon`；选中标题/图标不覆盖基础值或按钮自身的文字/图标齿轮。
清空选中标题或图标会立即恢复基础值，基础图标也为空时清除已有图标。
切回 Common 会先按旧模式取消选择，因此 Check 按钮可能联动关联控制器的相反页；
程序配置和选中赋值不投递 `StateChanged`。

`sound` 为 `None` 或空字符串时关闭点击音效，`sound_volume_scale` 保留有限原始值，
播放时沿用音频适配层的 0..=1 裁剪。`change_state_on_click = false` 保留点击事件和音效。
`down_effect` 为 0 关闭、1 压暗非文本子控件、2 绕中心缩放；缩放强度必须为有限正数。
按住期间更改效果会先撤销旧效果，再应用新效果，松开后恢复基础尺度；无效效果配置
恢复上次有效值，其余字段仍可更新。配置在本帧输入之前按完整属性顺序同步。
`GuiQuery::button_settings` 可查询根拥有者或按钮视图，直接编辑可用 `set_button_settings`。
`--smoke-button-properties` 验证真实点击、选中显示、模式切换、按住时的动态缩放和回收。

输入框的编辑属性通过 `GuiTextInputSettings` 修改：

```rust,ignore
fn configure_input(mut inputs: Query<&mut GuiTextInputSettings>, target: Entity) {
    if let Ok(mut input) = inputs.get_mut(target) {
        input.editable = true;
        input.password = true;
        input.max_length = 6;
        input.prompt_text = Some("请输入密码".into());
        input.prompt_color = Color::srgb(0.4, 0.6, 0.8);
        input.try_set_restrict(Some("0-9")).expect("valid character class");
    }
}
```

控件标题为输入框时也会转发这些属性，可用 `GuiQuery::input_settings` 查询。
单行/多行通过 `GuiTextFormat.single_line` 设置；密码输入按原版使用单行编辑。
`editable = false` 保留焦点、选择和复制，并阻止排队中的用户修改；程序通过
`GuiText` 赋值继续生效。字符限制和 UTF-16 长度预算只约束用户输入、粘贴和 IME 提交，
改变限制不截断已有值。密码遮罩保留真实编辑值，`GuiText` 始终提供真实提交值。

提示文字按原版 UBB 移除模式处理，使用独立的 `prompt_color`，不进入模型文本或
编辑缓冲。`try_set_restrict` 在修改组件前验证表达式并返回错误；直接写入非法
`restrict` 时记录告警并恢复上次有效规则，其余字段仍会应用。未编辑的字段跟随
原生变化，切换密码和只读状态保留输入实体。即时操作可用 `set_input_settings`。
`--smoke-input-properties` 验证提示、真实键盘/IME、密码、只读和程序赋值。

图片和 Loader 的常用属性也可直接修改：

```rust,ignore
fn configure_loader(
    mut loaders: Query<&mut GuiLoaderSettings>,
    mut fills: Query<&mut GuiImageFill>,
    mut colors: Query<&mut GuiColor>,
    target: Entity,
) {
    if let Ok(mut loader) = loaders.get_mut(target) {
        loader.fill = 1; // fit
        loader.align = 1;
        loader.vertical_align = 1;
        loader.shrink_only = true;
    }
    if let Ok(mut fill) = fills.get_mut(target) {
        fill.method = FillMethod::Radial360;
        fill.amount = 0.75;
    }
    if let Ok(mut color) = colors.get_mut(target) {
        color.0 = Color::srgb(0.4, 0.8, 1.0);
    }
}
```

`GuiLoaderSettings` 提供六种内容缩放、水平/垂直对齐、只缩小、自动尺寸、组件
`use_resize` 和后续失败时的错误图标开关。资源地址通过 `GuiIcon` 修改，加载结果
通过 `GuiLoaderState` 读取。改设置会保留已装载组件及其实体，自动尺寸结果同步回
`GuiTransform`；`use_resize` 让组件内容使用其自身关联调整尺寸。

`GuiImageFill` 用于 Image 和 Loader 图片/影片的填充遮罩。`amount` 保留 `f64`
精度及原始值，绘制时限制在 0..=1；修改遮罩保留对象的布局和输入矩形。
未编辑的字段会跟随进度条等原生更新。Loader 装载的组件通过自身属性控制外观，
Loader 的颜色和图片遮罩作用于其图片/影片通道。

`GuiColor` 提供图片、影片、Loader 的着色、图形填充色以及文字/控件标题颜色，
使用 Bevy `Color` 并转换到 sRGB。它与 `GuiTextFormat.color` 同步；同帧显式修改
二者时以 `GuiTextFormat.color` 为准。灰态、滤镜与祖先透明度继续参与最终绘制，
组件中保留原始颜色。`GuiQuery::color`、`image_fill`、`loader_settings` 可读取这些
属性，`GuiObjectMut` 也提供对应即时 setter。无对应属性的对象不会挂载该组件。
`--smoke-visual-properties` 验证 Loader 布局、颜色、遮罩、资源切换及实体保留。

图片翻转通过 `GuiImageFlip` 设置，仅用于 GImage：

```rust,ignore
if let Ok(mut flip) = flips.get_mut(image) { // Query<&mut GuiImageFlip>
    flip.0 = FlipType::Horizontal;
}
```

支持 None、Horizontal、Vertical、Both。它按原版修改对象缩放，而非翻转纹理 UV；
填充遮罩及挂在图片上的子节点也参与反射。改变翻转值会把缩放重置为对应的 ±1，
并捕获 Size 齿轮；重复设置同一值保留自定义缩放。同帧显式修改 `GuiTransform.scale`
时以该缩放为准。`GuiTransform` 仍保存逻辑位置/尺寸，`GuiSpace` 使用实际显示变换。
按原版，无轴心图片改尺寸或缩放后可能保留旧显示补偿，下一次位置变化才刷新；
重新赋同一翻转值不会刷新补偿。设置为 None 会取消翻转并重置缩放。

导出的图片翻转同样遵循这条链路，非零翻转会覆盖之前读取的缩放；MovieClip 的
导出翻转字节仍按原版忽略，Loader 没有该属性。
可用 `GuiQuery::image_flip` 查询，或调用 `GuiObjectMut::set_image_flip` 即时设置。
核心回归数据执行未修改的 GImage/GObject 方法，覆盖 87 个状态及 261 个坐标点，
使用标量 Sprite 矩阵宿主，不代表原版 GPU 像素一致性。
`--smoke-image-flip` 检查四种翻转、重复赋值、尺寸/位置补偿、轴心/旋转、填充、
置灰与透明度，保留视图/子节点，并捕获 48 个 GPU 采样点。

混合模式和颜色滤镜也可直接修改组件：

```rust,ignore
fn configure_effects(
    mut blends: Query<&mut GuiBlendMode>,
    mut filters: Query<&mut GuiColorFilter>,
    target: Entity,
) {
    blends.get_mut(target).unwrap().0 = Some(BlendMode::Multiply);
    filters.get_mut(target).unwrap().0 = Some(ColorFilterParams {
        saturation: -0.5,
        hue: 0.25,
        ..default()
    });
}
```

`GuiBlendMode(None)` 恢复父级继承，`Some(BlendMode::Normal)` 显式使用普通混合；
这和枚举值 `BlendMode::None` 不同，后者仍是一个显式模式。
`GuiColorFilter(None)` 移除本地滤镜，`Some(default())` 保留单位滤镜及其离屏隔离。
修改已有滤镜的单个通道会保留原生动画对其他通道的修改。参数使用原版 -1..1 单位，
矩阵计算会夹紧超出范围的值，将 NaN 按 0 处理；组件保留原值且不会因 NaN 逐帧改写。
置灰期间保留本地滤镜，恢复后显示最新配置；组件容器沿用原生置灰控制器/子控件行为。

可用 `GuiQuery::blend_mode` / `color_filter` 查询，或用 `GuiObjectMut::set_blend_mode`
/ `set_color_filter` 即时修改。`--smoke-compositor` 使用这些组件验证实际混合、滤镜、
嵌套隔离与动态更新。具体模式和渲染限制见[离屏合成](docs/compositing.md)。

组件空白区域是否参与命中，通过 `GuiOpaque` 修改：

```rust,ignore
if let Ok(mut opaque) = components.get_mut(panel) { // Query<&mut GuiOpaque>
    opaque.0 = false;
}
```

`true` 让组件矩形参与命中，`false` 让没有子控件的空白位置穿透；子控件仍可接收事件
并向该组件冒泡。它独立于 `GuiDisplay.alpha` 和 `touchable`，不会改变绘制不透明度，
也不会禁用整棵子树。透明父容器仍可作为拖动/滚动的祖先。可在等待资源的根拥有者上
挂载初始值，使用 `GuiQuery::opaque` 查询，或调用 `GuiObjectMut::set_opaque` 即时修改。

像素、子控件和遮罩命中区域遵循原生优先级，修改 opaque 不会替换自定义区域。
遮罩设置会改变内部 mouseThrough/hitTestPrior；清除遮罩保留这些副作用，重复赋相同
opaque 不会重置它们。显式切换 opaque 可重新打开穿透，届时遮罩区域仍限制组件自身
的命中，但不再作为子控件的提前命中门。普通组件的鼠标/触摸均使用这条策略。
核心对照执行原版 GComponent 和 InputManager 方法，覆盖 24 个状态与 120 个命中点。
`--smoke-opaque` 验证真实窗口中的空白穿透、组件切换、子事件冒泡、alpha 为零时的
命中、touchable/visible、旋转/改尺寸、按下期间切换及实体回收。

播放、补间、滚动和树展开等操作通过 `GuiMut` 直接调用，不经过对象消息队列：

```rust,ignore
fn play(mut gui: GuiMut, target: Entity) {
    if let Some(mut object) = gui.get_mut(target) {
        object.play_transition("t0", 1);
        object.scroll_to(Vec2::new(0., 200.), true);
    }
}
```

`get_mut` 接受控件视图、根拥有者或窗口拥有者；对象尚未创建或已销毁时返回 `None`。
操作立即修改模型，渲染及只读查询在插件同步后刷新。初始播放放在 `GuiSystems::Initialize`；
该阶段已完成对象创建。独占系统和测试可用 `World::gui_mut`，使用方式相同。

`object.tween(target, duration, ease)` 从调用时的当前值启动通用
补间。`GuiTweenTarget` 支持位置、尺寸、缩放、透明度和旋转；`ease` 使用 FairyGUI
缓动编号（5 为默认 QuadOut）。相同对象与属性的新补间会替换旧补间，不同属性可
同时运行；`stop_tween(GuiTweenProperty)` 停在当前值。零时长立即落到终值，非法
尺寸、非有限值或负时长丢弃；调用前须等待目标对象就绪。
补间自然结束后在控件视图实体上触发 `GuiTweenComplete`，可通过
`commands.entity(view).observe(...)` 接收；通知发生在 Sync 的最终几何快照刷新后。
停止、替换与销毁会取消尚未发布的完成通知，零时长也只通知一次。

Joystick 示例把指针归属、摇杆位置和回位状态保存在页面组件中，仅在触摸区域
绑定指针 observer。支持鼠标、独立触点、150 像素半径限制、区域外拖动和松手回位；
回位中再次按下会取消旧补间，完成 observer 恢复按钮和中心标记。按钮通过
`set_button_change_state_on_click(false)` 交由业务逻辑控制选择状态，窗口缩放会结束
当前手势并恢复位置。`--smoke-joystick` 验证这些交互与页面销毁。

Guide 示例通过 ECS 拥有可复用的 `GuideLayer`，用坐标查询对齐背包按钮，以
0.5 秒补间移动反向遮罩窗口。点击高亮区域内的背包会撤下引导层；页面销毁会
回收引导层和动画；窗口缩放时会重新对齐。`--smoke-guide` 验证实际开关、动画位置、
遮罩穿透、复用、缩放和销毁。
过渡方法还支持暂停/恢复、速度、反向播放和停止。临时界面也用 `FairyGuiRoot`
实体管理；销毁所有者实体会自动回收对应 FairyGUI 对象树。
列表、进度条、滑动条和按钮实体分别带有 `GuiListSelection`、`GuiProgressValue`、
`GuiSliderValue`、`GuiButtonSelected` 查询组件。它们在事件发布前和同帧 Sync 后反映最新的控件状态；
状态可直接通过可变 ECS 组件写入，可以直接用 ECS 查询驱动其他业务系统。

`GuiListSelection.0` 接受逻辑行索引，写入会替换当前选择；空数组清空，重复和无效索引
按原生选择规则归一化，单选模式保留最后一个有效项。选择会联动行按钮和选择控制器，
不会自动滚动、展开树节点或生成点击事件。可与 `GuiListContent`、`GuiVirtualList` 或 `GuiTreeContent`
同批插入；选择会在内容准备后应用。虚拟列表支持尚未生成视图的离屏索引。

```rust,ignore
fn select_mail(mut lists: Query<&mut GuiListSelection>, target: Entity) {
    if let Ok(mut selection) = lists.get_mut(target) {
        selection.0 = vec![2];
    }
}
```

进度条的 `GuiProgressValue` 使用 `f64`，`GuiProgressState` 或 `gui.progress(entity)`
可读取范围、目标值、当前显示值、显示比例、标题类型、反向配置及补间是否运行。

```rust,ignore
gui_edit.edit(progress, |object: &mut GuiObjectMut| object.set_progress_range(0., 1.));
gui_edit.edit(progress, |object: &mut GuiObjectMut| object.set_progress(0.25));
gui_edit.edit(progress, |object: &mut GuiObjectMut| object.tween_progress(0.75, 1.));
```

`tween_progress` 对应原版 `tweenValue`：立即更新 `value()`，按线性补间推进
`displayed_value()`；替换时从旧补间当前采样值续接。`stop_progress_tween` 冻结显示，
保留目标值。直接 `set_progress` 设置不同值会取消补间，设置相同目标则保持运行。
原值允许小数和越界，条带比例夹在 0..1；数值标题按原版向下取整，图片填充模式
保持条带尺寸。范围与尺寸变化会立即重绘目标，但不停止补间，下一采样继续动画。
`set_progress_title_type` 使用 0..3，含义与滑动条相同；沿用参考库 setter 的即时重绘行为，
将类型编号作为显示值，下一次数值、范围、尺寸或补间更新会恢复对应显示。

自然结束会在控件视图实体上触发 `GuiProgressTweenComplete`，可用
`commands.entity(view).observe(...)` 接收；observer 执行前已发布本帧状态与几何。
停止、替换、不同值赋值和销毁会取消未发布的完成通知；程序赋值和补间均不触发
`StateChanged`。零时长调用立即到达目标并通知一次，非有限值和负时长丢弃。
资产等待期间可挂载初始范围和值，补间在对象就绪后调用；`--smoke-progress` 验证实际窗口中的条带
伸缩、径向填充、冻结显示、完成 observer 和销毁。

滑动条的 `GuiSliderValue` 使用 `f64`，完整状态可通过 `GuiSliderState` 或
`gui.slider(entity)` 查询。范围、小数值和输入策略通过 `Query<&mut GuiSliderState>` 设置：

```rust,ignore
if let Ok(mut state) = sliders.get_mut(slider) { // Query<&mut GuiSliderState>
    state.min = 0.;
    state.max = 1.;
    state.value = 0.25;
    state.whole_numbers = false;
}
```

程序赋值保留小数及范围外的原值，仅将显示比例限制在 0..1；整数模式只对指针输入
取整。横向和纵向条带按比例伸缩，grip 跟随导出的关联，缩放后拖动仍按控件局部位移
取值，按下不跳变。`set_slider_change_on_click(false)` 关闭轨道点击取值；
`set_slider_can_drag(false)` 暂停当前拖动，同原版，新一次 grip 按下会重新启用拖动。
`set_slider_title_type` 使用 0=百分比、1=值/最大值、2=值、3=最大值。
范围必须有限且 min <= max；初始属性组件可在资产就绪前挂载，首个视图即应用最终值和几何。
滑动条与进度条的 `min`、`max` 分别合并业务修改：只改其中一端时，另一端保留本帧
原生/直接 setter 的最新值。合并后的整对范围必须合法，否则保留当前原生范围；
同一组件中合法的数值、标题或输入策略修改仍会应用。两端保留 `f64` 精度，进度范围
更新不取消正在运行的补间，也不额外触发完成或 StateChanged。
输入导致值改变才触发 `StateChanged`，控件 observer 内可读取当前滑动条状态；
程序赋值不触发该事件。`--smoke-slider` 验证实际布局与 picking 下的缩放拖动、
小数、取整、输入开关和销毁。

grip 的按下事件在 grip 处停止冒泡，并独占该滑动条的拖动；其皮肤为按钮时仍以
grip 按钮作为稳定目标。轨道在按下时取值一次，之后拖动可交给父滚动容器，
`change_on_click=false` 也保留父滚动。鼠标和触摸共用这些规则，触摸状态变化保留
原触摸 ID，第二根手指不能接管已占用的 grip。`--smoke-slider-scroll` 验证虚拟列表
中实际 Slider 行的按下冒泡、轨道滚动和 grip 独占拖动。

下拉框实体带有 `GuiComboBoxState`，也可用 `gui.combo_box(entity)` 读取选项、value、
选中索引、可见项数、展开方向、绑定控制器以及当前 dropdown/list 视图。
通过 `Query<&mut GuiComboBoxState>` 修改数据和选择：

```rust,ignore
if let Ok(mut state) = combos.get_mut(combo) { // Query<&mut GuiComboBoxState>
    state.items = vec!["Apple".into(), "Banana".into()];
    state.values = vec!["apple".into(), "banana".into()];
    state.selected_index = 1;
}
```

`set_combo_selection` 按索引选择；不存在的 value 会取消选择，`None` 选择第一个空 value。
items、values 和 icons 独立，不要求等长。更新 items 会保留或收拢当前索引，空表取消
选择；`set_combo_icons(None)` 移除图标数组并保留当前显示图标，短数组不覆盖其范围外
的有效选项图标。`set_combo_visible_item_count`、`set_combo_popup_direction` 调整下次展开，
`set_combo_selection_controller` 按名称绑定逻辑父组件上的控制器，`None` 解除绑定。
选中索引与控制器双向联动，超出控制器页面数的索引不会改变控制器。

同原版，programmatic setters 不发出 `StateChanged`；真实选择或重选都会触发控件的
`GuiEvent` observer，observer 内可直接读取当前 `GuiComboBoxState`。选项修改在下次
展开时刷新弹层内容，隐藏保留弹层视图，销毁下拉框拥有者时一并回收。
`--smoke-combo` 验证实际菜单导航、选择/重选、value 设置、数据替换、弹层复用与回收。

虚拟列表可直接在列表视图实体上挂 `GuiVirtualList`，同步行测量由插件负责：

```rust,ignore
if let Some(list) = gui.child_by_name(root_entity, "mailList") {
    commands.entity(list).insert(GuiVirtualList::new(1000, |index, row| {
        row.set_text(&format!("Mail {index}"));
        row.set_controller_page("IsRead", i32::from(index % 2 == 0));
        if let Some(mut time) = row.child_by_path("timeText") {
            time.set_text("5 Nov 2015 16:24:33");
        }
    }));
}
```

渲染器只处理可见行，`GuiListItem` 可设置文本、图标、控制器、尺寸和轴心，按路径
访问子控件；尺寸在计算最终可见范围前生效。回调要求 `Send + Sync`，可捕获业务
数据快照。通过 ECS 修改 `item_count`、`set_renderer` 或替换组件，会刷新当前行；
数量和渲染器一起修改时，先更新数量，旧索引不会访问新的短数据集。
外部共享数据更新后也可用 `Mut<GuiVirtualList>::set_changed()` 请求重绘。

Both/Height 自动尺寸文本默认在原生布局前同步测量，文字修改会在本帧更新气泡等
关联对象，再由列表计算行高；无需再发送单独的测量命令。根创建时也会先测量再
生成视图，`Initialize` 能读到已经就绪的文字尺寸。普通列表和虚拟列表的 renderer
返回时会测量整行；回调内部需要立即读取尺寸时，仍可调用 `ensure_size_correct()`。
普通字体支持富文本样式、内联图片及导出的最大宽度换行，并按目标窗口与
`UiScale` 的实际倍率测量；位图字体复用原生测量器。
普通 ECS 系统也可调用 `GuiObjectMut::ensure_text_size()` 立即测量。自动尺寸字段会随文字、
字体或资源变化重新测量，字体未就绪时保留尺寸并重试；固定尺寸、缩小和省略模式
保持原有策略。`--smoke-item-provider` 覆盖混合模板、长文本、内联图片、右对齐
和行高关联，以及滚动复用、事件索引和回收。

不同数据行可以用不同模板，通过 `.with_item_provider(|index| Some(url))` 选择。
支持本包条目名和已注册包的 `ui://` 地址；`None` 或空字符串使用导出的默认模板。
例如左右聊天行可返回 `ui://Chat/chatLeft` / `ui://Chat/chatRight`，无须访问运行时。
`set_item_provider` 修改现有回调，`clear_item_provider` 恢复默认模板；模板别名先
归一化，再按模板复用对象与视图。换模板保留逻辑选择，循环列表的 provider 同样
只接收逻辑索引。跨包资源未就绪时报告 `MissingResource`，包注册后自动重试，
`GuiVirtualListState::error()` 会反映当前刷新结果，恢复成功后清除旧错误。

ListEffect 使用 `GuiListContent` 创建十封邮件，配置已读/领取页和时间文字；在
`GuiSystems::PostLayout` 查询初始可见行，通过 `play_transition_with_options` 每隔
0.2 秒播放导出的 `t0`，等待期间隐藏对应行。效果只在创建后启动一次，滚动进入的
其他行保留普通显示。`--smoke-list-effect` 验证错开播放、最终位置、点击选择和销毁。

包内组件可以注册为应用的 ECS 扩展，对应原版 `UIObjectFactory.setExtension`：

```rust,no_run
use bevy::prelude::*;
use fairygui_bevy::prelude::*;

#[derive(Component, GuiExtension)]
#[gui(url = "ui://VirtualList/mailItem")]
struct MailItem {
    #[gui(path = "timeText")]
    time: Entity,
    #[gui(path = "")]
    view: Entity,
    #[gui(path = "optionalBadge")]
    badge: Option<Entity>,
    // Unannotated application fields start with Default::default().
    opened: bool,
}

fn register(app: &mut App) {
    app.register_gui_extension::<MailItem>()
        .register_gui_extension_at::<MailItem>("ui://ListEffect/mailItem");
}
```

注册支持 `ui://包名/条目名` 与 `ui://包ID条目ID`，同一资源优先匹配 ID 地址。
注册可在插件和包加载前完成，替换注册只影响之后创建的视图。扩展挂在控件视图
实体上，根拥有者可通过 `GuiRootView` 找到；`path` 使用精确子控件路径，空路径
绑定当前实体，`Option<Entity>` 允许缺失。必需路径缺失时不会插入半成品组件，
`GuiExtensionState::error()` 提供原因，原生控件仍保留。

使用 `#[gui(url = "...", on_construct = bind_mail)]` 可指定接收
`&mut bevy::ecs::system::EntityCommands` 的构建钩子，用于附加 observer 或发送
属性组件。也可手动实现 `GuiExtension`，自定义 `construct` 和 `on_construct`。
构建按子组件到父组件的顺序执行一次：根扩展在 `GuiSystems::Initialize` 前就绪，
列表新增行在 `PostLayout` 前就绪；该阶段滚动产生的行也在同帧投影前构建。
普通根、窗口、嵌套组件、跨包 Loader 和列表模板共用注册，归池或离台不重新构建，
销毁视图会回收扩展组件。`ListEffect` 通过自动绑定的 `MailItem` 播放邮件动画，
首帧检查同时验证时间控件绑定。

`GuiVirtualListState` 提供 `info()` 布局统计和 `error()` 配置错误，Sync 后可读取。
`.looping()` 使用原生六份循环记录，回调仍接收逻辑索引；普通/循环模式在首次
初始化后固定，尝试切换会报告 `LoopModeMismatch` 并保留原来的渲染逻辑。
移除组件会清空逻辑行、释放回调，原生行池保留至列表拥有者销毁；重新挂组件可复用
池中的对象和视图。控件的创建、渲染器配置和行视图生成都由插件执行。

所有列表及树控件上的 `GuiListState` 提供逻辑条目数、当前实体行 `items()` 和
`first_item_in_view()`；行实体上的 `GuiListRow` 提供所属列表、当前逻辑索引和虚拟
记录索引。滚动条、刷新头尾和池中行不属于当前实体行；同一行实体复用时索引自动
更新，归池时移除 `GuiListRow`。也可通过 `gui.list(root_or_view)` 查询列表状态。

导出的树节点会自动创建行视图，并应用标题、图标、名称、选中标题/图标和控制器
页面。`GuiTreeState`（或 `gui.tree(entity)`）提供逻辑节点、父索引、层级、文件夹
状态、展开状态、可见节点和选中节点；行上的 `GuiTreeNode` 提供所属树与逻辑索引。
`GuiTreeNode.expanded` 可直接写入；叶节点保持原生恒展开，文件夹修改后立即更新后代可见性与布局，
原生点击或控制器改变展开状态也会同步回字段。
树控件的可写 `GuiTreeSettings` 提供 `indent` 和 `click_to_expand`，可用 `gui.tree_settings(entity)` 查询。
按原版赋值规则，缩进在创建单元格时读取；修改后已有行的 `indent` 子控件保留尺寸，新增行使用新值。
点击策略 `0` 只允许箭头展开，`1` 点击文件夹切换，`2` 保留原版未实现双击切换的行为。
索引在展开/收起间保持稳定，收起的行保留实体但隐藏并从布局中折叠。

```rust,ignore
gui_edit.edit(tree, |object: &mut GuiObjectMut| object.collapse_tree());
gui_edit.edit(tree, |object: &mut GuiObjectMut| object.select_tree_node(3, true));
```

`select_tree_node` 会展开祖先再加入选择；`set_tree_expanded`、`toggle_tree_node`、
`expand_tree` 和 `collapse_tree` 控制展开。全部收起包含顶层文件夹，顶层行仍显示。
箭头的 `expanded` 控制器与节点状态双向同步；行点击遵循包的 `clickToExpand`
配置。状态在 Publish 前、PostLayout 前和 Sync 末尾刷新，未变化时保留变更标记。
`--smoke-tree` 验证导出树的真实点击、缩进、布局折叠、隐藏节点选择和回收。

动态树在控件视图上挂 `GuiTreeContent`，用嵌套数据表达层级，无需操作底层对象：

```rust,ignore
commands.entity(tree).insert(GuiTreeContent::new(vec![
    GuiTreeItem::folder("documents", "Documents", vec![
        GuiTreeItem::leaf("readme", "README.md"),
    ]),
]));
```

key 在整棵树内唯一；相同 key、模板和节点类型会复用行实体，调整顺序后选择状态
跟随对应行，删除的节点回收视图。`GuiTreeState::node_by_key` 可查询当前节点。
新文件夹默认收起；`expanded: None` 保留用户展开状态，`Some(bool)` 在应用配置时
指定展开状态。`icon: Some(url)` 赋图标，空字符串清除，`None` 保留皮肤或当前图标。
`template` 可指定本地模板名或已登记的 `ui://` URL，默认使用树的默认项。

修改组件后按 key 更新，重复 key、无效层级或模板错误会保留当前树，并在
`GuiTreeContentState::error()` 报告；修改组件或标记变更可重试。移除配置组件保留
当前内容和 key，重新挂载可继续复用；空内容回收全部数据行。TreeView 右侧的
动态文件夹和文件已使用这套接口。`GuiEvent::ClickItem` 在列表/树实体上发布一次，
observer 可直接查询本次选择和行文本。

依赖本帧列表布局的装饰系统可放入 `GuiSystems::PostLayout`。这是 Sync 内部的
阶段：原生列表刷新、行视图创建后，插件先更新逻辑层级、几何、文本、滚动及列表
快照，再运行业务系统，最后同步其组件修改并投影视图。循环列表示例
在此查询实际行并修改缩放和序号文本组件，业务不借用 `GuiRuntime`。
该阶段早于 Bevy 的 PostUpdate 布局及文本测量；改变可见范围或行生成的配置应在
`Respond` 或 `Initialize` 修改，使本帧原生列表布局能使用它们。

普通列表可挂 `GuiListContent`，用同一套行接口配置全部数据行：

```rust,ignore
commands.entity(list_entity).insert(GuiListContent::new(6, |index, row| {
    row.set_text(&format!("Item {index}"));
}));
```

更新配置会重绘所有行，前面的行保持对象和实体，新增行按来源包解析默认模板及其
跨包依赖，缩减时销毁尾部行；滚动条、刷新头尾始终排除在数据行之外。移除组件
会释放渲染回调并保留当前内容，清空内容可先设 `item_count = 0`。
`GuiListContentState::error()` 提供配置错误；增长时若模板缺失，已有内容保持不变。
配置只适用于普通 List，树和虚拟列表会报错，`GuiVirtualList` 的配置优先。
业务数据更新后可替换渲染器、替换组件或标记组件已变更；失败配置也可这样重试。

`GuiControllers`（或 `gui.controller(entity, "c1")`）提供当前和上一个页面索引、
页面 ID 及名称表。选中页可通过 ECS 组件直接修改；页面表、名称与上一个页面保持只读。

```rust,ignore
fn change_page(mut controls: Query<&mut GuiControllers>, view: Entity) {
    if let Ok(mut controls) = controls.get_mut(view)
        && let Some(page) = controls.get_mut("c1")
    {
        page.selected_index = 1;
        // 也可使用 page.set_selected_page_id("exported-page-id")
        // 或 page.set_selected_page_name("Settings")。
    }
}

commands.spawn((
    FairyGuiRoot::new(asset_server.load("ui/Basics.fui"), "Demo_Controller"),
    GuiControllers::new([("c1", 1), ("c2", 0)]),
));
```

拥有者上的初始页会等待资源加载，在 `Initialize` 前应用，并回填导出的页面表。
未指定的控制器保留模板初始页，不存在的控制器名被忽略；初始按名称/ID 选择需等待
页面表就绪。按不存在的页面名称选择时回到第一页，不存在的 ID 取消选择。
越界索引规范化为 `-1`，表示未选中。每帧只应用业务修改的选中页，调用原生切页逻辑，
包括齿轮、关联按钮、下拉框、列表选择、分页和控制器动作；不会直接覆盖底层字段。
按钮或滚动带来的页面变化在 Publish 前回写，因此 observer 可读取本帧的选中页，
再通过组件修改另一个控制器；Initialize、PostLayout 的修改也会在本帧渲染前应用。
未变化的组件保留变更标记，无控制器的控件不带该组件。下拉刷新示例使用该方式更新
头尾状态；`--smoke-controllers` 验证真实按钮、齿轮动画和同帧 observer 修改。

下拉刷新消息 `GuiPullRelease.entity` 直接提供滚动容器实体。
`GuiScrollState::header()`、`footer()` 提供刷新控件实体、当前尺寸和导出尺寸，
另可读取刷新轴向及头尾锁定尺寸。在 `Respond` 发送专门的锁定命令：

```rust,ignore
commands.write_message(GuiPullCommand::LockHeader {
    target: release.entity,
    size: header.source_size.y,
});
```

`LockFooter` 同样按逻辑像素锁定尾部，尺寸为零时解锁。命令按发送顺序在 Respond
之后、Advance 之前处理，让松手后的第一个滚动动画 tick 使用正确边界。目标须为
已经构建的滚动容器视图或根拥有者；无效目标、负数及非有限尺寸会被忽略。
下拉刷新示例的页面切换、延迟加载、增加普通行和解锁全部通过 ECS 完成。

窗口用独立的 `FairyGuiWindow` 实体拥有资产和对象树：

```rust,ignore
let mut window = FairyGuiWindow::new(assets.load("ui/Basics.fui"), "WindowA");
window.visible = false;
window.modal = true;
let owner = commands.spawn(window).id();
commands.write_message(GuiWindowCommand::new(owner, GuiWindowAction::Show));
```

资产就绪后插件创建原生窗口，按约定识别 `frame`、`content`、`closeButton` 与拖动区，
默认在 `GuiInputSettings` 选择的窗口内居中。组件的 `visible`、`modal`、`draggable`
可动态修改；`center_on_create = false` 保留导出位置。不要在同一实体上同时挂
`FairyGuiRoot` 和 `FairyGuiWindow`，两者分别拥有独立界面。

`GuiWindowAction` 支持 `Show`、`Hide`、`BringToFront` 和 `Center`，目标接受窗口
拥有者或根视图实体。资产等待期间的命令会保留；替换资产/条目、移除组件或销毁
拥有者会取消旧请求。`Center` 使用当前窗口尺寸。窗口命令在 Sync 的对象属性同步之后
执行，同类命令保持顺序；`GuiMut` 同样支持已就绪的窗口拥有者。

`GuiWindowState` 在拥有者和根视图上提供显示、模态、拖动状态及窗框/内容/关闭按钮
实体，也可通过 `gui.window(entity)` 查询。窗口拥有者支持 `GuiQuery` 的层级和属性
查询。原生关闭按钮会隐藏 ECS 窗口，并把组件的 `visible` 更新为 false；再次显示
复用同一对象和实体。移除组件或销毁拥有者才会回收整个窗口，模态切换保留窗口层级。
可将拥有者作为其他业务实体的子实体，让其销毁时一起回收。Basics/Window 的两个
按钮使用这一套 API；`--smoke-windows` 验证真实关闭按钮、复用、拖动、居中和销毁。

Bag 示例也通过子实体拥有 `BagWin` 窗口，每次打开生成 45 条物品数据，由
`GuiVirtualList` 渲染当前分页。点击行的 `GuiListItemClick` observer 直接读取该行图标，
更新预览 Loader 和地址文字；使用官方示例的十张外部 PNG。关闭窗口保留视图，
关闭所属页面则连同物品行和窗口一起回收。`--smoke-bag` 验证真实按钮/行点击、
外部图片解码、第三页与页控制器联动、关闭重开和页面销毁。

窗口可用 `modal_wait: Some("WindowModalWaiting".into())` 配置等待面板模板，也支持
已登记包的 `ui://` 地址。第一次 `ShowModalWait { request }` 才创建，
`CloseModalWait { request }` 只关闭匹配的非零请求；传零可结束当前任何请求。
等待面板在 `frame/contentArea` 内覆盖内容，标题栏和关闭按钮仍可使用；存在显式
`frame/dragArea` 时窗口优先使用它。没有内容区域时使用窗口的内容组件或整个窗口。

`GuiWindowState` 提供 `is_modal_waiting()`、`modal_wait_request()`、
`modal_wait_pane()` 和 `modal_wait_error()`。模板解析失败仍保留逻辑等待请求，
不会生成遮挡面板；登记依赖后重新发送 Show 可重试。未配置模板时也仅记录请求。
关闭等待会撤下面板并保留对象和实体，再次等待复用它；隐藏窗口会同时结束等待。
替换面板模板回收旧面板，销毁窗口也会回收已经撤下的面板。菜单的 `ShowMenu.source`
现在同样接受窗口拥有者，并按窗口资产来源解析模板、保留资产等待请求；普通弹层的
Show/Hide 使用 `GuiPopupCommand`，原生窗口的显示仍使用 `GuiWindowCommand`。
ModalWaiting 示例中的 Open Window / Refresh 按钮通过 ECS 驱动三秒等待，专项窗口
回归可运行 `--smoke-window-wait`。普通文字按原版不作为鼠标命中目标，父控件仍可
接收点击；富文本链接与输入框继续使用各自的交互路径。

全局等待使用独立资源和命令，覆盖整个舞台：

```rust,ignore
commands.insert_resource(GuiGlobalWaitSettings::new(
    assets.load("ui/ModalWaiting.fui"), "GlobalModalWaiting",
));
commands.write_message(GuiGlobalWaitCommand::Show { message: None });
```

任务结束后发送 `GuiGlobalWaitCommand::Hide` 撤下遮罩，保留对象和视图供下次复用。

`Show.message` 写入模板自身的原生文本属性；无文本属性的组件会忽略它。
模板可用本包名称或已登记包的 `ui://` 地址。请求会等待资产，解析失败可从
`GuiGlobalWaitState::error()` 查询；未创建成功的请求不会拦截输入。状态资源还提供
`is_requested()`、`is_waiting()` 和 `view()`，隐藏后仍可查询保留的实体。

遮罩按输入设置选择的窗口持续调整尺寸，位于原生窗口、菜单和普通界面上方，
鼠标、触摸、滚轮和文本焦点沿统一输入规则拒绝下层目标。全局等待独立于窗口的
模态状态及弹出栈。`Hide` 取消等待中的请求并保留已创建面板；`Dispose` 同时回收
隐藏的面板。更改或清空配置会回收旧模板，仍在请求时会尝试新的配置。命令按
发送顺序在窗口、弹层命令后执行。无 Bevy 窗口时沿用舞台尺寸，首次默认 1280×720。

ModalWaiting 示例进入时显示三秒全局等待，随后可以打开窗口并执行局部等待；页面
提前销毁会取消示例计时并撤下全局遮罩。`--smoke-window-wait` 同时验证全局遮挡、
真实点击拦截、窗口缩放及三秒后恢复，再验证单个窗口的等待行为。

工具提示使用独立模板，等价于原版 `UIConfig.tooltipsWin`：

```rust,ignore
commands.insert_resource(GuiTooltipSettings::new(
    assets.load("ui/Common.fui"), "Tooltip",
));
gui_edit.edit(button, |object: &mut GuiObjectMut| object.set_tooltips(Some("Open settings".into())));
```

模板应有 text/title 属性，也支持已登记的 `ui://` 地址。编辑器导出的 `tooltips`
自动生效，`GuiObjectInfo::tooltips()` / `gui.tooltips(entity)` 可读取当前配置。
鼠标悬停 100 毫秒后读取当前文字并显示；离开、隐藏、失去窗口焦点、被模态层阻挡
或销毁来源时隐藏。默认放在鼠标位置加 `(10, 20)`，按原版边缘规则调整，显示后
不随鼠标移动。子控件共享最近祖先的提示，切换其内部子节点不会重置延迟。

业务可发送 `GuiTooltipCommand::Show { text, position }` 立即显示，或用
`ShowView { view, position }` 指定自定义根拥有者/控件视图。`position: None` 使用
鼠标偏移，显式位置为窗口逻辑坐标，经舞台坐标转换后定位。提示与 popup 栈独立，
鼠标和触摸输入穿透提示；指针按下会隐藏当前提示，但保留尚未触发的悬停计时。
`Hide` 保留默认模板，`Dispose` 还会回收它并取消当前悬停计时。自定义视图在隐藏后
从舞台摘下，生命周期仍由原拥有者管理，所属页面销毁会回收已摘下的自定义子控件。

`GuiTooltipState` 提供请求、实际显示、视图、悬停来源和错误状态；默认模板等待资产
期间保留请求，配置更正后自动重试。命令可在 `Respond`、`Initialize` 或 `PostLayout`
发送并在本帧处理。`--smoke-tooltip` 验证真实悬停、边缘定位、复用及提示覆盖按钮时
的点击穿透。

全局滚动默认值通过 `GuiConfig` Bevy 资源配置；scroll pane 导出项若保留默认值，
会使用 `GuiConfig.scroll` 中的滚动条显示、触摸滚动、回弹、滚轮步长和分页阈值。
导出包显式设置的显示方式及触摸/回弹开关优先。配置在创建时捕获，修改资源影响
后续创建的界面，已有界面保持当前设置。

Windows MSVC 示例构建自动使用 8 KiB PDB 页，容纳完整 Bevy 调试符号，避免默认页容量
触发 `LNK1318 LIMIT`；其他目标保持原有链接配置。

Transition 演示播放期间可用 **Space** 暂停/恢复，**↑/↓** 在 0.25×～4× 之间
调整速度，**R** 从反向时间轴的起点重新播放。

应用可指定过渡的播放区间，时间单位为秒：

```rust,ignore
use fairygui_core::transition::TransitionPlayOptions;

runtime.store_mut().play_transition_with_options(owner, "t0", TransitionPlayOptions {
    start_time: 0.5,
    end_time: Some(1.5),
    ..Default::default()
});
```

起点之前的属性会直接还原，历史音效与起始标签会跳过；已完成补间的结束标签仍可触发。
正向跳转同时补偿序列帧。`end_time: None` 表示不截断，`reversed: true` 使用反向时间轴。

序列帧和 Loader 的播放属性直接修改 `GuiAnimationState`：

```rust,ignore
fn pause_at_frame(mut animations: Query<&mut GuiAnimationState>, movie: Entity) {
    if let Ok(mut animation) = animations.get_mut(movie) {
        animation.playing = false;
        animation.frame = 3;
        animation.time_scale = 2.0;
    }
}
```

Respond 中的暂停在本帧动画推进前生效；只修改速度或播放开关会保留当前帧及其计时。
未编辑的字段跟随播放器、控制器和过渡变化，连续播放不会被旧快照反复跳回。
可在等待资产的根拥有者上挂载 `GuiAnimationState::new(false, 3, 2.0)` 设置初始属性；
其默认值为播放开启、第 0 帧、1 倍速度。`frame_count()` 和 `is_ended()` 保持只读。
Loader 的 URL 改变后，旧播放器的帧数和结束状态不会用于新 URL。

分段播放、时间补偿和回到开头等操作通过 `GuiMut` 调用：

```rust,ignore
gui_edit.edit(movie, |object: &mut GuiObjectMut| object.set_animation_play_settings(GuiAnimationPlaySettings { start: 1, end: 3, times: 2, end_at: 2 }));
commands.entity(movie).observe(|event: On<GuiAnimationComplete>, gui: GuiQuery| {
    let state = gui.animation(event.entity).unwrap();
    assert!(state.is_ended());
});
```

`set_animation_playing`、`set_animation_frame`、`set_animation_time_scale`、
`set_animation_play_settings`、`advance_animation` 和 `rewind_animation` 在 Sync 同帧
更新播放器、纹理及 `GuiAnimationState`；可在 `PostLayout` 中继续调用方法，不会重复
推进帧时间。`gui.animation(entity)` 同时接受根拥有者与控件视图，提供当前帧、帧数、
播放开关、速度和结束标志，未变化时保留组件变更标记。静态或未就绪 Loader 的帧数为 0。

`end/end_at` 为 `-1` 时使用末帧，`times <= 0` 无限循环。配置不改变 Playing 或速度；
到达结束帧后，下一次满足帧计时条件的更新定位到 `end_at`，在 Sync 刷新最终快照后
向控件视图实体发送一次 `GuiAnimationComplete { entity }`，也可用 `MessageReader`
读取。完成保留 Playing 开关；`rewind_animation` 只重置位置和计时，重新配置播放才会
清除已结束状态。新播放、资源替换或销毁会取消尚未发布的旧完成通知。
`advance_animation(seconds)` 按原版 DeltaTime 补偿帧数，不受 Playing 和速度影响。
负数或非有限时长/速度、非法播放区间会忽略，帧号按资源范围夹取。

过渡支持运行中按标签编辑：`set_transition_value` 使用类型化的
`TransitionEditValue`，`set_transition_target` 修改同名起点的目标，
`set_transition_duration` 修改同名起点的补间时长，`transition_label_time` 查询标签时间。
取值更新所有匹配项，时长不重算导出的总时长；已创建的补间数值留到下一轮生效，
未触发的离散动作读取新值。`set_transition_hook(..., Some(42))` 只绑定第一个匹配项，
通过 `GuiTransitionCallback` 返回编号；`clear_transition_hooks` 清除这些用户绑定，
原有 `GuiTransitionHook` 标签消息继续发布。过渡的 `times=0` 只播一轮，负数无限循环，
与序列帧的循环计数规则不同。

`set_transition_auto_play(owner, "t0", true)` 配置进入舞台时自动播放（默认无限循环、
无延迟），`set_transition_auto_play_with_options` 可指定次数和延迟。同一开关值不会
覆盖原配置；在舞台上关闭自动播放不会中断当前播放。`set_root_attached(index, false)`
撤下并隐藏界面，保留对象与实体；重新挂载时按自动播放规则启动。

仓库内已放了一份取自官方示例工程的资源，开箱即可运行：示例默认加载
**`MainMenu.fui`**，从主菜单点击条目切换演示界面。仓库随附的 `Chat` 演示带一条
纵向滚动条，可以直接看到滚动条的实例化与联动（拖动滑块、点击箭头与轨道、滚动时
出现并在停止后淡出）；`Bag` 则用于查看序列帧。若要更换启动界面，请修改
`crates/fairygui-demo/src/main.rs` 中的 `MENU_PACKAGE` 与 `MENU_ITEM`。

滚动条保持在视口边缘，不计入列表行索引、布局或回收；业务访问列表项请使用
`list_items` / `list_item_at` / `list_item_index`。详见
[列表与滚动条](docs/list-scrollbars.md)。`--smoke-list-bars` 可运行 Chat 滚动条的窗口回归。
箭头和轨道在按下时立即滚动，松开不重复；轨道方向按指针相对滑块的局部位置判断。
滚轮支持启用开关、自定义步长、页模式和嵌套冒泡；行式输入按原版取方向与双倍步长，
像素式输入保留平滑移动，详见 [滚轮输入](docs/scroll-wheel.md)。
分页控制器支持导出绑定、双向联动、运行时绑定/解绑与当前页命令；初始页在列表和
视口就绪后应用。`--smoke-scroll-pages` 使用 BagWin 的真实分页列表验证，详见
[分页控制器](docs/scroll-pages.md)。
原版滚动命令还支持按方向和尾页计算分页吸附，以及普通组件/非虚拟列表的子项吸附；
使用 `set_scroll_pos_animated` 进入对齐路径，原有 `set_scroll_pos` 保留原始位置语义。
详见 [滚动对齐](docs/scroll-alignment.md)。
内容拖拽支持原版速度采样、释放惯性、分页限制、条目吸附和双向回弹；Bevy 保留
逻辑边界并绘制越界内容。时序参数与验证范围见 [拖拽与释放](docs/scroll-motion.md)。
下拉/上拉刷新会创建导出的页头/页脚，发布 `GuiPullRelease`，支持响应期间锁定和
解锁回弹；`--smoke-pull-refresh` 验证完整示例流程，详见 [下拉刷新](docs/pull-refresh.md)。

解析器自带命令行工具，可脱离渲染查看包结构：

```bash
cargo run -p fairygui-core --bin dump_fui -- crates/fairygui-core/tests/fixtures/MainMenu.fui --items
```

## 能力矩阵

### 已实现

| 模块 | 内容 |
| --- | --- |
| 包解析 | 六块索引表、zlib 压缩、分支与高清替代资源、字符串表两段式编码、自定义字符串 |
| 资源 | 图集切片映射、位图字体、序列帧、像素级命中数据 |
| 对象模型 | 竞技场 + 稳定句柄、对象树、脏标记增量同步、事件队列与冒泡 |
| 容器 | 递归构建对象树、控制器、关联、整体属性、过渡表 |
| 控件 | 图片、静态文本、输入框、矢量图形、分组、装载器、序列帧、按钮、标签、进度条、滑动条、列表、树、下拉框、滚动条 |
| 状态驱动 | 控制器（构建完成后执行首屏动作；动作同时匹配来源页与目标页 ID；跨对象切页按导出实例 ID 与页面 ID 查找，支持 `~1` 序号转发 / `~2` 名称转发；过渡支持启动延迟、离页取消/完成补间，再次触发时保留播放进度；连锁切页防重入）、10 种齿轮（坐标、尺寸与外观支持缓动、延迟与重复目标保留进度；颜色与动画按官方行为立即应用；显示齿轮与显式可见性分开，支持补间显示锁、Display2 的 AND/OR 条件与分组显隐传播）；运行时属性按原版 setter 回写当前页，支持默认快照、未选中页独立存储、逐对象锁和换父节点后保留绑定，详见 [齿轮状态](docs/gear-state.md) |
| 布局 | 关联（父节点与兄弟对象两种目标）、列表四种布局与分页、树的分层缩进；列表项会套用标题、图标与指定的控制器页面；分组移动同步到同级成员（含嵌套组与隐藏成员），组透明度按官方规则赋值给成员；装载器支持六种填充方式、水平/垂直对齐、只缩小与自动尺寸 |
| 滚动 | 方向约束、边界收拢、惯性衰减、越界回弹、拖拽与速度继承、滚轮；支持按对象/矩形/列表项滚入视口、点击自动滚入、页对齐与 0.5 秒缓动，内容/滚动条/虚拟范围按显示位置同步，详见 [滚入视口](docs/scroll-to-view.md)；滚动条按资源地址实例化、贴合容器边缘摆放、滑块随滚动位置与内容比例伸缩，滑块可拖动、箭头与轨道可点击，启用 display_in_demand 时内容放得下会隐藏整条，视口放不下箭头时也隐藏（「不显示」则不创建）；Auto 条按轴独立受拖动/动画与滑块按住状态控制，停止后延迟 0.5 秒再以 QuadOut 淡出，隐藏时 alpha 恢复为 1，详见 [显隐生命周期](docs/scrollbar-visibility.md) |
| 动画 | 过渡（逐项补间、重复/往返、跨循环剩余时间、导出运动路径、百分比坐标与容器自身的相对位移；支持同组件与子组件的嵌套调用、多层嵌套、等待子过渡完成、停止标记和停止传播；起点/终点标签通过 `GuiTransitionHook` 通知，`endLabel` 不改变时间轴；动作项含位移/尺寸/缩放/旋转/透明度/颜色/文本/图标/可见性/轴心/斜切、抖动、颜色滤镜、序列帧播放与音效入队），可选择完成或保留当前属性值来停止，抖动和显示锁会清理；通用补间支持重复/往复/取整/暂停/路径，提供 31 种缓动与四种路径曲线 |
| 交互 | 按钮状态页、Common/Check/Radio 点击、关联控制器双向切页、选中标题/图标与单选组深度调整（`autoRadioGroupDepth`）；列表单选/多选（Ctrl 切换、Shift 范围与单击多选）、选中按钮外观、父选择控制器双向联动与行点击消息，详见 [列表选择](docs/list-selection.md)；进度条与滑动条取值（含按坐标反推）、树的展开折叠与「展开到指定节点」；下拉框在按下时展开，填充标题/值/图标、对齐宽度并自动上下定位；选择或重选发出状态变化并收起；普通弹窗、菜单与下拉框共用按下关闭栈，关闭保留完整内容和实体 |
| 文本 | 位图字体按本包名称或跨包 `ui://` 地址解析，支持运行时与普通/富文本后端切换、迟到字体资源重试及独立 UBB/HTML 开关；切换保留对象、实体和应用子节点。使用原版字号缩放、逐字步进取整、换行/对齐/缩小/省略号、逻辑像素自动尺寸与裁剪字形偏移（按 tint 标记保留彩色纹理）；文本/字号/尺寸变化时从所属包重排。普通字体支持系统字体族/候选列表及外部 TTF/OTF 异步加载，加载期间回退，迟到资源切换保留输入组件，详见 [普通字体](docs/raster-fonts.md)。支持普通文本 UBB 样式；普通位图字段已按原版行几何绘制下划线，`set_object_underline` 支持普通/位图后端动态切换；位图 UBB 已接入跨样式段换行、独立字号/颜色、下划线/删除线、内联图片与链接命中，详见 [位图富文本](docs/bitmap-rich-text.md)。支持跨包及外部静态内联图片与链接点击消息；外部图片通过 AssetServer 异步加载，按独立显式轴/自然尺寸回填并重排，失败时隐藏并保留显式占位；富文本内容变化时重建专属段落和图片，保留对象子树；图片跟随文本的可见性、层级、变换、归池与销毁。位图排版范围见 [说明](docs/bitmap-text.md) |
| 运行时 | 对象池、多包注册表（三键索引 / 分支 / 全局变量 / `ui://` 地址解析）、多语言翻译表、分帧构建 |
| 根节点与窗口 | 内容层与弹窗层、模态阻挡与「输入只交给最上层模态窗口及其上方弹层」、层内顺序调整；窗口按条目创建、自动居中、显示隐藏切换、点击置顶、标题栏拖拽移动、关闭按钮联动、模态等待面板（带命令编号，多个并存请求互不干扰） |
| 渲染 | 绝对定位、图集子区域、九宫格与平铺、裁剪、轴心与旋转缩放、横纵与 90°/180°/360° 径向填充遮罩、着色与不透明度、图片翻转；矩形（含四角圆角）与椭圆使用节点样式，多边形与正多边形使用 UI 材质绘制填充与描边；透明度沿子树累积，容器变淡会带着子对象一起变淡 |
| 输入 | 悬停 / 鼠标移动 / 左键与右键按下、抬起和点击 / 按住拖动及拖动结束 / 多点触控与独立触摸捕获、触摸点击和拖动 / 滚轮、事件冒泡、像素级命中、按钮状态联动、列表选中、树的点击展开、标为不可点击的对象不响应、命中区域可指定为「某个子对象」或「某条目的像素数据」；指针拖拽：拖动滚动条滑块、拖动滑动条取值、拖动滚动容器内容（松手后按速度继续滑行）、拖动窗口标题栏移动窗口；拖拽与放置：跟手图标随指针移动，成功放置时在命中对象及其逻辑祖先收到带来源、业务数据和触摸 ID 的 `Drop` 事件 |
| 输入框编辑 | 普通输入框已接入 restrict 字符限制、UTF-16 长度预算及单行/多行模式；输入、粘贴和 IME 提交共用规则，预编辑保留，异步粘贴保持队列顺序；程序赋值绕过字符/长度限制。支持提示文字（普通/位图字体）、独立提示色和只读选择/复制；提示不进入模型或编辑缓冲。密码字段按 UTF-16 单元显示 ●，普通输入保留真实编辑值并映射遮罩点击、选择和 IME 几何，支持运行时开关。详见 [输入框编辑](docs/text-input.md) |
| 输入框焦点与事件 | 普通输入框通过 FairyGUI 命中/模态规则取得焦点，接收 Bevy KeyboardInput 和 IME 消息；逻辑键名随 KEY_DOWN / KEY_UP 事件沿 FairyGUI 对象树冒泡；支持运行时焦点请求、键盘输入仲裁、只读/窗口失焦清理与 IME 候选位置。GuiTextInput 提供目标级 INPUT / ENTER / CHANGE / FOCUS / BLUR 通知，Enter 可在 Respond 阶段取消默认结束 |
| 文本描边 | 普通字体使用实际排版字形的矢量轮廓描边，支持字段宽度/颜色、UBB/HTML 分段覆盖、运行时更新、普通输入/密码/提示文字、DPI 与输入滚动裁剪；描边和填充预合成后应用对象透明度，支持样式 alpha、本地滤镜与纹理复用；描边不改变文字布局，位图字体保持原版图片字形路径。透明叠加等边界见 [描边说明](docs/text-stroke.md) |
| 生命周期 | 舞台连接状态独立于显式可见性；进入舞台按自动播放配置启动，离开时按导出选项保留当前值停止/完成后停止/继续运行；界面撤下后保留并隐藏实体、再次挂载复用；逻辑父节点同步到 Bevy 层级；销毁回收整棵子树与映射；对象池、分帧构建的失败隔离、动态新增子对象的实体同步（父实体未就绪时延后）；窗口隐藏保留对象，重新显示与关闭分别执行挂载和销毁 |
| 声音 | 过渡与控件排队的音效由使用方取走并交给音频后端播放；自动播放走 Bevy 音频系统（一次性实体、放完回收），按声音所属包解析目录，音量夹在 0~1；显式启用 WAV/MP3 解码以支持官方示例资源 |
| 工具 | 包内文件名到资产路径的解析、`ui://` 地址的生成与反查、**多包注册表与跨包地址解析**（创建同步/异步根时自动登记包；`register_package` 返回尚未加载的依赖名）、根容器整体缩放 |

### 已知差异与限制

| 特性 | 现状 |
| --- | --- |
| 骨骼动画 | 不支持。3D 装载器会给出告警并降级为普通容器 |
| 分帧构建粒度 | 按**条目**分帧。单个特别庞大的界面仍会在构建它的那一帧占用较多时间 |
| 图片填充遮罩 | 横向、纵向与 90°/180°/360° 径向已接入 UI 材质，按原版起点、方向与进度裁剪纹理，保留布局尺寸；支持 Image、Loader 图片与序列帧内容。进度条使用填充模式时更新 `fillAmount`，不缩小图片。详见 [图片填充](docs/image-fill.md) |
| 颜色滤镜 | 图片、文字、位图字形、内联图片、Loader 内容与图形使用统一离屏路径，在子树合成后应用亮度、对比度、饱和度与色相矩阵；支持嵌套滤镜、显式恒等滤镜隔离与组透明度，保留填充遮罩和九宫格几何。无渲染器时保留本地投影回退。详见 [颜色滤镜](docs/color-filters.md) 与 [离屏合成](docs/compositing.md) |
| 混合模式 | 接入 Normal、Add、Multiply、Screen、Erase、Mask，支持父级继承、显式 Normal 覆盖与 reset；滤镜边界将混合和累积透明度转移到合成结果。Screen 遵循所查 Laya 版本的加法因子；其余枚举和边界限制见 [离屏合成](docs/compositing.md) |
| 九宫格平铺 | 已按 FairyGUI-Laya 链路接入 Image 和跨包 Loader，支持边缘/中心平铺、尾块裁剪、窄目标的按轴边框压缩、裁剪切片 UV、翻转及填充遮罩。原版把非零 `tileGridIndice` 转为统一平铺开关，并非九个格子的独立位。通过同一 UI 材质绘制，无需额外九个节点。详见 [九宫格绘制](docs/nine-grid.md) |
| 列表虚拟化的项管理 | 五种非循环布局已接入可见行自动构建、同步 renderer/provider、尺寸缓存、按方向复用与 FIFO 池；离屏选择和定位使用逻辑索引，Bevy 保留池内实体并在滚动/改尺寸后自动刷新。VirtualList 使用 1000 条数据，原始视口通常只需 5 个行对象。循环列表支持原版允许的单列、单行和分页布局，使用六倍内部记录，并在命令、滚轮/箭头、拖拽与缓动期间重定位和映射索引；LoopList 已接入五张图片、中心放大和索引文字。原有手工范围/回收/通知 API 保留。详见 [虚拟列表](docs/virtual-lists.md)和[循环列表](docs/loop-lists.md) |
| 图形控件 | `GuiGraph` 接入动态形状、线条、颜色、圆角、顶点与距离系数；多边形使用 `UiMaterial`，支持凹轮廓、原版折角描边和轮廓命中。矩形使用节点样式，圆使用保留声明布局的独立正方形节点，并统一普通图形/遮罩命中。原版像素抗锯齿、半透明填充对对象 alpha 的副作用尚未完全复刻。详见 [图形绘制](docs/polygon-rendering.md) |
| 对象关联 | 支持父节点和兄弟目标，目标移动时同步对齐与扩展边；比例关系按目标尺寸比率缩放位置、尺寸与扩展边，非比例关系按尺寸增量应用；目标锚点参与兄弟关系计算，并在循环关系中阻止重入 |
| 文本自动尺寸 | Both/Height 文本默认在原生布局与视图生成前同步测量，普通字体使用与渲染一致的窗口/DPI/UiScale 倍率，位图字体复用原生测量器；文字修改、空文本和迟到资源在本帧更新关联布局，字体未就绪时保留当前尺寸并重试。Fixed/Shrink/Ellipsis 保留外框；普通文字的断行差异见下一项 |
| 文本换行的完整规则 | 普通与 UBB 位图文本已按 Laya 3.3 的命令、单词边界、标点回退与 UTF-16 字形测量处理；UBB 支持跨样式段移词和图片占位。普通/富文本依赖 Bevy，仍有断行差异；完整 Unicode 断行和普通文字的原版排版仍未覆盖 |
| 富文本排版与链接 | UBB 与 HTML 采用原版转换/样式栈，支持实体、相对字号、嵌套/错配标签及默认链接下划线；RichText 关闭 UBB 后保留 HTML。粗细、斜体、下划线与删除线已映射到 Bevy，实际字形取决于字体支持；内联图片用 Bevy `InlineBox` 按原始画布和左右留白参与测量、换行，裁剪切片保留偏移并在本帧布局后定位；包内 MovieClip 地址会在普通和位图富文本中按舞台生命周期播放；行内垂直居中、行高和取整仍与 Laya 存在差异；UBB 链接按实测文字段命中并原样携带地址，发布 `GuiLink` 和冒泡的 `EventKind::Link`；外部静态图片已接入两种文本后端，详见 [外部内联图片](docs/external-inline-images.md)；HTML 表单对象仍未接入，详见 [标记解析](docs/markup.md) |
| 文本描边的剩余边界 | 普通矢量文字已支持描边/填充预合成及透明度；原版浏览器字形像素、整串/逐字缓存边界、彩色字体描边和完整文本/容器离屏合成仍待对齐。详见 [描边说明](docs/text-stroke.md) |
| 按钮的附加效果 | 选中状态与控制器页已联动，点击改变选择时投递一次 `StateChanged`；`downEffect` 已接入按下缩放与非文本直接子控件压色；松开恢复原尺度或白色，按住移出保持效果；`linkedPopup` 支持运行时绑定的对象/窗口，在按下时切换，普通弹层支持栈、外部按下关闭和保留视图重用；`fire_button_click` 接入原版内部点击逻辑及 Common 按钮 100/200 ms 反馈，不派发普通 `Click`，详见 [按钮交互](docs/button-interaction.md) |
| 装载器的扩展内容 | 图片与序列帧使用独立内容节点；`ui://` 组件内容支持跨包资源、嵌套 Loader、缩放或 `useResize` 重排、按钮输入与过渡生命周期。已接入可配置失败提示图、FIFO 对象复用、加载状态查询和显式重试；切换 URL 清理旧内容。详见 [Loader 组件与加载状态](docs/loader-components.md) |
| 置灰 | 叶对象的离屏结果使用 Laya 灰度矩阵；置灰暂时覆盖本地滤镜，取消后恢复。文字默认前景使用 `#AAAAAA`。组件优先切换 `grayed` 控制器，否则向子对象赋值；按钮优先使用禁用状态页。支持导出实例初始化、首次 false、重复赋值与换父节点语义，详见 [组件置灰](docs/grayed-state.md) 与 [离屏合成](docs/compositing.md) |

部分不支持的子特性通过 `tracing` 输出提示；完整限制以此表及各专题文档为准。

## 测试

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
```

核心层的解析测试使用**官方示例工程导出的真实 `.fui` 文件**作为夹具
（`crates/fairygui-core/tests/fixtures/`，MIT 许可，来源见该目录 `README.md`），
覆盖包结构、对象树、控制器、齿轮、关联、列表、滚动、过渡、进度条等链路。

## License

MIT
