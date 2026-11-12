//! `.fui` 资产的加载。

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::prelude::*;

use fairygui_core::package::FuiPackage;

/// 加载完成的 FairyGUI 包。
#[derive(Asset, TypePath, Debug)]
pub struct FuiAsset {
    /// 解析后的包数据。纯数据，不含运行期状态。
    pub package: FuiPackage,
    /// 资源键，即不含扩展名的资产路径，用于拼出图集与声音的路径。
    pub res_key: String,
}

impl FuiAsset {
    /// 把包内记录的文件名解析成可加载的资产路径。
    ///
    /// 包内记录的图集文件名带包名前缀但不带目录，目录与包文件本身相同。
    pub fn resolve_asset_path(&self, file: &str) -> String {
        match self.res_key.rsplit_once('/') {
            Some((dir, _)) if file.starts_with(&format!("{dir}/")) => file.to_string(),
            Some((dir, _)) => format!("{dir}/{file}"),
            None => file.to_string(),
        }
    }
}

/// `.fui` 文件的加载器。
#[derive(Debug, Default, TypePath)]
pub struct FuiAssetLoader;

/// 加载 `.fui` 时可能出现的错误。
#[derive(Debug, thiserror::Error)]
pub enum FuiLoadError {
    /// 读取文件失败。
    #[error("failed to read package bytes: {0}")]
    Io(#[from] std::io::Error),
    /// 解析包体失败。
    #[error("failed to parse package: {0}")]
    Parse(#[from] fairygui_core::Error),
}

impl AssetLoader for FuiAssetLoader {
    type Asset = FuiAsset;
    type Settings = ();
    type Error = FuiLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        // 资源键取资产路径去掉扩展名，图集与声音的路径都以它为基准。
        let res_key = load_context
            .path()
            .path()
            .to_string_lossy()
            .trim_end_matches(".fui")
            .replace('\\', "/");

        let package = FuiPackage::parse(res_key.clone(), &bytes)?;
        Ok(FuiAsset { package, res_key })
    }

    fn extensions(&self) -> &[&str] {
        &["fui"]
    }
}

#[cfg(test)]
mod tests {
    use super::FuiAsset;

    #[test]
    fn resolve_asset_path_does_not_duplicate_package_directory() {
        let package = fairygui_core::package::FuiPackage::parse(
            "MainMenu",
            include_bytes!("../../fairygui-core/tests/fixtures/MainMenu.fui"),
        )
        .expect("fixture package");
        let asset = FuiAsset {
            package,
            res_key: "ui/MainMenu".to_string(),
        };

        assert_eq!(
            asset.resolve_asset_path("ui/MainMenu_atlas0.png"),
            "ui/MainMenu_atlas0.png"
        );
        assert_eq!(
            asset.resolve_asset_path("MainMenu_atlas0.png"),
            "ui/MainMenu_atlas0.png"
        );
    }
}
