use super::*;
use crate::widget::ChildrenRenderOrder;

#[cfg(test)]
#[path = "render_order_tests.rs"]
mod tests;

impl ObjectStore {
    pub fn children_render_order(&self, object: GObjectId) -> Option<ChildrenRenderOrder> {
        Some(
            self.object(object)?
                .control()
                .component()?
                .children_render_order,
        )
    }

    pub fn apex_index(&self, object: GObjectId) -> Option<i32> {
        Some(self.object(object)?.control().component()?.apex_index)
    }

    pub fn set_children_render_order(
        &mut self,
        object: GObjectId,
        order: ChildrenRenderOrder,
    ) -> bool {
        let Some(data) = self
            .objects
            .get_mut(object)
            .and_then(|o| o.control.component_mut())
        else {
            return false;
        };
        if data.children_render_order == order {
            return false;
        }
        data.children_render_order = order;
        self.touch(object, dirty::CHILDREN);
        true
    }

    /// Preserve the requested index; the display builder clamps it using the
    /// current child count. Changing it outside Arch does not reorder painting.
    pub fn set_apex_index(&mut self, object: GObjectId, index: i32) -> bool {
        let Some(data) = self
            .objects
            .get_mut(object)
            .and_then(|o| o.control.component_mut())
        else {
            return false;
        };
        if data.apex_index == index {
            return false;
        }
        data.apex_index = index;
        if data.children_render_order == ChildrenRenderOrder::Arch {
            self.touch(object, dirty::CHILDREN);
        }
        true
    }

    /// Local paint key for a logical child. Pane refresh parts stay below
    /// content; registered scroll bars stay above it regardless of render mode.
    /// Gaps for hidden/group objects are intentional and preserve native apex
    /// indexing. Sorting these keys reproduces the native display order.
    pub fn child_paint_index(&self, child: GObjectId) -> Option<i32> {
        let parent = self.object(child)?.parent()?;
        let object = self.object(parent)?;
        let raw = object.children().iter().position(|&id| id == child)?;
        if self.is_scroll_refresh_overlay(child) {
            return Some(raw as i32 - object.children().len() as i32);
        }
        let count = object
            .children()
            .iter()
            .filter(|&&id| !self.is_scroll_overlay(id))
            .count();
        if self.is_scroll_bar_overlay(child) {
            return Some((count + raw) as i32);
        }
        let index = object.children()[..raw]
            .iter()
            .filter(|&&id| !self.is_scroll_overlay(id))
            .count();
        let Some(data) = object.control().component() else {
            return Some(index as i32);
        };
        let apex = (data.apex_index.max(0) as usize).min(count);
        Some(match data.children_render_order {
            ChildrenRenderOrder::Ascent => index,
            ChildrenRenderOrder::Descent => count - 1 - index,
            ChildrenRenderOrder::Arch if index >= apex => count - 1 - index + apex,
            ChildrenRenderOrder::Arch => index,
        } as i32)
    }
}
