//! Generate checked Bevy ECS bindings from exported `.fui` binaries in build.rs.
//!
//! ```no_run
//! fairygui_build::Bindings::new()
//!     .package("assets/ui/Basics.fui")
//!     .generate_to_out_dir("fairygui.rs")?;
//! # Ok::<(), fairygui_build::Error>(())
//! ```
//!
//! Include the result with `include!(concat!(env!("OUT_DIR"), "/fairygui.rs"))`.
//! Generated code depends on `bevy` and `fairygui-bevy`; this build dependency
//! itself has no Bevy dependency and reads metadata without constructing UI trees.
mod metadata;
mod names;

use std::{
    collections::BTreeMap,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use fairygui_core::package::{FuiPackage, ObjectType, PackageItemType};
use metadata::Component;
use names::{Names, pascal, snake};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Package {
        path: PathBuf,
        #[source]
        source: fairygui_core::Error,
    },
    #[error("{0}")]
    Schema(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Add all packages referenced by component children, including dependency
/// packages. Output order and collision suffixes are deterministic.
#[derive(Debug, Default, Clone)]
pub struct Bindings {
    paths: Vec<PathBuf>,
}
impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn package(mut self, path: impl Into<PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }
    /// Generate source without Cargo output or filesystem mutations.
    pub fn generate(&self) -> Result<String> {
        let mut packages = BTreeMap::new();
        for path in &self.paths {
            let bytes = fs::read(path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
            let package =
                FuiPackage::parse(path.to_string_lossy().as_ref(), &bytes).map_err(|source| {
                    Error::Package {
                        path: path.clone(),
                        source,
                    }
                })?;
            let id = package.id().to_owned();
            if packages.insert(id.clone(), package).is_some() {
                return Err(Error::Schema(format!("duplicate package ID {id:?}")));
            }
        }
        if packages.is_empty() {
            return Err(Error::Schema("no .fui packages configured".into()));
        }
        generate(&packages)
    }
    /// Write only when contents changed; tell Cargo to rerun for each input.
    pub fn generate_to(&self, output: impl AsRef<Path>) -> Result<()> {
        let source = self.generate()?;
        let output = output.as_ref();
        for path in &self.paths {
            println!("cargo:rerun-if-changed={}", path.display());
        }
        if fs::read_to_string(output).ok().as_deref() != Some(&source) {
            if let Some(parent) = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).map_err(|source| Error::Io {
                    path: parent.into(),
                    source,
                })?;
            }
            fs::write(output, source).map_err(|source| Error::Io {
                path: output.into(),
                source,
            })?;
        }
        Ok(())
    }
    /// Write a single file below Cargo's OUT_DIR. Generated files are not source
    /// artifacts and should not be checked into version control.
    pub fn generate_to_out_dir(&self, filename: &str) -> Result<PathBuf> {
        if Path::new(filename).components().count() != 1
            || !filename.ends_with(".rs")
            || !matches!(
                Path::new(filename).components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(Error::Schema(
                "output must be a Rust filename, for example fairygui.rs".into(),
            ));
        }
        let out = std::env::var_os("OUT_DIR").ok_or_else(|| {
            Error::Schema("OUT_DIR is not set; call from build.rs or use generate_to".into())
        })?;
        let path = PathBuf::from(out).join(filename);
        self.generate_to(&path)?;
        Ok(path)
    }
}

fn kind(kind: ObjectType) -> String {
    match kind {
        ObjectType::Extension(code) => format!("Extension::<{code}>"),
        other => other.as_str().into(),
    }
}
fn kind_expr(kind: ObjectType) -> String {
    match kind {
        ObjectType::Extension(code) => format!("Extension({code})"),
        other => other.as_str().into(),
    }
}

