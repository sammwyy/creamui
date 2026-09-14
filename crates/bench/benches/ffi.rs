//! Overhead of ABI-v2's `cui_*` handle+mutation entry points
//! (`crates/ffi/src/runtime.rs`) over calling `creamui_core::runtime`
//! directly. Mutates one already-mounted node repeatedly rather than
//! building a tree, so the number isolates marshaling cost instead of
//! `insert_child`'s own per-call taffy child-list rebuild.

use creamui_abi::CUI_NODE_KIND_TEXT;
use creamui_core::runtime::{Mutation, NodeKind, Runtime};
use creamui_ffi::runtime::{cui_create_node, cui_runtime_free, cui_runtime_new, cui_set_text};
use criterion::{criterion_group, criterion_main, Criterion};
use std::ffi::CString;

fn bench_native_set_text(c: &mut Criterion) {
    let mut runtime = Runtime::new();
    let node = runtime
        .transaction()
        .create_node(NodeKind::Text(Default::default()));
    let mut tick = 0u32;
    c.bench_function("ffi/native_set_text", |b| {
        b.iter(|| {
            tick = tick.wrapping_add(1);
            runtime.transaction().apply(Mutation::SetText {
                node,
                text: format!("value {tick}").into(),
            });
        });
    });
}

fn bench_abi_v2_set_text(c: &mut Criterion) {
    unsafe {
        let rt = cui_runtime_new();
        let node = cui_create_node(rt, CUI_NODE_KIND_TEXT);
        let mut tick = 0u32;
        c.bench_function("ffi/abi_v2_set_text", |b| {
            b.iter(|| {
                tick = tick.wrapping_add(1);
                let text = CString::new(format!("value {tick}")).expect("no interior NUL");
                cui_set_text(rt, node, text.as_ptr());
            });
        });
        cui_runtime_free(rt);
    }
}

criterion_group!(benches, bench_native_set_text, bench_abi_v2_set_text);
criterion_main!(benches);
