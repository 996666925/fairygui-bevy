//! 分帧构建。
//!
//! 一次性建起多个界面时，逐帧摊开可以避免启动瞬间的卡顿。粒度是**条目**：
//! 每次推进构建若干条，而不是把单个条目的内部拆开——后者的收益有限，
//! 却要把递归构建改造成显式状态机，得不偿失。
//!
//! 每帧的预算按「条目数」计。单个条目特别大时仍会占用较多时间，
//! 因此预算应当配合实际的界面规模来调。

use std::collections::VecDeque;

use tracing::warn;

use crate::error::{Error, Result};
use crate::object::{GObjectId, ObjectStore};
use crate::package::{FuiPackage, LoadContext, PackageRegistry};
use crate::widget::{create_from_package, create_from_package_with_registry};

/// 分帧构建器。
#[derive(Debug)]
pub struct AsyncBuilder {
    /// 尚未构建的条目。
    queue: VecDeque<usize>,
    /// 每帧最多构建多少个条目。
    budget: usize,
    /// 已构建成功的根对象。
    roots: Vec<GObjectId>,
    /// 构建失败的条目数。
    failures: usize,
}

impl AsyncBuilder {
    /// 创建构建任务。
    ///
    /// `item_indices` 是要构建的条目，`budget` 是每帧最多构建的条目数。
    pub fn new(item_indices: impl IntoIterator<Item = usize>, budget: usize) -> Self {
        Self {
            queue: item_indices.into_iter().collect(),
            budget: budget.max(1),
            roots: Vec::new(),
            failures: 0,
        }
    }

    /// 待构建的条目数量。
    pub fn remaining(&self) -> usize {
        self.queue.len()
    }

    /// 已构建完成的根对象。
    pub fn roots(&self) -> &[GObjectId] {
        &self.roots
    }

    /// 构建失败的条目数量。
    pub fn failures(&self) -> usize {
        self.failures
    }

    /// 是否已经全部处理完。
    pub fn is_finished(&self) -> bool {
        self.queue.is_empty()
    }

    /// 推进一帧。返回是否已经全部完成。
    ///
    /// 单个条目构建失败不会中断整体流程，只会被计入失败数并告警——
    /// 让一个坏条目挡住其余界面的加载是更糟的选择。
    pub fn advance(
        &mut self,
        store: &mut ObjectStore,
        package: &FuiPackage,
        ctx: &LoadContext,
    ) -> bool {
        self.advance_using(store, package, ctx, None)
    }

    /// Advance with dependency resolution, using the same per-entry budget.
    pub fn advance_with_registry(
        &mut self,
        store: &mut ObjectStore,
        package: &FuiPackage,
        ctx: &LoadContext,
        registry: &PackageRegistry,
    ) -> bool {
        self.advance_using(store, package, ctx, Some(registry))
    }

    fn advance_using(
        &mut self,
        store: &mut ObjectStore,
        package: &FuiPackage,
        ctx: &LoadContext,
        registry: Option<&PackageRegistry>,
    ) -> bool {
        let mut processed = 0;

        while processed < self.budget {
            let Some(item_index) = self.queue.pop_front() else {
                return true;
            };
            processed += 1;

            let result = match registry {
                Some(registry) => {
                    create_from_package_with_registry(store, package, item_index, ctx, registry)
                }
                None => create_from_package(store, package, item_index, ctx),
            };
            match result {
                Ok(root) => self.roots.push(root),
                Err(err) => {
                    self.failures += 1;
                    warn!(
                        item_index,
                        error = %err,
                        "分帧构建时该条目失败，已跳过"
                    );
                }
            }
        }

        self.queue.is_empty()
    }

    /// 一次性构建完，直到队列为空。
    ///
    /// 供不关心分帧的场景使用（例如测试或资源很少的启动路径）。
    pub fn finish(
        &mut self,
        store: &mut ObjectStore,
        package: &FuiPackage,
        ctx: &LoadContext,
    ) -> &[GObjectId] {
        while !self.advance(store, package, ctx) {}
        &self.roots
    }

