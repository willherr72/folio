//! Conservative proof for native transactions retaining original page/annotation
//! identities. Copied pages need a separate explicit relocation proof.
use super::*;
use lopdf::{Dictionary, Document, Object, ObjectId};

#[cfg_attr(test, track_caller)]
fn bad() -> EngineError {
    #[cfg(test)]
    eprintln!(
        "Annotation ownership proof refused at {}",
        std::panic::Location::caller()
    );
    EngineError::InvalidRequest("Native edit could not prove original annotation ownership.".into())
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    Exact,
    Annotation,
    Appearance,
    Resources,
}

struct CopyContext {
    old_page: ObjectId,
    new_page: ObjectId,
    annotations: BTreeMap<ObjectId, ObjectId>,
    target_roots: BTreeSet<ObjectId>,
    resources: BTreeMap<ObjectId, ObjectId>,
    reverse_resources: BTreeMap<ObjectId, ObjectId>,
}

struct Budget {
    work: usize,
    bytes: usize,
    metadata: usize,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
}
impl Budget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        if (self.cancelled)() {
            return Err(bad());
        }
        self.work = self
            .work
            .checked_add(1)
            .filter(|v| *v <= MAX_WORK)
            .ok_or_else(bad)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|v| *v <= 256 * 1024 * 1024)
            .ok_or_else(bad)?;
        if depth > 64 {
            return Err(bad());
        }
        Ok(())
    }
    fn entry(&mut self) -> EngineResult<()> {
        self.metadata = self
            .metadata
            .checked_add(256)
            .filter(|v| *v <= MAX_METADATA)
            .ok_or_else(bad)?;
        Ok(())
    }
}

