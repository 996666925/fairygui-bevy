//! Native selection-controller references shared by lists, trees and combos.
use super::{GObjectId, ObjectStore};
use crate::widget::ControlData;

impl ObjectStore {
    pub(super) fn selection_controller_ref(&self, object: GObjectId) -> Option<(GObjectId, usize)> {
        let data = self.object(object)?;
        let index = match data.control() {
            ControlData::List(list) => list.selection_controller,
            ControlData::Tree(tree) => tree.list.selection_controller,
            ControlData::ComboBox(combo) => combo.selection_controller,
            _ => return None,
        };
        // A combo temporarily writes -1 while updating its controller, matching
        // native's temporary null reference and preventing source reentry.
        let index = usize::try_from(index).ok()?;
        if let Some(binding) = self.selection_controllers.get(&object) {
            return Some(*binding);
        }
        Some((data.parent()?, index))
    }

    /// Resolve exported parent indices before detachment. A captured reference
    /// remains attached to the original owner, including an expired owner ID.
    pub(super) fn preserve_selection_controller(&mut self, object: GObjectId) {
        if self.selection_controllers.contains_key(&object) {
            return;
        }
        if let Some((owner, index)) = self.selection_controller_ref(object)
            && self
                .object(owner)
                .and_then(|o| o.control().component())
                .and_then(|c| c.controllers.get(index))
                .is_some()
        {
            self.selection_controllers.insert(object, (owner, index));
        }
    }
}