    /// 按名称收集要构建的条目。
    ///
    /// 名称不存在时返回错误，这样拼错界面名能立刻发现，而不是静默少建一个界面。
    pub fn from_names(package: &FuiPackage, names: &[&str], budget: usize) -> Result<Self> {
        let mut indices = Vec::with_capacity(names.len());
        for name in names {
            let index = package
                .item_index_by_name(name)
                .ok_or_else(|| Error::ItemNotFound((*name).to_string()))?;
            indices.push(index);
        }
        Ok(Self::new(indices, budget))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    const MAIN_MENU: &[u8] = include_bytes!("../tests/fixtures/MainMenu.fui");

    fn package() -> FuiPackage {
        FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package")
    }

    fn main_index(package: &FuiPackage) -> usize {
        package
            .item_index_by_name("Main")
            .expect("Main should exist")
    }

    #[test]
    fn budget_limits_work_per_step() {
        let package = package();
        let index = main_index(&package);
        let mut store = ObjectStore::new();
        let ctx = LoadContext::none();

        // 两个条目、每次只做一个。
        let mut builder = AsyncBuilder::new([index, index], 1);
        assert_eq!(builder.remaining(), 2);

        assert!(!builder.advance(&mut store, &package, &ctx), "还有剩余");
        assert_eq!(builder.roots().len(), 1);
        assert_eq!(builder.remaining(), 1);

        assert!(builder.advance(&mut store, &package, &ctx), "应当全部完成");
        assert_eq!(builder.roots().len(), 2);
        assert!(builder.is_finished());
    }

    #[test]
    fn budget_of_zero_is_treated_as_one() {
        let builder = AsyncBuilder::new([0, 1], 0);
        assert_eq!(builder.remaining(), 2);

        // 预算为 0 会让推进永远不动，这里应当被纠正为至少 1。
        let package = package();
        let mut store = ObjectStore::new();
        let mut builder = builder;
        builder.advance(&mut store, &package, &LoadContext::none());
        assert_eq!(builder.remaining(), 1);
    }

    #[test]
    fn invalid_entry_is_counted_as_failure_without_stopping() {
        let package = package();
        let index = main_index(&package);
        let mut store = ObjectStore::new();
        let ctx = LoadContext::none();

        // 中间夹一个越界索引。
        let mut builder = AsyncBuilder::new([index, 9999, index], 1);
        builder.finish(&mut store, &package, &ctx);

        assert_eq!(builder.roots().len(), 2, "其余条目仍应构建成功");
        assert_eq!(builder.failures(), 1);
    }

    #[test]
    fn from_names_rejects_unknown_names() {
        let package = package();
        assert!(AsyncBuilder::from_names(&package, &["Main"], 4).is_ok());

        let err = AsyncBuilder::from_names(&package, &["Main", "NotExist"], 4).unwrap_err();
        assert!(matches!(err, Error::ItemNotFound(_)), "实际 {err:?}");
    }

    #[test]
    fn finish_builds_everything() {
        let package = package();
        let index = main_index(&package);
        let mut store = ObjectStore::new();

        let mut builder = AsyncBuilder::new([index, index, index], 2);
        let roots = builder.finish(&mut store, &package, &LoadContext::none());

        assert_eq!(roots.len(), 3);
        assert!(builder.is_finished());
        assert!(store.len() > 3, "每个界面都应当展开成一棵对象树");
    }

    #[test]
    fn empty_queue_finishes_immediately() {
        let package = package();
        let mut store = ObjectStore::new();

        let mut builder = AsyncBuilder::new([], 4);
        assert!(builder.is_finished());
        assert!(builder.advance(&mut store, &package, &LoadContext::none()));
        assert!(builder.roots().is_empty());
    }

    #[test]
    fn built_roots_are_usable() {
        let package = package();
        let index = main_index(&package);
        let mut store = ObjectStore::new();

        let mut builder = AsyncBuilder::new([index], 1);
        builder.finish(&mut store, &package, &LoadContext::none());

        let root = builder.roots()[0];
        let object = store.object(root).expect("root object");
        assert_eq!(object.kind(), ObjectType::Component);
        assert!(!object.children().is_empty(), "界面应当已经展开");
    }
}
