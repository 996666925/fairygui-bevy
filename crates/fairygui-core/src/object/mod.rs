//! 对象模型。
//!
//! 界面在运行期是一棵对象树：容器持有子对象，子对象反向记录父对象。
//! 为避免引用计数带来的循环引用与借用冲突，这里用竞技场（[`ObjectStore`]）
//! 承载全部对象，树关系只保存句柄 [`GObjectId`]。
//!
//! 对象数据的文件名与所在模块同名，这是刻意的：它描述的正是「对象」本身。

#![allow(clippy::module_inception)]

mod event;
mod object;
mod store;

pub use event::{EventData, EventKind, EventQueue, UiEvent};
pub use object::{BlendMode, GObject, GObjectId};
pub use store::{
    AnimationCommand, AnimationPlaySettings, CoordinateTransform, DisplayLockToken, GearTween,
    ListInteractionSettings, ListLayoutSettings, ObjectStore, VirtualItemFactory,
    VirtualItemProvider, VirtualItemRenderer, VirtualListError, VirtualListInfo, dirty,
};
pub(crate) use store::{NestedHandle, TransitionId};
