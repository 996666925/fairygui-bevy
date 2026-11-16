//! Postorder construction, matching AsyncOperation's prepared-child pool.
use super::*;
use crate::package::PackageRegistry;
use std::{collections::VecDeque, sync::Arc};

#[derive(Debug)]
enum Step {
    Resource {
        package: Arc<FuiPackage>,
        index: usize,
        context: LoadContext,
        children: usize,
    },
    Plain {
        kind: ObjectType,
        package: String,
    },
}

#[derive(Debug)]
pub(crate) struct BuildPlan {
    steps: VecDeque<Step>,
    pool: Vec<GObjectId>,
    owned: Vec<GObjectId>,
    branch: String,
    total: usize,
}

impl BuildPlan {
    pub(crate) fn new(
        package: &FuiPackage,
        index: usize,
        context: &LoadContext,
        registry: Option<&PackageRegistry>,
    ) -> Result<Self> {
        let mut plan = Self {
            steps: VecDeque::new(),
            pool: Vec::new(),
            owned: Vec::new(),
            branch: package
                .branches()
                .get(context.branch_index as usize)
                .cloned()
                .unwrap_or_default(),
            total: 0,
        };
        plan.collect(
            Arc::new(package.clone()),
            index,
            context,
            registry,
            &mut Vec::new(),
        )?;
        plan.total = plan.steps.len();
        Ok(plan)
    }

    fn collect(
        &mut self,
        package: Arc<FuiPackage>,
        index: usize,
        context: &LoadContext,
        registry: Option<&PackageRegistry>,
        active: &mut Vec<(String, usize)>,
    ) -> Result<()> {
        let context = LoadContext {
            branch_index: package.branch_index_of(&self.branch),
            ..*context
        };
        let source_index = index;
        let index = package.resolve_branch(index, &context);
        let item = package
            .item(index)
            .ok_or_else(|| Error::ItemNotFound(format!("#{index}")))?;
        let identity = (package.id().to_owned(), index);
        if active.contains(&identity) {
            return Err(Error::invalid_data(
                0,
                format!("cyclic component reference: {} / {}", package.id(), item.id),
            ));
        }
        active.push(identity);
        let mut children = 0;
        if item.item_type == PackageItemType::Component {
            let mut buffer = package
                .raw_data(item)
                .ok_or_else(|| Error::invalid_data(0, "component item has no raw data"))?;
            if !buffer.seek(0, 2)? {
                return Err(Error::MissingBlock { block: 2 });
            }
            children = buffer.read_i16()?.max(0) as usize;
            for _ in 0..children {
                let length = buffer.read_i16()?.max(0) as usize;
                let start = buffer.position();
                if !buffer.seek(start, 0)? {
                    return Err(Error::MissingBlock { block: 0 });
                }
                let kind = ObjectType::from_code(buffer.read_u8()?);
                let source = buffer.read_s(package.string_table())?;
                let owner = buffer.read_s(package.string_table())?;
                let source_package = match owner.as_deref() {
                    None => Some(package.clone()),
                    Some(id) if id == package.id() => Some(package.clone()),
                    Some(id) => registry
                        .and_then(|registry| registry.get_by_id(id))
                        .cloned(),
                };
                let resource = source.as_deref().and_then(|id| {
                    let owner = source_package.as_ref()?;
                    Some((owner.clone(), owner.item_index_by_id(id)?))
                });
                if let Some((owner, index)) = resource {
                    self.collect(owner, index, &context, registry, active)?;
                } else {
                    self.steps.push_back(Step::Plain {
                        kind,
                        package: package.id().into(),
                    });
                }
                buffer.set_position(start + length);
            }
        }
        active.pop();
        self.steps.push_back(Step::Resource {
            package,
            index: source_index,
            context,
            children,
        });
        Ok(())
    }

    pub(crate) fn remaining(&self) -> usize {
        self.steps.len()
    }
    pub(crate) fn total(&self) -> usize {
        self.total
    }

    /// Each step allocates exactly one object, including plain inline children.
    pub(crate) fn advance(
        &mut self,
        store: &mut ObjectStore,
        registry: Option<&PackageRegistry>,
    ) -> Result<Option<GObjectId>> {
        let Some(step) = self.steps.pop_front() else {
            return Ok(None);
        };
        let object = match step {
            Step::Plain { kind, package } => {
                let object = store.create(kind);
                store.object_mut(object).unwrap().source_package_id = Some(package);
                object
            }
            Step::Resource {
                package,
                index,
                context,
                children,
            } => {
                let start = self
                    .pool
                    .len()
                    .checked_sub(children)
                    .ok_or_else(|| Error::invalid_data(0, "incomplete prepared child pool"))?;
                let children = self.pool.split_off(start);
                create_with_context(
                    store,
                    &package,
                    index,
                    &context,
                    &mut Construction {
                        registry,
                        active: Vec::new(),
                        branch: self.branch.clone(),
                        children: Some(children),
                    },
                )?
            }
        };
        self.owned.push(object);
        self.pool.push(object);
        if self.steps.is_empty() {
            if self.pool.len() != 1 {
                return Err(Error::invalid_data(0, "unconsumed prepared objects"));
            }
            self.owned.clear();
            return Ok(self.pool.pop());
        }
        Ok(None)
    }

    pub(crate) fn cancel(&mut self, store: &mut ObjectStore) {
        for object in self.owned.drain(..) {
            store.remove(object);
        }
        self.pool.clear();
        self.steps.clear();
    }
}
