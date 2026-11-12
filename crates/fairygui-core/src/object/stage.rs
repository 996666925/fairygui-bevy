use super::*;

impl ObjectStore {
    /// 显示树是否连接到舞台。显式 visible 不影响此状态；显示齿轮会影响。
    pub fn is_on_stage(&self, id: GObjectId) -> bool {
        self.staged.contains(&id)
    }

    /// 把独立对象树连接到外部舞台。已有逻辑父节点时先摘下。
    pub fn attach_to_stage(&mut self, root: GObjectId) -> bool {
        if self
            .object(root)
            .is_none_or(|object| object.kind() == ObjectType::Group)
        {
            return false;
        }
        if self.stage_roots.contains(&root) {
            return true;
        }
        self.detach_from_parent(root);
        self.stage_roots.push(root);
        self.refresh_stage();
        true
    }

    /// 撤下外部舞台根，保留对象。树内对象使用 detach_from_parent。
    pub fn detach_from_stage(&mut self, root: GObjectId) -> bool {
        if !self.stage_roots.contains(&root) {
            return false;
        }
        self.stage_roots.retain(|id| *id != root);
        self.refresh_stage();
        true
    }

    fn stage_connected(&self, id: GObjectId) -> bool {
        let mut current = Some(id);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return false;
            }
            let Some(object) = self.object(id) else {
                return false;
            };
            if !object.display_connected() || !self.refresh_overlay_attached(id) {
                return false;
            }
            current = object.group();
        }
        true
    }

    pub(super) fn refresh_stage(&mut self) {
        if self.stage_batch > 0 || (self.stage_roots.is_empty() && self.staged.is_empty()) {
            return;
        }
        let mut order = Vec::new();
        let mut staged = HashSet::new();
        let mut pending: Vec<_> = self.stage_roots.iter().rev().copied().collect();
        while let Some(id) = pending.pop() {
            let Some(object) = self.object(id) else {
                continue;
            };
            if object.kind() == ObjectType::Group || !staged.insert(id) {
                continue;
            }
            order.push(id);
            for child in object.children().iter().rev().copied() {
                if self.stage_connected(child) {
                    pending.push(child);
                }
            }
        }
        for &id in &self.stage_order {
            if !staged.contains(&id) {
                self.stage_events.push_back((id, false));
            }
        }
        for &id in &order {
            if !self.staged.contains(&id) {
                self.stage_events.push_back((id, true));
            }
        }
        self.staged = staged;
        self.stage_order = order;
        for &(id, _) in &self.stage_events {
            if let Some(object) = self.objects.get_mut(id) {
                object.dirty |= dirty::LOOK;
                self.dirty_ids.insert(id);
            }
        }
        self.flush_stage_events();
    }

    fn flush_stage_events(&mut self) {
        if self.processing_stage {
            return;
        }
        self.processing_stage = true;
        while let Some((owner, added)) = self.stage_events.pop_front() {
            if self.contains(owner) {
                self.dispatch_transition_stage(owner, added);
            }
        }
        self.processing_stage = false;
    }
}