struct Proof<'a, 'b> {
    old: &'a Document,
    new: &'a Document,
    pages: &'a BTreeSet<ObjectId>,
    budget: &'b mut Budget,
    seen: BTreeSet<(ObjectId, ObjectId, Mode)>,
    old_resources: &'a Object,
    new_resources: &'a Object,
    copy: Option<CopyContext>,
}
impl Proof<'_, '_> {
    fn value(
        &mut self,
        old: &Object,
        new: &Object,
        mut mode: Mode,
        depth: usize,
    ) -> EngineResult<()> {
        self.budget.take(0, depth)?;
        // Page identities are transaction anchors. Their content/resources may
        // change; inherited AP resources are compared separately below.
        if let (Object::Reference(a), Object::Reference(b)) = (old, new) {
            if self.pages.contains(a)
                || self.pages.contains(b)
                || self
                    .copy
                    .as_ref()
                    .is_some_and(|c| *a == c.new_page || *b == c.new_page)
            {
                return if a == b && self.pages.contains(a) {
                    Ok(())
                } else {
                    Err(bad())
                };
            }
            // Only the typed AP preservation operation relocates objects. A
            // metadata/action/popup edge cannot silently adopt an equal object.
            if let Some(copy) = &mut self.copy {
                if let Some(mapped) = copy.annotations.get(a) {
                    if mapped != b {
                        return Err(bad());
                    }
                    mode = Mode::Annotation;
                } else if matches!(mode, Mode::Resources | Mode::Appearance) {
                    if (*a != *b && self.old.objects.contains_key(b))
                        || copy.resources.get(a).is_some_and(|mapped| mapped != b)
                        || copy
                            .reverse_resources
                            .get(b)
                            .is_some_and(|mapped| mapped != a)
                    {
                        return Err(bad());
                    }
                    if !copy.resources.contains_key(a) {
                        self.budget.entry()?;
                        self.budget.entry()?;
                        copy.resources.insert(*a, *b);
                        copy.reverse_resources.insert(*b, *a);
                    }
                } else if a != b {
                    return Err(bad());
                }
            } else if mode != Mode::Appearance && a != b {
                return Err(bad());
            }
            let old_value = self.old.objects.get(a).ok_or_else(bad)?;
            let new_value = self.new.objects.get(b).ok_or_else(bad)?;
            // Memoization handles genuine object-graph cycles (popup parents,
            // resource dictionaries). A reference-only cycle resolves to no PDF
            // value and must fail before it can enter that memo.
            if matches!(old_value, Object::Reference(_)) {
                resolve(self.old, old, self.budget)?;
            }
            if matches!(new_value, Object::Reference(_)) {
                resolve(self.new, new, self.budget)?;
            }
            // Popup /Parent cycles revisit an annotation already being proved
            // with its typed AP context, not an untyped stream comparison.
            if self.seen.contains(&(*a, *b, Mode::Annotation)) {
                return Ok(());
            }
            if self.seen.contains(&(*a, *b, mode)) {
                return Ok(());
            }
            self.budget.entry()?;
            self.seen.insert((*a, *b, mode));
            return self.value(old_value, new_value, mode, depth + 1);
        }
        // Resolving a direct/indirect change is permitted only inside the proved
        // closure; root annotation ownership is checked before this traversal.
        if let Object::Reference(id) = old {
            if !matches!(mode, Mode::Appearance | Mode::Resources) || self.pages.contains(id) {
                return Err(bad());
            }
            return self.value(
                self.old.objects.get(id).ok_or_else(bad)?,
                new,
                mode,
                depth + 1,
            );
        }
        if let Object::Reference(id) = new {
            let copied_direct_root = mode == Mode::Annotation
                && self
                    .copy
                    .as_ref()
                    .is_some_and(|c| c.target_roots.contains(id));
            if (!matches!(mode, Mode::Appearance | Mode::Resources) && !copied_direct_root)
                || self.pages.contains(id)
                || self.copy.as_ref().is_some_and(|c| c.new_page == *id)
            {
                return Err(bad());
            }
            return self.value(
                old,
                self.new.objects.get(id).ok_or_else(bad)?,
                mode,
                depth + 1,
            );
        }
        match (old, new) {
            (Object::Array(a), Object::Array(b)) if a.len() == b.len() => {
                for (a, b) in a.iter().zip(b) {
                    self.value(
                        a,
                        b,
                        if mode == Mode::Resources {
                            Mode::Resources
                        } else {
                            Mode::Exact
                        },
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            (Object::Dictionary(a), Object::Dictionary(b)) => {
                self.dict(a, b, mode, false, depth + 1)
            }
            (Object::Stream(a), Object::Stream(b)) => {
                if a.content.len() > 64 * 1024 * 1024 || b.content.len() > 64 * 1024 * 1024 {
                    return Err(bad());
                }
                self.budget.take(
                    a.content
                        .len()
                        .checked_add(b.content.len())
                        .ok_or_else(bad)?,
                    depth,
                )?;
                if a.content != b.content {
                    return Err(bad());
                }
                self.dict(&a.dict, &b.dict, mode, true, depth + 1)?;
                if mode == Mode::Appearance {
                    let old = a.dict.get(b"Resources").unwrap_or(self.old_resources);
                    let new = b.dict.get(b"Resources").unwrap_or(self.new_resources);
                    self.value(
                        old,
                        new,
                        if self.copy.is_some() {
                            Mode::Resources
                        } else {
                            Mode::Exact
                        },
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            (Object::String(a, af), Object::String(b, bf)) => {
                self.budget
                    .take(a.len().checked_add(b.len()).ok_or_else(bad)?, depth)?;
                if a == b && af == bf {
                    Ok(())
                } else {
                    Err(bad())
                }
            }
            (Object::Name(a), Object::Name(b)) => {
                self.budget
                    .take(a.len().checked_add(b.len()).ok_or_else(bad)?, depth)?;
                if a == b {
                    Ok(())
                } else {
                    Err(bad())
                }
            }
            _ if old == new => Ok(()),
            _ => Err(bad()),
        }
    }
    fn dict(
        &mut self,
        old: &Dictionary,
        new: &Dictionary,
        mode: Mode,
        stream: bool,
        depth: usize,
    ) -> EngineResult<()> {
        let skip = |key: &[u8]| {
            stream && (key == b"Length" || (mode == Mode::Appearance && key == b"Resources"))
        };
        for (key, value) in old.iter() {
            self.budget.take(key.len(), depth)?;
            if skip(key) {
                continue;
            }
            if mode == Mode::Annotation && key == b"P" {
                if let Some(copy) = &self.copy {
                    if resolve(self.old, value, self.budget)?.0 != Some(copy.old_page)
                        || resolve(self.new, new.get(key).map_err(|_| bad())?, self.budget)?.0
                            != Some(copy.new_page)
                    {
                        return Err(bad());
                    }
                    continue;
                }
            }
            let next = if mode == Mode::Annotation && key == b"AP" {
                Mode::Appearance
            } else if mode == Mode::Appearance && !stream {
                Mode::Appearance
            } else if mode == Mode::Resources {
                Mode::Resources
            } else {
                Mode::Exact
            };
            self.value(value, new.get(key).map_err(|_| bad())?, next, depth + 1)?;
        }
        for (key, _) in new.iter() {
            self.budget.take(key.len(), depth)?;
            if !skip(key) && !old.has(key) {
                if mode == Mode::Annotation && key == b"P" {
                    if let Some(copy) = &self.copy {
                        if resolve(self.new, new.get(key).map_err(|_| bad())?, self.budget)?.0
                            == Some(copy.new_page)
                        {
                            continue;
                        }
                    }
                }
                return Err(bad());
            }
        }
        Ok(())
    }
}

fn resolve<'a>(
    pdf: &'a Document,
    mut value: &'a Object,
    budget: &mut Budget,
) -> EngineResult<(Option<ObjectId>, &'a Object)> {
    let mut id = None;
    for depth in 0..=64 {
        budget.take(0, depth)?;
        match value {
            Object::Reference(next) => {
                id = Some(*next);
                value = pdf.objects.get(next).ok_or_else(bad)?;
            }
            _ => return Ok((id, value)),
        }
    }
    Err(bad())
}
fn page_value<'a>(
    pdf: &'a Document,
    mut page: ObjectId,
    key: &[u8],
    inherit: bool,
    budget: &mut Budget,
) -> EngineResult<Option<&'a Object>> {
    for depth in 0..=64 {
        budget.take(0, depth)?;
        let dict = resolve(pdf, pdf.objects.get(&page).ok_or_else(bad)?, budget)?
            .1
            .as_dict()
            .map_err(|_| bad())?;
        if let Ok(value) = dict.get(key) {
            return Ok(Some(value));
        }
        if !inherit {
            return Ok(None);
        }
        match dict.get(b"Parent") {
            Ok(value) => page = value.as_reference().map_err(|_| bad())?,
            Err(_) => return Ok(None),
        }
    }
    Err(bad())
}
fn annotations<'a>(
    pdf: &'a Document,
    page: ObjectId,
    budget: &mut Budget,
) -> EngineResult<&'a [Object]> {
    let dict = resolve(pdf, pdf.objects.get(&page).ok_or_else(bad)?, budget)?
        .1
        .as_dict()
        .map_err(|_| bad())?;
    match dict.get(b"Annots") {
        Err(_) => Ok(&[]),
        Ok(value) => {
            let values = resolve(pdf, value, budget)?
                .1
                .as_array()
                .map_err(|_| bad())?;
            if values.len() > 10_000 {
                return Err(bad());
            }
            Ok(values)
        }
    }
}

fn page_tree(pdf: &Document, budget: &mut Budget) -> EngineResult<Vec<ObjectId>> {
    fn walk(
        pdf: &Document,
        node: &Object,
        budget: &mut Budget,
        seen: &mut BTreeSet<ObjectId>,
        output: &mut Vec<ObjectId>,
        depth: usize,
    ) -> EngineResult<usize> {
        budget.take(0, depth)?;
        let (id, value) = resolve(pdf, node, budget)?;
        let id = id.ok_or_else(bad)?;
        budget.entry()?;
        if !seen.insert(id) {
            return Err(bad());
        }
        let dict = value.as_dict().map_err(|_| bad())?;
        let kind = resolve(pdf, dict.get(b"Type").map_err(|_| bad())?, budget)?
            .1
            .as_name()
            .map_err(|_| bad())?;
        match kind {
            b"Page" => {
                if output.len() >= 100_000 {
                    return Err(bad());
                }
                budget.entry()?;
                output.push(id);
                Ok(1)
            }
            b"Pages" => {
                let count = resolve(pdf, dict.get(b"Count").map_err(|_| bad())?, budget)?
                    .1
                    .as_i64()
                    .map_err(|_| bad())?;
                if !(0..=100_000).contains(&count) {
                    return Err(bad());
                }
                let kids = resolve(pdf, dict.get(b"Kids").map_err(|_| bad())?, budget)?
                    .1
                    .as_array()
                    .map_err(|_| bad())?;
                let mut actual = 0usize;
                for child in kids {
                    actual = actual
                        .checked_add(walk(pdf, child, budget, seen, output, depth + 1)?)
                        .filter(|v| *v <= 100_000)
                        .ok_or_else(bad)?;
                }
                if actual != count as usize {
                    return Err(bad());
                }
                Ok(actual)
            }
            _ => Err(bad()),
        }
    }
    let catalog = resolve(pdf, pdf.trailer.get(b"Root").map_err(|_| bad())?, budget)?
        .1
        .as_dict()
        .map_err(|_| bad())?;
    let mut output = Vec::new();
    walk(
        pdf,
        catalog.get(b"Pages").map_err(|_| bad())?,
        budget,
        &mut BTreeSet::new(),
        &mut output,
        0,
    )?;
    Ok(output)
}

fn mapped_slot(
    slot: usize,
    removed: &[(usize, usize)],
    budget: &mut Budget,
) -> EngineResult<usize> {
    let (mut low, mut high) = (0, removed.len());
    while low < high {
        budget.take(0, 0)?;
        let middle = low + (high - low) / 2;
        if removed[middle].1 < slot {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    if removed.get(low).is_some_and(|v| v.1 == slot) {
        return Err(bad());
    }
    slot.checked_sub(low).ok_or_else(bad)
}

#[derive(Clone, Copy)]
enum RemovalKind {
    Widgets,
    NativeComments,
}

fn validate_removal(
    old: &Document,
    new: &Document,
    before: &[Object],
    after: &[Object],
    removed: &[(usize, usize)],
    kind: RemovalKind,
    budget: &mut Budget,
) -> EngineResult<()> {
    if before.len().checked_sub(removed.len()) != Some(after.len()) {
        return Err(bad());
    }
    let mut deleted = 0;
    let mut kept = 0;
    for (index, value) in before.iter().enumerate() {
        budget.take(0, 0)?;
        let (old_id, object) = resolve(old, value, budget)?;
        let dict = object.as_dict().map_err(|_| bad())?;
        if removed.get(deleted).is_some_and(|v| v.1 == index) {
            let subtype = resolve(old, dict.get(b"Subtype").map_err(|_| bad())?, budget)?
                .1
                .as_name()
                .map_err(|_| bad())?;
            let allowed = match kind {
                RemovalKind::Widgets => old_id.is_some() && subtype == b"Widget",
                RemovalKind::NativeComments => {
                    subtype == b"Text" || (old_id.is_some() && subtype == b"Popup")
                }
            };
            if !allowed {
                return Err(bad());
            }
            deleted += 1;
        } else {
            let target = after.get(kept).ok_or_else(bad)?;
            let (new_id, object) = resolve(new, target, budget)?;
            object.as_dict().map_err(|_| bad())?;
            // The native removal only filters its selected annotation entries. It
            // does not reorder, relocate or wrap the remaining annotation roots.
            if old_id != new_id || value.as_reference().ok() != target.as_reference().ok() {
                return Err(bad());
            }
            kept += 1;
        }
    }
    if deleted != removed.len() || kept != after.len() {
        return Err(bad());
    }
    Ok(())
}

impl Index {
    /// Only used by native retained-graph transactions, never arbitrary file
    /// opens. Immutable source bytes plus original root identities are authority.
    pub(super) fn retained_snapshot(&self, next: &Index, sources: (&crate::source_storage::SourceHandle, &crate::source_storage::SourceHandle)) -> EngineResult<Snapshot> {
        self.retained_snapshot_controlled(next, sources, Arc::new(|| false))
    }
    fn retained_snapshot_controlled(
        &self,
        next: &Index,
        sources: (&crate::source_storage::SourceHandle, &crate::source_storage::SourceHandle),
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> EngineResult<Snapshot> {
        self.retained_snapshot_transfer(next, sources, None, cancelled)
    }
    fn retained_snapshot_transfer(
        &self,
        next: &Index,
        sources: (&crate::source_storage::SourceHandle, &crate::source_storage::SourceHandle),
        removed_widgets: Option<&[(usize, usize)]>,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> EngineResult<Snapshot> {
        self.retained_snapshot_after_removal(
            next, sources,
            removed_widgets.map(|slots| (RemovalKind::Widgets, slots)),
            cancelled,
        )
    }
    fn retained_snapshot_after_removal(
        &self,
        next: &Index,
        sources: (&crate::source_storage::SourceHandle, &crate::source_storage::SourceHandle),
        removal: Option<(RemovalKind, &[(usize, usize)])>,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> EngineResult<Snapshot> {
        self.retained_snapshot_transaction(next, sources, removal, None, cancelled)
    }
    fn retained_snapshot_transaction(
        &self,
        next: &Index,
        sources: (&crate::source_storage::SourceHandle, &crate::source_storage::SourceHandle),
        removal: Option<(RemovalKind, &[(usize, usize)])>,
        copied_from: Option<usize>,
        cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> EngineResult<Snapshot> {
        if cancelled() {
            return Err(bad());
        }
        if !self.bound_to(sources.0.identity()) || !next.bound_to(sources.1.identity()) || (copied_from.is_some() && removal.is_some()) {
            return Err(bad());
        }
        let removed = removal.map(|(_, slots)| slots).unwrap_or(&[]);
        if removed.len() > MAX_WORK {
            return Err(bad());
        }
        let mut budget = Budget {
            work: 0,
            bytes: 0,
            metadata: self
                .used
                .checked_add(next.used)
                .and_then(|v| {
                    v.checked_add(
                        removed
                            .len()
                            .checked_mul(std::mem::size_of::<(usize, usize)>())?,
                    )
                })
                .ok_or_else(bad)?,
            cancelled: cancelled.clone(),
        };
        if budget.metadata > MAX_METADATA {
            return Err(bad());
        }
        for (i, pair) in removed.iter().enumerate() {
            budget.take(0, 0)?;
            if pair.1 >= 10_000 || (i > 0 && removed[i - 1] >= *pair) {
                return Err(bad());
            }
        }
        let limits = lopdf::ResourceLimits {
            input_bytes: 1024 * 1024 * 1024,
            object_bytes: 256 * 1024 * 1024,
            ..Default::default()
        };
        if self.source.len() > 512 * 1024 * 1024 || next.source.len() > 512 * 1024 * 1024 {
            return Err(bad());
        }
        let old_bytes = sources.0.lease()?;
        let new_bytes = sources.1.lease()?;
        let (old, remaining) =
            Document::load_mem_with_resource_control(&old_bytes, limits, cancelled.clone())
                .map_err(|_| bad())?;
        let (new, _) =
            Document::load_mem_with_resource_control(&new_bytes, remaining, cancelled.clone())
                .map_err(|_| bad())?;
        if old.objects.len().saturating_add(new.objects.len()) > 100_000 {
            return Err(bad());
        }
        // The general reader's page iterator silently skips malformed entries
        // and can truncate traversal. Ownership requires a complete, unique,
        // budgeted tree, with reservations before retaining each page identity.
        let old_pages = page_tree(&old, &mut budget)?;
        let new_pages = page_tree(&new, &mut budget)?;
        if let Some(origin) = copied_from {
            if origin >= old_pages.len()
                || new_pages.len() != old_pages.len() + 1
                || new_pages[..old_pages.len()] != old_pages
                || old.objects.contains_key(new_pages.last().ok_or_else(bad)?)
            {
                return Err(bad());
            }
        } else if old_pages != new_pages {
            return Err(bad());
        }
        if removed.last().is_some_and(|p| p.0 >= old_pages.len()) {
            return Err(bad());
        }
        for _ in &old_pages {
            budget.entry()?;
        }
        let pages = old_pages.iter().copied().collect();
        let mut saved = next.snapshot_with_budget(MAX_METADATA)?.ok_or_else(bad)?;
        // Reserve before indexing, then merge sorted slots in linear time. Do
        // not scan every binding for every page or search a whole target vector.
        for _ in &self.bindings {
            budget.entry()?;
        }
        let mut originals: Vec<_> = self.bindings.iter().map(|b| (b.page_index, b)).collect();
        if let Some(origin) = copied_from {
            for binding in &self.bindings {
                budget.take(0, 0)?;
                if binding.page_index == origin {
                    budget.entry()?;
                    originals.push((old_pages.len(), binding));
                }
            }
        }
        if originals.len() != saved.bindings.len() {
            return Err(bad());
        }
        originals.sort_unstable_by_key(|(page, b)| (*page, b.annotation_index));
        saved
            .bindings
            .sort_unstable_by_key(|b| (b.page_index, b.annotation_index));
        let empty = Object::Dictionary(Dictionary::new());
        let mut visited = 0;
        let mut removal_cursor = 0;
        for (page_index, new_page) in new_pages.iter().enumerate() {
            budget.take(0, 0)?;
            let copied = page_index == old_pages.len();
            let origin = if copied {
                copied_from.ok_or_else(bad)?
            } else {
                page_index
            };
            let page = &old_pages[origin];
            let a = annotations(&old, *page, &mut budget)?;
            let b = annotations(&new, *new_page, &mut budget)?;
            let removal_start = removal_cursor;
            while removed
                .get(removal_cursor)
                .is_some_and(|r| r.0 == page_index)
            {
                budget.take(0, 0)?;
                removal_cursor += 1;
            }
            let page_removed = &removed[removal_start..removal_cursor];
            if let Some((kind, _)) = removal {
                validate_removal(&old, &new, a, b, page_removed, kind, &mut budget)?;
            }
            let old_resources =
                page_value(&old, *page, b"Resources", true, &mut budget)?.unwrap_or(&empty);
            let new_resources =
                page_value(&new, *new_page, b"Resources", true, &mut budget)?.unwrap_or(&empty);
            let mut proof = Proof {
                old: &old,
                new: &new,
                pages: &pages,
                budget: &mut budget,
                seen: BTreeSet::new(),
                old_resources,
                new_resources,
                copy: None,
            };
            if copied {
                let mut context = CopyContext {
                    old_page: *page,
                    new_page: *new_page,
                    annotations: BTreeMap::new(),
                    target_roots: BTreeSet::new(),
                    resources: BTreeMap::new(),
                    reverse_resources: BTreeMap::new(),
                };
                // Seed only explicit original slots, before following Popup/Parent
                // cycles. Equal names or appearances never establish ownership.
                for (_, binding) in originals[visited..]
                    .iter()
                    .take_while(|(p, _)| *p == page_index)
                {
                    for slot in
                        std::iter::once(&binding.annotation_index).chain(&binding.popup_indices)
                    {
                        let (old_id, old_value) =
                            resolve(&old, a.get(*slot).ok_or_else(bad)?, proof.budget)?;
                        let (new_id, new_value) =
                            resolve(&new, b.get(*slot).ok_or_else(bad)?, proof.budget)?;
                        let old_dict = old_value.as_dict().map_err(|_| bad())?;
                        let new_dict = new_value.as_dict().map_err(|_| bad())?;
                        // Tagged annotation relocation needs a separate typed
                        // structure transaction; integer equality is not proof.
                        if old_dict.has(b"StructParent") || new_dict.has(b"StructParent") {
                            return Err(bad());
                        }
                        match (old_id, new_id) {
                            (Some(old_id), Some(new_id)) => {
                                proof.budget.entry()?;
                                proof.budget.entry()?;
                                if old.objects.contains_key(&new_id)
                                    || context.annotations.insert(old_id, new_id).is_some()
                                    || !context.target_roots.insert(new_id)
                                {
                                    return Err(bad());
                                }
                            }
                            (None, Some(new_id)) => {
                                proof.budget.entry()?;
                                if old.objects.contains_key(&new_id)
                                    || !context.target_roots.insert(new_id)
                                {
                                    return Err(bad());
                                }
                            }
                            (None, None) => {}
                            _ => return Err(bad()),
                        }
                    }
                }
                proof.copy = Some(context);
            }
            let default_rotate = Object::Integer(0);
            let default_unit = Object::Integer(1);
            let old_media =
                page_value(&old, *page, b"MediaBox", true, proof.budget)?.ok_or_else(bad)?;
            let new_media =
                page_value(&new, *new_page, b"MediaBox", true, proof.budget)?.ok_or_else(bad)?;
            proof.value(old_media, new_media, Mode::Exact, 0)?;
            for (key, inherit, old_default, new_default) in [
                (b"CropBox".as_slice(), true, old_media, new_media),
                (b"Rotate".as_slice(), true, &default_rotate, &default_rotate),
                (b"UserUnit".as_slice(), false, &default_unit, &default_unit),
            ] {
                let a = page_value(&old, *page, key, inherit, proof.budget)?.unwrap_or(old_default);
                let b =
                    page_value(&new, *new_page, key, inherit, proof.budget)?.unwrap_or(new_default);
                proof.value(a, b, Mode::Exact, 0)?;
            }
            // Match by explicit original page/slot and canonical identity. Never
            // search NM, content, coordinates, or a semantic digest for an owner.
            while let Some((_, binding)) = originals.get(visited).filter(|(p, _)| *p == page_index)
            {
                proof.budget.take(0, 0)?;
                let target = saved.bindings.get_mut(visited).ok_or_else(bad)?;
                if (target.page_index, target.annotation_index)
                    != (
                        page_index,
                        mapped_slot(binding.annotation_index, page_removed, proof.budget)?,
                    )
                {
                    return Err(bad());
                }
                if (!copied && binding.annotation_id != target.annotation_id)
                    || binding.popup_indices.len() != target.popup_indices.len()
                    || binding.initial_semantics_sha256 != target.initial_semantics_sha256
                {
                    return Err(bad());
                }
                for (old_slot, new_slot) in binding.popup_indices.iter().zip(&target.popup_indices)
                {
                    if mapped_slot(*old_slot, page_removed, proof.budget)? != *new_slot {
                        return Err(bad());
                    }
                }
                for slot in std::iter::once(&binding.annotation_index).chain(&binding.popup_indices)
                {
                    let a = a.get(*slot).ok_or_else(bad)?;
                    let b = b
                        .get(mapped_slot(*slot, page_removed, proof.budget)?)
                        .ok_or_else(bad)?;
                    let ai = resolve(&old, a, proof.budget)?.0;
                    let bi = resolve(&new, b, proof.budget)?.0;
                    if (!copied && ai != bi)
                        || (*slot == binding.annotation_index
                            && (ai != binding.annotation_id || bi != target.annotation_id))
                    {
                        return Err(bad());
                    }
                    proof.value(a, b, Mode::Annotation, 0)?;
                }
                target.overlay_id = binding.overlay_id.clone();
                visited += 1;
            }
        }
        if visited != originals.len() || removal_cursor != removed.len() {
            return Err(bad());
        }
        saved.metadata_reservation()?;
        Ok(saved)
    }
}

impl WorkerRuntime {
    fn retained_annotation_snapshot(&mut self, candidate: &str, result: EngineResult<Snapshot>) -> EngineResult<Option<Snapshot>> {
        match result {
            // A semantic refusal keeps the established unbound route, but an
            // unavailable backing is an operation failure, not new authority.
            Err(error @ EngineError::Io(_)) => { self.discard_unpublished_source(candidate); Err(error) },
            result => Ok(result.ok()),
        }
    }

    /// The complete temporary import view is validated before it is omitted from
    /// the reply. Callers already hold session drafts; no new drafts replace them.
    pub(in crate::engine) fn retain_session_annotation_sources(
        &mut self,
        original: &str,
        info: DocumentInfo,
        removed_comments: Option<&[(usize, usize)]>,
    ) -> EngineResult<DocumentInfo> {
        let snapshot = self
            .document(original)?
            .imported_overlay_sources
            .retained_snapshot_after_removal(
                &self.document(&info.id)?.imported_overlay_sources,
                (&self.document(original)?.source_bytes, &self.document(&info.id)?.source_bytes),
                removed_comments.map(|slots| (RemovalKind::NativeComments, slots)),
                Arc::new(|| false),
            )
            ;
        let snapshot = self.retained_annotation_snapshot(&info.id, snapshot)?;
        let mut info = self.restore_annotation_sources(info, snapshot)?;
        for page in &mut info.pages {
            page.overlays.clear();
        }
        Ok(info)
    }
    pub(in crate::engine) fn retain_annotation_sources_after_widget_removal(
        &mut self,
        original: &str,
        info: DocumentInfo,
        removed: &[(usize, usize)],
    ) -> EngineResult<DocumentInfo> {
        let snapshot = self
            .document(original)?
            .imported_overlay_sources
            .retained_snapshot_transfer(
                &self.document(&info.id)?.imported_overlay_sources,
                (&self.document(original)?.source_bytes, &self.document(&info.id)?.source_bytes),
                Some(removed),
                Arc::new(|| false),
            )
            ;
        let snapshot = self.retained_annotation_snapshot(&info.id, snapshot)?;
        self.restore_annotation_sources(info, snapshot)
    }
    pub(in crate::engine) fn retain_annotation_sources_with_copy(
        &mut self,
        original: &str,
        info: DocumentInfo,
        copied_from: Option<usize>,
    ) -> EngineResult<DocumentInfo> {
        if copied_from.is_none() {
            return self.retain_annotation_sources(original, info);
        }
        let snapshot = self
            .document(original)?
            .imported_overlay_sources
            .retained_snapshot_transaction(
                &self.document(&info.id)?.imported_overlay_sources,
                (&self.document(original)?.source_bytes, &self.document(&info.id)?.source_bytes),
                None,
                copied_from,
                Arc::new(|| false),
            )
            ;
        let snapshot = self.retained_annotation_snapshot(&info.id, snapshot)?;
        self.restore_annotation_sources(info, snapshot)
    }
    pub(in crate::engine) fn retain_annotation_sources(
        &mut self,
        original: &str,
        info: DocumentInfo,
    ) -> EngineResult<DocumentInfo> {
        let snapshot = self
            .document(original)?
            .imported_overlay_sources
            .retained_snapshot(&self.document(&info.id)?.imported_overlay_sources,
                (&self.document(original)?.source_bytes, &self.document(&info.id)?.source_bytes))
            ;
        let snapshot = self.retained_annotation_snapshot(&info.id, snapshot)?;
        // A legacy origin or unproved relocation must not turn random fresh IDs
        // into authority over existing drafts. Keep editing data, explicitly unbound.
        self.restore_annotation_sources(info, snapshot)
    }
}

#[cfg(test)]
#[path = "imported_overlay_transfer_tests.rs"]
mod tests;
