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
| `crates/fairygui-bevy-macros` | 派生宏（预留：自定义扩展组件注册）。 |
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
两类命令在 Sync 中按 `GuiObjectCommand`、`GuiPopupCommand` 的顺序执行；
同一类消息保持顺序。

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
- Bevy 0.20.1（2026-11-12 再次核实为最新稳定版，MSRV 为 Rust 1.97.1）

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

fn on_menu_button(event: On<GuiEvent>, mut commands: Commands) {
    if event.event().event.kind == fairygui_bevy::core::object::EventKind::Click {
        commands.write_message(GuiObjectCommand::new(
            event.entity,
            GuiObjectAction::SetText("Clicked".into()),
        ));
    }
}

// 在 Sync 后绑定本帧刚创建的控件。
app.add_systems(Update, bind_menu.after(GuiSystems::Sync));
```

`child_by_name` 查找直属子节点，`find_by_name` 按 FairyGUI 逻辑顺序递归查找，
`child_by_path` 按 `/` 分隔的子节点名查找；都接受根拥有者或控件视图实体。
控件上的只读 `GuiObjectInfo` 提供名称、类型、父节点及有序子节点，排除渲染辅助实体，
隐藏或暂离舞台的控件仍可查询。数据在 `GuiSystems::Sync` 更新；Respond 阶段读取
上一帧的快照，Sync 后可查询本帧新建的控件。高级模型操作可通过
`GuiRootInstance` 获取底层对象 ID 和 root handle。

`GuiTransform` 提供控件的逻辑位置、未缩放尺寸、缩放、轴心、旋转与斜切；
可用 `Query<&GuiTransform>` 查询视图实体，或用 `gui.transform(root_or_view)` 读取。
坐标相对 FairyGUI 逻辑父节点，旋转和斜切以度为单位，不包含滚动偏移、轴心补偿
或祖先变换，也不依赖 Bevy 布局是否已完成。例如根界面居中只需查询 ECS：

```rust,ignore
fn center_root(gui: &GuiQuery, commands: &mut Commands, root: Entity, viewport: Vec2) {
    if let Some(transform) = gui.transform(root) {
        commands.write_message(GuiObjectCommand::new(
            root,
            GuiObjectAction::SetPosition((viewport - transform.size()) * 0.5),
        ));
    }
}
```

几何状态在 Sync 末尾更新，包含本帧命令、控制器、分组和动画引起的变化，
未变化时不会触发 `Changed<GuiTransform>`。通过 `SetPivot { pivot, as_anchor }`
可同时配置比例轴心与锚点定位。`GuiFont` 资源也在根构建前同步，首帧和后续动态
界面会直接使用配置的字体，无需业务系统手工调用 `GuiRuntime::set_font`。

`GuiText::value()` 或 `gui.text(entity)` 读取 FairyGUI 原始文本：按钮返回基础标题，
标签和下拉框转发到标题子控件；输入框返回已提交内容，不含提示文字、密码掩码、
IME 预编辑或翻译/UBB 渲染结果。无文本属性的控件没有 `GuiText`。
`GuiDisplay`（也可用 `gui.display(entity)`）分别提供显式显隐、控制器/分组显示状态、
舞台连接状态、本地/祖先累积透明度、置灰和输入开关。`display_visible()` 是本地
显示状态，不包含祖先显隐、裁剪或遮挡；隐藏和离开舞台也是独立状态。

文本和显示状态会在发布输入事件前、Sync 结束时各刷新一次，因此 observer 能读取
本帧输入刚提交的值，再发命令在同帧更新其他控件。没有变化时保留 ECS 变更标记。
`SetTouchable(bool)` 通过命令修改输入开关。富文本链接消息 `GuiLink.entity`
直接提供原始目标的控件实体，可据此查询、发命令或匹配业务组件。

滚动容器上的只读 `GuiScrollState`（或 `gui.scroll(entity)`）提供逻辑目标位置、
动画当前显示位置、视口/内容尺寸、最大位置、两种位置对应的进度比例及当前页。
它在输入事件发布前和 Sync 结束时更新，没有滚动容器的对象不带该组件。
`ScrollTo { position, animated }` 按原生分页/条目对齐规则滚动；
`SetScrollPercentage { percentage, animated }` 将比例限制在 0～1，并在执行时读取
实际范围，因此到顶部/底部无需业务代码提前计算尺寸：

```rust,ignore
commands.write_message(GuiObjectCommand::new(
    list_entity,
    GuiObjectAction::SetScrollPercentage { percentage: Vec2::Y, animated: false },
));
```

`ScrollToView { rect, animated, set_first }` 把内容坐标中的矩形滚入视口；
`set_first` 为真时优先对齐矩形起点，最终位置仍受分页、条目吸附和边界约束。
原有 `SetScrollPosition` 直接设置
位置，允许选择是否越界；动画是否进行中可通过 `is_animating()` 查询。

`GuiTransitions` 提供控件上全部过渡的 ECS 快照；`gui.transition(entity, "t0")`
可读取播放中、暂停、反向、时间、导出时长、速度及播放编号。状态在 Publish 前和
Sync 末尾更新。`PlayTransitionWithOptions { name, options }` 支持延迟、区间、
重复次数和反向播放，与核心 `TransitionPlayOptions` 使用同一规则。

自然播放结束通过 `GuiTransitionComplete { entity, transition, play_generation }`
通知，包含所有重复及等待中的嵌套任务；同帧开始并结束的短过渡也会通知。
停止、重播替换、离开舞台和销毁不会伪造自然完成。业务可比较当前状态的
`play_generation()`，避免把旧一次播放的完成消息用于新一次播放。
消息在 Sync 后发布，Respond 系统下一帧读取；放在 Sync 后的业务系统可同帧读取。
`GuiTransitionHook` 使用 `entity` 与 `hook` 字段，`GuiTransitionCallback` 使用
`entity` 与 `callback` 字段，也在 Sync 后发布。demo 通过完成消息回收临时界面。

业务系统可在 `GuiSystems::Respond` 用实体发送 `GuiObjectCommand`，本帧 Sync 会把
修改应用到对象模型并投影到 Bevy UI 视图：

```rust,ignore
commands.write_message(GuiObjectCommand::new(
    root_entity,
    GuiObjectAction::SetVisible(false),
));
```

命令目标既可用根拥有者实体，也可用带 `GObjectRef` 的控件视图实体。
对根拥有者的命令可以在 `spawn(FairyGuiRoot::new(...))` 后立即发送：资产等待期间
按顺序保留，就绪后先应用命令再生成视图；根被移除、替换或销毁会取消其等待命令。

`GuiRuntime` 和 `ObjectStore` 仍可用于高级控件 API；大多数界面的创建、常见属性修改与销毁由
ECS 组件和消息驱动。`GuiObjectAction` 覆盖位置、尺寸、显隐、文本、颜色、置灰、
按钮选择、滑动条值、控制器切页、进度值、滚动位置、列表选择替换/清空/滚入视口和
过渡播放。命令在 `GuiSystems::Sync` 应用，因此 observer 和业务系统只需写消息，
不需要独占借用运行时。
过渡命令还支持暂停/恢复、速度、反向播放和停止。临时界面也用 `FairyGuiRoot`
实体管理；销毁所有者实体会自动回收对应 FairyGUI 对象树。
列表、进度条、滑动条和按钮实体分别带有 `GuiListSelection`、`GuiProgressValue`、
`GuiSliderValue`、`GuiButtonSelected` 查询组件。它们在同帧 Sync 后反映最新的控件状态；
状态写入统一通过 `GuiObjectCommand`，可以直接用 ECS 查询驱动其他业务系统。
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

序列帧可通过 `ObjectStore::set_animation_play_settings(movie, settings)` 配置
`AnimationPlaySettings { start, end, times, end_at }`：`end/end_at` 为 `-1` 时
使用末帧，`times <= 0` 无限循环。配置不改变 Playing 或速度；到达指定结束帧后，
下一次满足该帧计时条件的更新定位到 `end_at`，发布一次 `GuiAnimationComplete`。`rewind_animation`
只重置位置和计时，重新配置播放才会清除已结束状态。

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
| 图形控件的多边形 | 任意多边形与正多边形已通过 `UiMaterial` 绘制，支持凹轮廓、距离系数、填充与原版折角描边、动态尺寸/颜色/透明度、轮廓命中与凹口穿透。矩形与椭圆继续使用节点样式；原版像素抗锯齿、图形半透明填充对对象 alpha 的副作用尚未完全复刻。详见 [多边形绘制](docs/polygon-rendering.md) |
| 对象关联 | 支持父节点和兄弟目标，目标移动时同步对齐与扩展边；比例关系按目标尺寸比率缩放位置、尺寸与扩展边，非比例关系按尺寸增量应用；目标锚点参与兄弟关系计算，并在循环关系中阻止重入 |
| 文本自动尺寸 | 普通文本依赖引擎布局回读，存在一帧延迟；位图文本使用独立逻辑像素测量回填 Both/Height，包含空文本和缺字文本，保留 Shrink/Ellipsis 的外框 |
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
