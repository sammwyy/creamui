use creamui_reactive::{untrack, Owner, Signal};
use std::cell::RefCell;
use std::rc::Rc;

use super::binding::SharedRuntime;
use super::node::RuntimeNodeId;
use super::region::ChildRegion;

struct BranchBinding {
    region: ChildRegion,
    variant: Option<bool>,
    active: Option<Owner>,
}

pub fn create_branch(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    condition: Signal<bool>,
    mount: impl Fn(&SharedRuntime, &Owner) -> RuntimeNodeId + 'static,
) -> RuntimeNodeId {
    create_branch_when(owner, runtime, parent, move || condition.get(), mount)
}

pub fn create_branch_when(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    condition: impl Fn() -> bool + 'static,
    mount: impl Fn(&SharedRuntime, &Owner) -> RuntimeNodeId + 'static,
) -> RuntimeNodeId {
    create_switch(
        owner,
        runtime,
        parent,
        condition,
        move |runtime, owner, value| value.then(|| mount(runtime, owner)),
    )
}

pub fn create_switch(
    owner: &Owner,
    runtime: SharedRuntime,
    parent: RuntimeNodeId,
    condition: impl Fn() -> bool + 'static,
    mount: impl Fn(&SharedRuntime, &Owner, bool) -> Option<RuntimeNodeId> + 'static,
) -> RuntimeNodeId {
    let scope = owner.child();
    let region = ChildRegion::new(runtime.clone(), parent);
    let anchor = region.anchor();
    let branch = Rc::new(RefCell::new(BranchBinding {
        region,
        variant: None,
        active: None,
    }));
    scope.on_cleanup({
        let branch = branch.clone();
        move || branch.borrow_mut().region.clear()
    });
    let container = scope.downgrade();
    scope.effect(move || {
        let value = condition();
        untrack(|| {
            let Some(container) = container.upgrade() else {
                return;
            };
            {
                let branch = branch.borrow();
                if !branch.region.is_live() || branch.variant == Some(value) {
                    return;
                }
            }
            let previous = branch.borrow_mut().active.take();
            if let Some(previous) = previous {
                previous.dispose();
            }
            if container.is_disposed() {
                return;
            }
            branch.borrow_mut().region.remove_roots();
            let active = container.child();
            let root = mount(&runtime, &active, value);
            if container.is_disposed() || !branch.borrow().region.is_live() {
                active.dispose();
                runtime.transaction(|tx| tx.remove_subtrees(&root.into_iter().collect::<Vec<_>>()));
                return;
            }
            let mut branch = branch.borrow_mut();
            branch.region.replace(root.into_iter().collect());
            branch.active = Some(active);
            branch.variant = Some(value);
        });
    });
    anchor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{create_binding, NodeKind, Runtime};
    use std::cell::Cell;

    fn leaf(runtime: &SharedRuntime, _owner: &Owner) -> RuntimeNodeId {
        runtime.transaction(|tx| tx.create_node(NodeKind::Container))
    }

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
        create_branch(&owner, runtime.clone(), parent, condition.clone(), leaf);

        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 1);

        condition.set(true);
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 2);

        condition.set(false);
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 1);
    }

    #[test]
    fn a_mount_closure_that_recurses_into_another_mount_does_not_panic() {
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
            condition,
            |runtime, owner| {
                let child = leaf(runtime, owner);
                let grandchild = leaf(runtime, owner);
                runtime.transaction(|tx| tx.insert_child(child, grandchild, None));
                child
            },
        );

        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 2);
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
            move |runtime, branch_owner| {
                create_binding(branch_owner, inner_runtime.clone(), {
                    let inner_signal = inner_signal.clone();
                    let inner_runs = inner_runs.clone();
                    move |_| {
                        let _ = inner_signal.get();
                        inner_runs.set(inner_runs.get() + 1);
                    }
                });
                leaf(runtime, branch_owner)
            }
        });
        assert_eq!(inner_runs.get(), 1);

        condition.set(false);
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 1);

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
        create_branch(&owner, runtime.clone(), parent, condition.clone(), leaf);
        assert_eq!(runtime.with(|r| r.get(parent).unwrap().children.len()), 2);

        owner.dispose();
        condition.set(false);
        assert!(runtime.with(|r| r.get(parent).unwrap().children.is_empty()));
    }
}
