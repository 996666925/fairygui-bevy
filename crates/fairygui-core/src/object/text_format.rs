use super::{GObjectId, ObjectStore, dirty};
use crate::widget::{ControlData, TextFieldData};

// The native GTextField setters below only invalidate text layout. Unlike
// color/strokeColor they never capture a controller gear.
macro_rules! text_setters {
    ($($(#[$doc:meta])* $method:ident, $field:ident, $ty:ty;)*) => {
        impl ObjectStore {
            $(
                $(#[$doc])*
                pub fn $method(&mut self, id: GObjectId, value: $ty) -> bool {
                    let Some(target) = self.text_field_of(id) else { return false };
                    let field = match &mut self.objects[target].control {
                        ControlData::Text(data) => data,
                        ControlData::TextInput(data) => &mut data.text_field,
                        _ => return false,
                    };
                    if field.$field == value { return false }
                    field.$field = value;
                    self.touch(target, dirty::TEXT);
                    true
                }
            )*
        }
    }
}

text_setters! {
    /// Native horizontal text alignment: 0 left, 1 center, 2 right.
    set_object_text_align, align, i32;
    /// Native vertical text alignment: 0 top, 1 middle, 2 bottom.
    set_object_text_vertical_align, vertical_align, i32;
    /// Extra logical-pixel spacing added to the font size between lines.
    set_object_text_leading, leading, i32;
    /// Logical-pixel spacing between characters.
    set_object_letter_spacing, letter_spacing, i32;
    /// Native autoSize: 0 fixed, 1 both, 2 height, 3 shrink, 4 ellipsis.
    set_object_text_auto_size, auto_size, i32;
    /// Set field-wide bold, forwarding compound controls to their title field.
    set_object_bold, bold, bool;
    /// Set field-wide italic, forwarding compound controls to their title field.
    set_object_italic, italic, bool;
}

impl ObjectStore {
    /// The text field itself or the recursively resolved title of a button,
    /// label or combo box. Text is committed model data, before presentation.
    pub fn object_text_field(&self, id: GObjectId) -> Option<&TextFieldData> {
        self.object(self.text_field_of(id)?)?
            .control()
            .as_text_field()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::ObjectType;

    #[test]
    fn nested_title_layout_setters_preserve_values_and_reject_unresolved_fields() {
        let mut store = ObjectStore::new();
        let button = store.create(ObjectType::Button);
        let label = store.create(ObjectType::Label);
        let input = store.create(ObjectType::InputText);
        for (child, parent) in [(label, button), (input, label)] {
            store.set_name(child, "title");
            store.set_parent(child, Some(parent));
        }
        store.set_object_text(input, "committed value");
        store.set_input_password(input, true);
        store.take_dirty();
        assert!(store.set_object_text_auto_size(button, 2));
        assert!(store.set_object_text_vertical_align(button, 1));
        assert!(store.set_object_bold(button, true));
        assert!(store.set_object_italic(button, true));
        let field = store.object_text_field(button).unwrap();
        assert_eq!(field, store.object_text_field(input).unwrap());
        assert_eq!(field.text, "committed value");
        assert_eq!((field.auto_size, field.vertical_align), (2, 1));
        assert!(field.bold && field.italic);
        assert_eq!(store.take_dirty().len(), 1);
        assert!(!store.set_object_bold(button, true));
        assert!(store.take_dirty().is_empty());
        let image = store.create(ObjectType::Image);
        assert_eq!(store.object_text_field(image), None);
        assert!(!store.set_object_text_align(image, 1));
        store.remove(label);
        assert!(!store.set_object_letter_spacing(button, 3));
        assert_eq!(store.object_text_field(button), None);
    }
}
