use fairygui_core::{
    ByteBuffer, Error, Result,
    package::{FuiPackage, ObjectType, PackageItem},
};
use std::collections::BTreeSet;

#[derive(Debug)]
pub(crate) struct Child {
    pub name: String,
    pub kind: ObjectType,
    pub source: Option<String>,
    pub package: Option<String>,
}
#[derive(Debug)]
pub(crate) struct Controller {
    pub name: String,
    pub pages: Vec<(String, String)>,
}
#[derive(Debug, Default)]
pub(crate) struct Component {
    pub children: Vec<Child>,
    pub controllers: Vec<Controller>,
    pub transitions: Vec<String>,
}

fn record<'a>(buffer: &mut ByteBuffer<'a>) -> Result<ByteBuffer<'a>> {
    let position = buffer.position();
    let length = buffer.read_i16()?;
    let length = usize::try_from(length)
        .map_err(|_| Error::invalid_data(position, "negative metadata record length"))?;
    let bytes = buffer.read_bytes(length)?;
    let mut record = ByteBuffer::new(bytes);
    record.set_version(buffer.version());
    Ok(record)
}
fn count(buffer: &mut ByteBuffer) -> Result<usize> {
    let position = buffer.position();
    usize::try_from(buffer.read_i16()?)
        .map_err(|_| Error::invalid_data(position, "negative metadata count"))
}
fn unique(names: &mut BTreeSet<String>, name: &str, label: &str) -> Result<()> {
    if !names.insert(name.into()) {
        return Err(Error::invalid_data(
            0,
            format!("duplicate {label} name {name:?}; named bindings would be ambiguous"),
        ));
    }
    Ok(())
}
pub(crate) fn parse(package: &FuiPackage, item: &PackageItem) -> Result<Component> {
    let mut buffer = package
        .raw_data(item)
        .ok_or_else(|| Error::invalid_data(0, "component has no data"))?;
    let table = package.string_table();
    let mut component = Component::default();
    let mut names = BTreeSet::new();
    if buffer.seek(0, 2)? {
        for _ in 0..count(&mut buffer)? {
            let mut child = record(&mut buffer)?;
            if !child.seek(0, 0)? {
                return Err(Error::MissingBlock { block: 0 });
            }
            let kind = ObjectType::from_code(child.read_u8()?);
            let source = child.read_s(table)?;
            let package = child.read_s(table)?;
            child.read_s(table)?; // instance ID
            let name = child.read_s(table)?.unwrap_or_default();
            unique(&mut names, &name, "child")?;
            component.children.push(Child {
                name,
                kind,
                source,
                package,
            });
        }
    } else {
        return Err(Error::MissingBlock { block: 2 });
    }
    names.clear();
    if buffer.seek(0, 1)? {
        for _ in 0..count(&mut buffer)? {
            let mut controller = record(&mut buffer)?;
            if !controller.seek(0, 0)? {
                return Err(Error::MissingBlock { block: 0 });
            }
            let name = controller.read_s(table)?.unwrap_or_default();
            unique(&mut names, &name, "controller")?;
            let mut pages = Vec::new();
            if controller.seek(0, 1)? {
                let mut ids = BTreeSet::new();
                for _ in 0..count(&mut controller)? {
                    let id = controller.read_s(table)?.unwrap_or_default();
                    let page = controller.read_s(table)?.unwrap_or_default();
                    unique(&mut ids, &id, "page ID")?;
                    pages.push((id, page));
                }
            }
            component.controllers.push(Controller { name, pages });
        }
    }
    names.clear();
    if buffer.seek(0, 5)? {
        for _ in 0..count(&mut buffer)? {
            let mut transition = record(&mut buffer)?;
            let name = transition.read_s(table)?.unwrap_or_default();
            unique(&mut names, &name, "transition")?;
            component.transitions.push(name);
        }
    }
    Ok(component)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_record_lengths_cannot_read_into_the_next_exported_record() {
        for bytes in [&[255, 255][..], &[0, 5, 0][..]] {
            assert!(record(&mut ByteBuffer::new(bytes)).is_err());
        }
        let mut outer = ByteBuffer::new(&[0, 1, 7, 8, 9]);
        let mut inner = record(&mut outer).unwrap();
        assert_eq!(inner.read_u8().unwrap(), 7);
        assert!(inner.read_u8().is_err());
        assert_eq!(outer.read_u8().unwrap(), 8);
    }
    #[test]
    fn ambiguous_original_names_fail_instead_of_binding_the_first_match() {
        for label in ["child", "controller", "transition", "page ID"] {
            let mut names = BTreeSet::new();
            unique(&mut names, "same", label).unwrap();
            let error = unique(&mut names, "same", label).unwrap_err();
            assert!(error.to_string().contains(label));
        }
    }
}
