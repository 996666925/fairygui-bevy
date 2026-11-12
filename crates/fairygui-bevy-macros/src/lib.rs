//! `fairygui-bevy` 的派生宏。
//!
//! 目前尚未导出任何宏：对象模型的扩展注册机制（对应官方运行时的
//! `UIObjectFactory.resolvePackageItemExtension`）落地后，本 crate 将提供把普通
//! Rust 类型注册为 FairyGUI 扩展组件的派生宏，使编辑器里设置了扩展号的条目能够
//! 实例化为用户自定义类型。
//!
//! 这个 crate 在此之前保持为空，以避免提供无用或被后续设计推翻的 API。
