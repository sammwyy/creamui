use creamui_reactive::{untrack, Owner, Signal};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use super::binding::SharedRuntime;
use super::node::RuntimeNodeId;
use super::region::ChildRegion;

struct ListEntry<T> {
    owner: Owner,
    root: RuntimeNodeId,
    item: Signal<T>,
}

pub fn create_keyed_list<T, K>(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    items: Signal<Vec<T>>,
    key: impl Fn(&T) -> K + 'static,
    render: impl Fn(&SharedRuntime, &Owner, Signal<T>) -> RuntimeNodeId + 'static,
) -> RuntimeNodeId
where
    T: Clone + PartialEq + 'static,
    K: Hash + Eq + Clone + 'static,
{
    create_keyed_list_with(owner, runtime, parent, move || items.get(), key, render)
}

pub fn create_keyed_list_with<T, K>(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    render: impl Fn(&SharedRuntime, &Owner, Signal<T>) -> RuntimeNodeId + 'static,
) -> RuntimeNodeId
where
    T: Clone + PartialEq + 'static,
    K: Hash + Eq + Clone + 'static,
{
    let scope = owner.child();
    let region = Rc::new(RefCell::new(ChildRegion::new(runtime.clone(), parent)));
    let anchor = region.borrow().anchor();
    scope.on_cleanup({
        let region = region.clone();
        move || region.borrow_mut().clear()
    });
    let container = scope.downgrade();
    let mut entries: HashMap<K, ListEntry<T>> = HashMap::new();
    scope.effect(move || {
        let current = items();
        let keys: Vec<K> = untrack(|| current.iter().map(&key).collect());
        let unique: HashSet<_> = keys.iter().collect();
        assert_eq!(unique.len(), keys.len(), "keyed list keys must be unique");
        untrack(|| {
            let Some(container) = container.upgrade() else {
                return;
            };
            if !region.borrow().is_live() {
                return;
            }
            let mut next_entries = HashMap::with_capacity(keys.len());
            let mut ordered_roots = Vec::with_capacity(keys.len());
            for (item, key) in current.into_iter().zip(keys) {
                let entry = match entries.remove(&key) {
                    Some(entry) => {
                        entry.item.set_if_changed(item);
                        entry
                    }
                    None => {
                        let owner = container.child();
                        let item = Signal::new(item);
                        let root = render(&runtime, &owner, item.clone());
                        ListEntry { owner, root, item }
                    }
                };
                ordered_roots.push(entry.root);
                next_entries.insert(key, entry);
                if container.is_disposed() || !region.borrow().is_live() {
                    Owner::dispose_many(next_entries.values().map(|entry| entry.owner.clone()));
                    runtime.transaction(|tx| tx.remove_subtrees(&ordered_roots));
                    entries.clear();
                    return;
                }
            }
            let removed: Vec<_> = entries.drain().map(|(_, entry)| entry).collect();
            Owner::dispose_many(removed.iter().map(|entry| entry.owner.clone()));
            let removed_roots: Vec<_> = removed.iter().map(|entry| entry.root).collect();
            runtime.transaction(|tx| tx.remove_subtrees(&removed_roots));
            if container.is_disposed() || !region.borrow().is_live() {
                Owner::dispose_many(next_entries.values().map(|entry| entry.owner.clone()));
                runtime.transaction(|tx| tx.remove_subtrees(&ordered_roots));
                return;
            }
            region.borrow_mut().replace(ordered_roots);
            entries = next_entries;
        });
    });
    anchor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Mutation, NodeKind, Runtime};
    use crate::PaintStyle;
    use creamui_theme::Color;
    use std::cell::Cell;
    use std::rc::Rc;

    fn leaf(runtime: &SharedRuntime, _owner: &Owner, id: Signal<u32>) -> RuntimeNodeId {
        runtime.transaction(|tx| {
            let node = tx.create_node(NodeKind::Container);
            tx.apply(Mutation::SetPaintStyle {
                node,
                style: PaintStyle {
                    background: Some(Color::rgb(id.get() as u8, 0, 0).into()),
                    ..Default::default()
                },
            });
            node
        })
    }

    #[test]
    fn reordering_preserves_node_identity() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let items = Signal::new(vec![1u32, 2, 3]);
        let owner = Owner::new();
        create_keyed_list(
            &owner,
            runtime.clone(),
            parent,
            items.clone(),
            |&id| id,
            leaf,
        );

        let before: Vec<RuntimeNodeId> =
            runtime.with(|r| r.get(parent).unwrap().children.as_slice().to_vec());

        items.set(vec![3, 1, 2]);
        let after: Vec<RuntimeNodeId> =
            runtime.with(|r| r.get(parent).unwrap().children.as_slice().to_vec());

        assert_eq!(after, vec![before[2], before[0], before[1], before[3]]);
    }

    #[test]
    fn removed_keys_are_disposed_and_new_keys_are_mounted() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let items = Signal::new(vec![1u32, 2]);
        let owner = Owner::new();
        create_keyed_list(
            &owner,
            runtime.clone(),
            parent,
            items.clone(),
            |&id| id,
            leaf,
        );
        let node_for_1 = runtime.with(|r| r.get(parent).unwrap().children.as_slice()[0]);

        items.set(vec![2, 3]);

        assert!(
            runtime.with(|r| r.get(node_for_1).is_none()),
            "key 1's node must be removed once its key drops out"
        );
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 3);
    }

    #[test]
    fn removing_a_key_disposes_bindings_created_for_its_item() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);
        let inner_runtime = SharedRuntime::new(Runtime::new());

        let signal_1 = Signal::new(1u8);
        let runs = Rc::new(Cell::new(0));
        let items = Signal::new(vec![1u32]);
        let owner = Owner::new();
        create_keyed_list(&owner, runtime.clone(), parent, items.clone(), |&id| id, {
            let signal_1 = signal_1.clone();
            let runs = runs.clone();
            let inner_runtime = inner_runtime.clone();
            move |runtime, item_owner, _id| {
                crate::runtime::create_binding(item_owner, inner_runtime.clone(), {
                    let signal_1 = signal_1.clone();
                    let runs = runs.clone();
                    move |_| {
                        let _ = signal_1.get();
                        runs.set(runs.get() + 1);
                    }
                });
                leaf(runtime, item_owner, Signal::new(0))
            }
        });
        assert_eq!(runs.get(), 1);

        items.set(vec![]);
        signal_1.set(2);
        assert_eq!(
            runs.get(),
            1,
            "removing an item's key must dispose the binding created for it"
        );
    }
}
