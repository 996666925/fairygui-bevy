//! FairyGUI 运行时核心，平台无关。
//!
//! 本 crate 不依赖任何渲染引擎，负责三件事：
//!
//! 1. **解析**：读取 FairyGUI 编辑器导出的 `.fui` 二进制包，产出纯数据的包描述符。
//! 2. **对象模型**：把包描述实例化为可操作的对象树（`GObject` 及其全部控件）。
//! 3. **行为**：控制器、齿轮、关联、过渡与补间等驱动界面变化的逻辑。
//!
//! 渲染与输入由上层集成 crate 负责，例如 `fairygui-bevy`。
//!
//! 移植蓝本为官方 [FairyGUI-layabox](https://github.com/fairygui/FairyGUI-layabox)，
//! 包格式与运行期语义均以其为准。

pub mod action;
pub mod async_build;
pub mod binary;
pub mod bitmap_text;
pub mod color;
pub mod color_matrix;
pub mod drag_drop;
pub mod error;
pub mod fill;
pub mod geom;
pub mod graph;
pub mod object;
pub mod package;
pub mod pool;
pub mod popup;
pub mod root;
pub mod scroll;
pub mod text;
pub mod tile_grid;
pub mod transition;
pub mod translation;
pub mod widget;
pub mod window;

pub use binary::{ByteBuffer, StringTable};
pub use error::{Error, Result};
pub use geom::{Margin, Point, Rect, Size};
pub use object::{GObject, GObjectId, ObjectStore};
pub use package::FuiPackage;
pub use pool::ObjectPool;
