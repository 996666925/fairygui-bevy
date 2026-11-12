# demo 资源目录

把 FairyGUI 编辑器导出的资源放到本目录下的 `ui/` 子目录中。

## 需要放什么

1. 在 FairyGUI 编辑器中打开 UI 工程，选择「文件 → 发布」，
   发布格式选择**二进制（`.fui`）**，与官方运行时保持一致。
2. 把生成的 `<包名>.fui` 与其图集一起放进 `ui/`。图集与包同名，
   形如 `<包名>_atlas0.png`，图集较多时还会有 `_atlas1.png` 等。
3. 若包内使用了位图字体或序列帧且它们引用独立纹理，也一并放入。

```
assets/
└── ui/
    ├── MainMenu.fui
    ├── MainMenu_atlas0.png
    └── ...
```

仓库里已经放齐 FairyGUI 官方 Laya 示例工程的 15 个资源包，让
`cargo run -p fairygui-demo` 开箱即可运行。启动后进入**主菜单**，
点击条目切换演示，右下角的关闭按钮退回菜单：

| 包 | 菜单项 | 用来演示 |
| --- | --- | --- |
| `MainMenu` | 启动即进入 | 主菜单本身：按钮、图片与九宫格 |
| `Basics` | `n1` | 控制器、齿轮、按钮与可滚动容器 |
| `Transition` | `n2` | 过渡动画与缓动 |
| `Joystick` | `n9` | 摇杆与输入 |
| `Bag` | `n10` | 序列帧（影片剪辑） |
| `Chat` | `n11` | 纵向滚动条：拖动滑块、点击箭头与轨道、按需显示的淡出 |
| `ListEffect` | `n12` | 列表项过渡效果 |
| `ScrollPane` | `n13` | 滚动容器与列表交互 |
| `TreeView` | `n14` | 树节点展开与折叠 |
| `Guide` | `n15` | 引导页与高亮区域 |
| `Cooldown` | `n16` | 冷却进度展示 |

`Basics`、`Transition`、`VirtualList`、`LoopList`、`HitTest`、`PullToRefresh`、
`ModalWaiting` 也已随仓库提供，对应主菜单中的 `n1`、`n2`、`n4`、`n5`、`n6`、
`n7`、`n8`。所有 15 个菜单项均可加载；其中部分演示的业务逻辑仍由 Rust
使用方扩展（例如虚拟列表数据源和聊天发送回调）。

## 换成自己的界面

`main.rs` 顶部的两张表决定了菜单怎么映射：

```rust
// 菜单条目 → 演示包。每个演示都以 Main 作为入口界面。
const DEMOS: &[(&str, &str)] = &[("n1", "Basics"), ...];
// 仓库里确实带了资源的包。点其余的会提示。
const AVAILABLE: &[&str] = &["Basics", "Transition", ...];
```

把导出的 `<包名>.fui` 与图集放进 `ui/`，再把包名加进 `AVAILABLE` 即可。
菜单条目名（`n1`、`n2`……）要与 FairyGUI 编辑器里按钮的名字一致——
`DEMOS` 里的对应关系取自官方示例的 `MainMenu.ts`。

## 只想验证解析

解析器自带测试夹具，不需要任何外部资源：

```bash
cargo test -p fairygui-core
```

也可以单独查看某个包的结构：

```bash
cargo run -p fairygui-core --bin dump_fui -- crates/fairygui-core/tests/fixtures/MainMenu.fui --items
```

## 内联外部图片测试资源

`images/inline-swatch.png` 是本项目自行生成的 64×32 RGBA PNG（148 字节），
包含红、绿、蓝、灰、自定义蓝、半透明橙、透明洋红和白色八条色带，不含第三方素材。
SHA256：
`ea00b76081462adcf98f3ed17ded82e7bf5ca6fbb3e4d8dcd26728ff3c5cc62b`。
窗口 smoke 使用 AssetServer 实际加载它，检查普通/位图富文本的自动与显式尺寸、
重排、滤镜、灰度、透明度及销毁；路径相对于 Bevy assets 根目录。

## 外部字体测试资源

`fonts/FiraMono-subset.ttf`（18,848 字节）复制自 Bevy 0.20.1 的
`bevy_text/src/FiraMono-subset.ttf`，用于通过 AssetServer 实际加载普通字体。
SHA256：`45131c5e913a5c7b4311101303e3c320a97b93a27a64b7262b533ee0d31ae03c`。
字体由 Mozilla Foundation 和 Telefonica S.A. 发布，使用 SIL Open Font License 1.1；
随附 `fonts/Fira-OFL.txt` 取自 <https://github.com/mozilla/Fira/blob/master/LICENSE>。
该拉丁子集用于普通/富文本及输入框字体切换 smoke，不包含中文字形。

`fonts/NotoSansSC-VF.ttf` 来自 Google Fonts 的 Noto Sans SC，覆盖 demo 所需的
CJK 字形，使用 SIL Open Font License 1.1；许可证见 `fonts/NotoSansSC-OFL.txt`。
它用作 demo 默认字体，也用于实际文件字体加载与中文字形渲染 smoke。
