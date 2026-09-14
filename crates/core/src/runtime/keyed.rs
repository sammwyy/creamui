use creamui_reactive::{Owner, Signal};
use std::collections::HashMap;
use std::hash::Hash;

use super::binding::SharedRuntime;
use super::node::RuntimeNodeId;
use super::transaction::RuntimeTransaction;

struct ListEntry {
    owner: Owner,
    root: RuntimeNodeId,
}

/// Wires `items` to a keyed, order-preserving list of subtrees under
/// `parent` — the runtime-tree analogue of `for_each(items, key, render)`.
/// An item whose key survives an update keeps its runtime node (and
/// therefore its owner, layer cache, focus, ...); only genuinely new or
/// removed keys mount or dispose.
pub fn create_keyed_list<T, K>(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    items: Signal<Vec<T>>,
    key: impl Fn(&T) -> K + 'static,
    render: impl Fn(&mut RuntimeTransaction, &Owner, &T) -> RuntimeNodeId + 'static,
) where
    T: Clone + 'static,
    K: Hash + Eq + Clone + 'static,
{
    let container = owner.child();
    let mut entries: HashMap<K, ListEntry> = HashMap::new();

    owner.effect(move || {
        let current = items.get();
        let keys: Vec<K> = current.iter().map(&key).collect();

        let mut next_entries: HashMap<K, ListEntry> = HashMap::with_capacity(keys.len());
        let mut ordered_roots: Vec<RuntimeNodeId> = Vec::with_capacity(keys.len());

        runtime.transaction(|tx| {
            for (item, k) in current.iter().zip(&keys) {
                let entry = match entries.remove(k) {
                    Some(entry) => entry,
                    None => {
                        let item_owner = container.child();
                        let root = render(tx, &item_owner, item);
                        ListEntry {
                            owner: item_owner,
                            root,
                        }
                    }
                };
                ordered_roots.push(entry.root);
                next_entries.insert(k.clone(), entry);
            }
            tx.reorder_children(parent, &ordered_roots);
        });

        for (_, removed) in entries.drain() {
            removed.owner.dispose();
            runtime.transaction(|tx| tx.remove_subtree(removed.root));
        }
        entries = next_entries;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Mutation, NodeKind, Runtime};
    use crate::PaintStyle;
    use creamui_theme::Color;
    use std::cell::Cell;
    use std::rc::Rc;

    fn leaf(tx: &mut RuntimeTransaction, _owner: &Owner, id: &u32) -> RuntimeNodeId {
        let node = tx.create_node(NodeKind::Container);
        tx.apply(Mutation::SetPaintStyle {
            node,
            style: PaintStyle {
                background: Some(Color::rgb(*id as u8, 0, 0).into()),
                ..Default::default()
            },
        });
        node
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

        assert_eq!(after, vec![before[2], before[0], before[1]]);
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
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 2);
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
            move |tx, item_owner, _id| {
                crate::runtime::create_binding(item_owner, inner_runtime.clone(), {
                    let signal_1 = signal_1.clone();
                    let runs = runs.clone();
                    move |_| {
                        let _ = signal_1.get();
                        runs.set(runs.get() + 1);
                    }
                });
                tx.create_node(NodeKind::Container)
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
