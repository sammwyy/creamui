use creamui_reactive::{Owner, Signal};
use std::cell::RefCell;
use std::rc::Rc;

use super::binding::SharedRuntime;
use super::node::RuntimeNodeId;
use super::transaction::RuntimeTransaction;

/// A conditionally-mounted subtree anchored at a stable parent node — the
/// runtime-tree analogue of `if condition.get() { <Panel/> }`: only the
/// branch's own subtree changes when the condition flips, never the rest
/// of the tree.
struct BranchBinding {
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    active: Option<(Owner, RuntimeNodeId)>,
}

impl BranchBinding {
    fn new(runtime: SharedRuntime, parent: RuntimeNodeId) -> Self {
        BranchBinding {
            runtime,
            parent,
            active: None,
        }
    }

    fn show(
        &mut self,
        container: &Owner,
        mount: &dyn Fn(&mut RuntimeTransaction, &Owner) -> RuntimeNodeId,
    ) {
        if self.active.is_some() {
            return;
        }
        let branch_owner = container.child();
        let root = self.runtime.transaction(|tx| {
            let root = mount(tx, &branch_owner);
            tx.insert_child(self.parent, root, None);
            root
        });
        self.active = Some((branch_owner, root));
    }

    fn hide(&mut self) {
        let Some((owner, root)) = self.active.take() else {
            return;
        };
        owner.dispose();
        self.runtime.transaction(|tx| tx.remove_subtree(root));
    }
}

/// Wires `condition` to mount/unmount a subtree under `parent` — see
/// [`BranchBinding`]. `owner` should be the enclosing scope; the mounted
/// content gets its own child scope, disposed on every hide and whenever
/// `owner` itself is disposed.
pub fn create_branch(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    condition: Signal<bool>,
    mount: impl Fn(&mut RuntimeTransaction, &Owner) -> RuntimeNodeId + 'static,
) {
    let container = owner.child();
    let branch = Rc::new(RefCell::new(BranchBinding::new(runtime, parent)));
    owner.effect(move || {
        if condition.get() {
            branch.borrow_mut().show(&container, &mount);
        } else {
            branch.borrow_mut().hide();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{create_binding, NodeKind, Runtime};
    use std::cell::Cell;

    #[test]
    fn toggling_the_condition_mounts_and_unmounts_the_branch() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let condition = Signal::new(false);
        let owner = Owner::new();
        create_branch(
            &owner,
            runtime.clone(),
            parent,
            condition.clone(),
            |tx, _| tx.create_node(NodeKind::Container),
        );

        assert!(runtime.with(|r| r.get(parent).unwrap().children.is_empty()));

        condition.set(true);
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 1);

        condition.set(false);
        assert!(runtime.with(|r| r.get(parent).unwrap().children.is_empty()));
    }

    #[test]
    fn hiding_the_branch_disposes_bindings_created_inside_it() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);
        let inner_runtime = SharedRuntime::new(Runtime::new());

        let condition = Signal::new(true);
        let inner_signal = Signal::new(1u8);
        let inner_runs = Rc::new(Cell::new(0));
        let owner = Owner::new();
        create_branch(&owner, runtime.clone(), parent, condition.clone(), {
            let inner_signal = inner_signal.clone();
            let inner_runs = inner_runs.clone();
            let inner_runtime = inner_runtime.clone();
            move |tx, branch_owner| {
                create_binding(branch_owner, inner_runtime.clone(), {
                    let inner_signal = inner_signal.clone();
                    let inner_runs = inner_runs.clone();
                    move |_| {
                        let _ = inner_signal.get();
                        inner_runs.set(inner_runs.get() + 1);
                    }
                });
                tx.create_node(NodeKind::Container)
            }
        });
        assert_eq!(inner_runs.get(), 1);

        condition.set(false);
        assert!(runtime.with(|r| r.get(parent).unwrap().children.is_empty()));

        inner_signal.set(2);
        assert_eq!(
            inner_runs.get(),
            1,
            "hiding the branch must dispose bindings created inside it"
        );
    }

    #[test]
    fn disposing_the_owner_stops_the_branch_from_reacting_to_further_toggles() {
        let mut runtime = Runtime::new();
        let parent = {
            let mut tx = runtime.transaction();
            tx.create_node(NodeKind::Container)
        };
        let runtime = SharedRuntime::new(runtime);

        let condition = Signal::new(true);
        let owner = Owner::new();
        create_branch(
            &owner,
            runtime.clone(),
            parent,
            condition.clone(),
            |tx, _| tx.create_node(NodeKind::Container),
        );
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 1);

        owner.dispose();
        condition.set(false);
        assert_eq!(
            runtime.with(|r| r.get(parent).unwrap().children.len()),
            1,
            "a disposed branch must not react to further condition changes"
        );
    }
}
