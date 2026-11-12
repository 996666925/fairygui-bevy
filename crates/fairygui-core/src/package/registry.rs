//! 运行时包注册表。
//!
//! 官方把包登记为「编号 / 资源键 / 自定义编号」三套键，外加一套名称键；
//! `ui://` 协议的地址正是靠它解析到具体条目的。分支切换时官方会立刻
//! 重算所有包的分支序号，这里改为按需计算——分支序号由包自己的分支列表
//! 推导即可得到，不必缓存。

use std::collections::HashMap;
use std::sync::Arc;

use crate::package::{FuiPackage, LoadContext};

/// 包注册表。
#[derive(Debug, Default)]
pub struct PackageRegistry {
    /// 编号、资源键与自定义编号共用的索引。
    by_key: HashMap<String, Arc<FuiPackage>>,
    /// 包名索引。
    by_name: HashMap<String, Arc<FuiPackage>>,
    /// 当前分支名。空串表示未启用分支。
    branch: String,
    /// 全局变量表，供控制器首屏页面等场景读取。
    vars: HashMap<String, String>,
    /// 内容缩放等级，决定是否取用高清替代资源。
    content_scale_level: i32,
}

impl PackageRegistry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 已登记的包数量。
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// 是否没有登记任何包。
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// 登记一个包。
    ///
    /// 同一编号重复登记时保留先登记的那个，与官方行为一致。
    pub fn add(&mut self, package: Arc<FuiPackage>, res_key: impl Into<String>) -> Arc<FuiPackage> {
        let id = package.id().to_string();

        if let Some(existing) = self.by_key.get(&id) {
            return existing.clone();
        }

        self.by_key.insert(id, package.clone());
        self.by_key.insert(res_key.into(), package.clone());
        self.by_name
            .insert(package.name().to_string(), package.clone());

        package
    }

    /// 为包补充一个自定义编号，之后可以用它检索。
    pub fn set_custom_id(&mut self, package: &Arc<FuiPackage>, custom_id: impl Into<String>) {
        self.by_key.insert(custom_id.into(), package.clone());
    }

    /// 注销一个包（接受编号、资源键、自定义编号或包名）。
    pub fn remove(&mut self, id_or_name: &str) -> Option<Arc<FuiPackage>> {
        let package = self
            .by_key
            .get(id_or_name)
            .or_else(|| self.by_name.get(id_or_name))?
            .clone();

        self.by_key.retain(|_, value| !Arc::ptr_eq(value, &package));
        self.by_name
            .retain(|_, value| !Arc::ptr_eq(value, &package));

        Some(package)
    }

    /// 清空全部登记。
    pub fn clear(&mut self) {
        self.by_key.clear();
        self.by_name.clear();
    }

    /// 按编号、资源键或自定义编号取包。
    pub fn get_by_id(&self, id: &str) -> Option<&Arc<FuiPackage>> {
        self.by_key.get(id)
    }

    /// 按名称取包。
    pub fn get_by_name(&self, name: &str) -> Option<&Arc<FuiPackage>> {
        self.by_name.get(name)
    }

    /// 遍历已登记的包（按名称去重，每个包只出现一次）。
    pub fn packages(&self) -> impl Iterator<Item = &Arc<FuiPackage>> {
        self.by_name.values()
    }

    // ---- 分支与变量 ----

    /// 当前分支名。
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// 切换分支。分支序号由包自己的分支列表推导，无需额外维护。
    pub fn set_branch(&mut self, branch: impl Into<String>) {
        self.branch = branch.into();
    }

    /// 当前内容缩放等级。
    pub fn content_scale_level(&self) -> i32 {
        self.content_scale_level
    }

    /// 设置内容缩放等级，影响高清替代资源的取用。
    pub fn set_content_scale_level(&mut self, level: i32) {
        self.content_scale_level = level.max(0);
    }

    /// 读取全局变量。
    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// 设置全局变量。
    pub fn set_var(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// 为某个包生成加载上下文。
    ///
    /// 分支序号直接由分支名在包的分支列表中的位置得出。
    pub fn load_context(&self, package: &FuiPackage) -> LoadContext {
        LoadContext {
            branch_index: package.branch_index_of(&self.branch),
            content_scale_level: self.content_scale_level,
        }
    }

    /// 生成与当前分支、缩放等级一致的加载上下文。
    pub fn context_for(&self, package: &Arc<FuiPackage>) -> LoadContext {
        self.load_context(package)
    }

    // ---- 地址解析 ----

    /// 解析 `ui://` 地址，返回包与条目下标。
    ///
    /// 支持两种形式：`ui://<包编号><条目编号>`（编号固定 8 字符）与
    /// `ui://<包名>/<条目名>`。
    pub fn resolve_url(&self, url: &str) -> Option<(&Arc<FuiPackage>, usize)> {
        url.strip_prefix("ui://")?;
        let rest_start = 5;

        match url[rest_start..].find('/') {
            Some(offset) => {
                let slash = rest_start + offset;
                let package = self.get_by_name(&url[rest_start..slash])?;
                let index = package.item_index_by_name(&url[slash + 1..])?;
                Some((package, index))
            }
            None => {
                // 编号形式：前 8 个字符是包编号，其余是条目编号。
                if url.len() <= rest_start + 8 {
                    return None;
                }
                let package = self.get_by_id(url.get(rest_start..rest_start + 8)?)?;
                let index = package.item_index_by_id(&url[rest_start + 8..])?;
                Some((package, index))
            }
        }
    }

    /// 生成 `ui://` 地址（编号形式）。
    pub fn item_url(&self, package_name: &str, item_name: &str) -> Option<String> {
        let package = self.get_by_name(package_name)?;
        let index = package.item_index_by_name(item_name)?;
        let item = package.item(index)?;
        Some(format!("ui://{}{}", package.id(), item.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_MENU: &[u8] = include_bytes!("../../tests/fixtures/MainMenu.fui");

    fn registry_with_menu() -> (PackageRegistry, Arc<FuiPackage>) {
        let package = Arc::new(FuiPackage::parse("MainMenu", MAIN_MENU).expect("parse package"));
        let mut registry = PackageRegistry::new();
        registry.add(package.clone(), "ui/MainMenu");
        (registry, package)
    }

    #[test]
    fn registers_under_several_keys() {
        let (registry, package) = registry_with_menu();

        assert_eq!(registry.len(), 1);
        assert!(Arc::ptr_eq(
            registry.get_by_id(package.id()).unwrap(),
            &package
        ));
        assert!(Arc::ptr_eq(
            registry.get_by_id("ui/MainMenu").unwrap(),
            &package
        ));
        assert!(Arc::ptr_eq(
            registry.get_by_name("MainMenu").unwrap(),
            &package
        ));
        assert!(registry.get_by_id("missing").is_none());
    }

    #[test]
    fn custom_id_adds_another_key() {
        let (mut registry, package) = registry_with_menu();
        registry.set_custom_id(&package, "menu-alias");
        assert!(Arc::ptr_eq(
            registry.get_by_id("menu-alias").unwrap(),
            &package
        ));
    }

    #[test]
    fn duplicate_registration_keeps_the_first() {
        let (mut registry, package) = registry_with_menu();
        let again = registry.add(package.clone(), "another/key");

        assert!(Arc::ptr_eq(&again, &package));
        assert_eq!(registry.len(), 1);
        assert!(
            registry.get_by_id("another/key").is_none(),
            "重复登记不应再增加键"
        );
    }

    #[test]
    fn removal_drops_all_keys() {
        let (mut registry, package) = registry_with_menu();
        assert!(registry.remove("MainMenu").is_some());

        assert!(registry.is_empty());
        assert!(registry.get_by_id(package.id()).is_none());
        assert!(registry.get_by_name("MainMenu").is_none());
        assert!(registry.remove("MainMenu").is_none());
    }

    #[test]
    fn resolves_by_name_and_by_id() {
        let (registry, package) = registry_with_menu();

        // 名称形式
        let (found, index) = registry.resolve_url("ui://MainMenu/Main").expect("resolve");
        assert!(Arc::ptr_eq(found, &package));
        assert_eq!(package.item(index).unwrap().name, "Main");

        // 编号形式
        let item = package.item_by_name("Main").unwrap();
        let url = format!("ui://{}{}", package.id(), item.id);
        let (found, index) = registry.resolve_url(&url).expect("resolve");
        assert!(Arc::ptr_eq(found, &package));
        assert_eq!(package.item(index).unwrap().name, "Main");
    }

    #[test]
    fn unknown_urls_resolve_to_nothing() {
        let (registry, _) = registry_with_menu();

        assert!(registry.resolve_url("ui://MainMenu/Missing").is_none());
        assert!(registry.resolve_url("ui://NoSuchPackage/Main").is_none());
        assert!(registry.resolve_url("ui://short").is_none());
        assert!(registry.resolve_url("not-a-url").is_none());
    }

    #[test]
    fn item_url_round_trips() {
        let (registry, _) = registry_with_menu();
        let url = registry.item_url("MainMenu", "Main").expect("build url");
        assert!(url.starts_with("ui://"));
        assert!(registry.resolve_url(&url).is_some());
        assert!(registry.item_url("MainMenu", "Missing").is_none());
    }

    #[test]
    fn branch_and_scale_feed_into_load_context() {
        let (mut registry, package) = registry_with_menu();

        assert_eq!(registry.load_context(&package).branch_index, -1);
        assert_eq!(registry.load_context(&package).content_scale_level, 0);

        registry.set_branch("hd");
        registry.set_content_scale_level(2);
        let ctx = registry.load_context(&package);

        // 该包没有分支，序号应当是「找不到」。
        assert_eq!(ctx.branch_index, -1);
        assert_eq!(ctx.content_scale_level, 2);
    }

    #[test]
    fn global_vars_are_stored() {
        let (mut registry, _) = registry_with_menu();
        assert!(registry.get_var("lang").is_none());

        registry.set_var("lang", "zh");
        assert_eq!(registry.get_var("lang"), Some("zh"));
    }
}