fn generate(packages: &BTreeMap<String, FuiPackage>) -> Result<String> {
    let mut modules = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut module_names = Names::default();
    for (id, package) in packages {
        modules.insert(id.clone(), module_names.take(&snake(package.name())));
        let mut type_names = Names::default();
        for item in package
            .items()
            .iter()
            .filter(|item| item.item_type == PackageItemType::Component)
        {
            types.insert(
                (id.clone(), item.id.clone()),
                type_names.take(&pascal(&item.name)),
            );
        }
    }
    let mut output = String::from("// Generated by fairygui-build. Do not edit.\n");
    for (id, package) in packages {
        let module = &modules[id];
        writeln!(
            output,
            "#[allow(dead_code, non_camel_case_types)]\npub mod {module} {{"
        )
        .unwrap();
        writeln!(
            output,
            "pub const PACKAGE_ID: &str = {:?};\npub const PACKAGE_NAME: &str = {:?};",
            id,
            package.name()
        )
        .unwrap();
        // Reserve every component name before allocating controller enum names.
        let mut extra_types = Names::default();
        for ((pkg, _), name) in &types {
            if pkg == id {
                extra_types.reserve(name);
            }
        }
        for item in package
            .items()
            .iter()
            .filter(|item| item.item_type == PackageItemType::Component)
        {
            let component = metadata::parse(package, item).map_err(|source| {
                Error::Schema(format!("{}/{}: {source}", package.name(), item.name))
            })?;
            let name = &types[&(id.clone(), item.id.clone())];
            emit_component(
                &mut output,
                package,
                item,
                name,
                &component,
                packages,
                &modules,
                &types,
                &mut extra_types,
            )?;
        }
        writeln!(output, "}}").unwrap();
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn emit_component(
    out: &mut String,
    package: &FuiPackage,
    item: &fairygui_core::package::PackageItem,
    name: &str,
    component: &Component,
    packages: &BTreeMap<String, FuiPackage>,
    modules: &BTreeMap<String, String>,
    types: &BTreeMap<(String, String), String>,
    extra_types: &mut Names,
) -> Result<()> {
    let mut fields = String::new();
    let mut values = String::new();
    let mut field_names = Names::default();
    field_names.reserve("root");
    for child in &component.children {
        let field = field_names.take(&snake(&child.name));
        let target = if let Some(source) = &child.source {
            let package_id = child.package.as_deref().unwrap_or(package.id());
            let owner = packages.get(package_id).ok_or_else(|| {
                Error::Schema(format!(
                    "{}/{} child {:?}: add dependency package {package_id:?} with .package(...)",
                    package.name(),
                    item.name,
                    child.name
                ))
            })?;
            let source = owner.item_by_id(source).ok_or_else(|| {
                Error::Schema(format!(
                    "{}/{} child {:?}: source item {source:?} is missing in {}",
                    package.name(),
                    item.name,
                    child.name,
                    owner.name()
                ))
            })?;
            if source.item_type == PackageItemType::Component {
                format!(
                    "super::{}::{}",
                    modules[package_id],
                    types[&(package_id.into(), source.id.clone())]
                )
            } else {
                format!(
                    "::fairygui_bevy::binding::kind::{}",
                    kind(source.object_type)
                )
            }
        } else {
            format!("::fairygui_bevy::binding::kind::{}", kind(child.kind))
        };
        writeln!(fields, "pub {field}: ::fairygui_bevy::GuiHandle<{target}>,").unwrap();
        writeln!(
            values,
            "{field}: ::fairygui_bevy::GuiHandle::child(gui, root.entity(), {:?})?,",
            child.name
        )
        .unwrap();
    }
    for controller in &component.controllers {
        let field = field_names.take(&format!("c_{}", snake(&controller.name)));
        let page_type = extra_types.take(&format!("{name}{}Page", pascal(&controller.name)));
        let mut variant_names = Names::default();
        let variants: Vec<_> = controller
            .pages
            .iter()
            .enumerate()
            .map(|(i, (_, name))| {
                variant_names.take(&if name.is_empty() {
                    format!("Page{i}")
                } else {
                    pascal(name)
                })
            })
            .collect();
        writeln!(
            out,
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {page_type} {{ {} }}",
            variants.join(",")
        )
        .unwrap();
        writeln!(out, "impl ::fairygui_bevy::GuiPage for {page_type} {{\nconst PAGES: &'static [(&'static str, &'static str)] = &{:?};\nfn id(self) -> &'static str {{ match self {{", controller.pages).unwrap();
        for (variant, (id, _)) in variants.iter().zip(&controller.pages) {
            writeln!(out, "Self::{variant} => {id:?},").unwrap();
        }
        writeln!(
            out,
            "}} }}\nfn from_id(id: &str) -> ::core::option::Option<Self> {{ match id {{"
        )
        .unwrap();
        for (variant, (id, _)) in variants.iter().zip(&controller.pages) {
            writeln!(
                out,
                "{id:?} => ::core::option::Option::Some(Self::{variant}),"
            )
            .unwrap();
        }
        writeln!(out, "_ => ::core::option::Option::None, }} }} }}").unwrap();
        writeln!(
            fields,
            "pub {field}: ::fairygui_bevy::GuiController<{page_type}>,"
        )
        .unwrap();
        writeln!(
            values,
            "{field}: ::fairygui_bevy::GuiController::new(gui, root.entity(), {:?})?,",
            controller.name
        )
        .unwrap();
    }
    for transition in &component.transitions {
        let field = field_names.take(&format!("t_{}", snake(transition)));
        writeln!(fields, "pub {field}: ::fairygui_bevy::GuiTransition,").unwrap();
        writeln!(
            values,
            "{field}: ::fairygui_bevy::GuiTransition::new(gui, root.entity(), {transition:?})?,"
        )
        .unwrap();
    }
    let url = format!("ui://{}{}", package.id(), item.id);
    writeln!(out, "/// Exported component {:?} from package {:?}.\n#[derive(::bevy::prelude::Component, Debug, Clone)]\npub struct {name} {{ pub root: ::fairygui_bevy::GuiHandle<Self>,\n{fields}}}", item.name, package.name()).unwrap();
    writeln!(out, "impl ::fairygui_bevy::GuiType for {name} {{ const KIND: ::fairygui_bevy::core::package::ObjectType = ::fairygui_bevy::core::package::ObjectType::{}; const RESOURCE: ::core::option::Option<(&'static str, &'static str)> = ::core::option::Option::Some(({:?}, {:?})); }}", kind_expr(item.object_type), package.id(), item.id).unwrap();
    writeln!(out, "impl ::fairygui_bevy::GuiBinding for {name} {{ const URL: &'static str = {url:?}; const ITEM_NAME: &'static str = {:?};\nfn bind(gui: &::fairygui_bevy::GuiQuery, entity: ::bevy::prelude::Entity) -> ::core::result::Result<Self, ::fairygui_bevy::GuiBindingError> {{ let root = ::fairygui_bevy::GuiHandle::<Self>::new(gui, entity)?; ::core::result::Result::Ok(Self {{ {values} root }}) }} }}", item.name).unwrap();
    writeln!(out, "impl {name} {{ pub const URL: &'static str = <Self as ::fairygui_bevy::GuiBinding>::URL; pub const ITEM_NAME: &'static str = <Self as ::fairygui_bevy::GuiBinding>::ITEM_NAME; pub fn bind(gui: &::fairygui_bevy::GuiQuery, entity: ::bevy::prelude::Entity) -> ::core::result::Result<Self, ::fairygui_bevy::GuiBindingError> {{ <Self as ::fairygui_bevy::GuiBinding>::bind(gui, entity) }} }}").unwrap();
    writeln!(out, "impl ::fairygui_bevy::GuiExtension for {name} {{ const URL: &'static str = Self::URL; fn construct(entity: ::bevy::prelude::Entity, gui: &::fairygui_bevy::GuiQuery) -> ::core::result::Result<Self, ::fairygui_bevy::GuiExtensionError> {{ Self::bind(gui, entity).map_err(|error| ::fairygui_bevy::GuiExtensionError::Construction(error.to_string())) }} }}").unwrap();
    Ok(())
}

#[cfg(test)]
mod tests;
