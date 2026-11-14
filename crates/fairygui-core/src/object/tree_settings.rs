use super::{GObjectId, ObjectStore};
use crate::widget::ControlData;

/// Live native GTree settings independent of node expansion and selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeSettings {
    pub indent: i32,
    /// 0 arrows only, 1 click folders to toggle, 2 retains native inert behavior.
    pub click_to_expand: i32,
}

#[cfg(test)]
#[path = "tree_settings_tests.rs"]
mod tests;
impl Default for TreeSettings {
    fn default() -> Self {
        Self {
            indent: 15,
            click_to_expand: 0,
        }
    }
}

impl ObjectStore {
    pub fn tree_settings(&self, tree: GObjectId) -> Option<TreeSettings> {
        let ControlData::Tree(data) = self.object(tree)?.control() else {
            return None;
        };
        Some(TreeSettings {
            indent: data.indent,
            click_to_expand: data.click_to_expand,
        })
    }

    /// Native assignment only. Indent is sampled when a cell is constructed;
    /// retained cells keep their existing indent-part geometry.
    pub fn set_tree_settings(&mut self, tree: GObjectId, settings: TreeSettings) -> bool {
        let Some(ControlData::Tree(data)) = self.objects.get_mut(tree).map(|o| &mut o.control)
        else {
            return false;
        };
        let changed =
            data.indent != settings.indent || data.click_to_expand != settings.click_to_expand;
        if !changed {
            return false;
        }
        data.indent = settings.indent;
        data.click_to_expand = settings.click_to_expand;
        true
    }
}
