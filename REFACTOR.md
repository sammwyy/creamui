# CreamUI High-Performance Architecture Refactor Plan

**Project:** [sammwyy/creamui](https://github.com/sammwyy/creamui)  
**Scope:** Core runtime, reactivity, layout, event routing, paint architecture, GPU rendering, text, JSX/macros, ABI compatibility, profiling, and large-tree scalability  
**Goal:** Move CreamUI from a retained-node / whole-view rebuild architecture into a truly incremental retained UI engine whose update cost is proportional to the amount of UI that changed.

---

## 1. Executive Summary

CreamUI already has several useful foundations:

- `Signal<T>` based reactive state.
- A persistent `TaffyTree`.
- Persistent Taffy `NodeId`s across rebuilds.
- A backend-independent `Painter` abstraction.
- `winit` window/event integration.
- `wgpu` presentation.
- JSX proc macros.
- Separate workspace crates for `core`, `reactive`, `render`, `widgets`, `macros`, `dynamic`, `abi`, `ffi`, etc.
- Existing experiments with cached layers, animation-only repaints, paint fingerprints, and damage rectangles.

The main performance problem is architectural rather than a single slow function.

Today the hot path is approximately:

```text
Signal write
  -> window-wide reactive effect reruns
  -> rebuild immutable Box<dyn Widget> tree
  -> recursively reconcile the whole tree
  -> call Taffy setters throughout the tree
  -> recompute layout from the root
  -> walk/paint the tree
  -> rasterize through tiny-skia on CPU
  -> upload the resulting pixels to the GPU
  -> present
```

This means CreamUI retains some expensive objects, but it still performs work at frame scope or tree scope for changes that should be node-local.

The desired end state is:

```text
Signal write
  -> one or more fine-grained bindings become dirty
  -> binding emits mutations against persistent RuntimeNode IDs
  -> mutations set precise dirty flags
  -> affected Taffy nodes are updated only when layout inputs changed
  -> layout recomputes using Taffy's own invalidation/cache behavior
  -> affected paint records are regenerated only when paint inputs changed
  -> compositor/GPU scene receives small updates
  -> only damaged output is redrawn/presented where possible
```

For a 10,000-node UI, changing the background color of one leaf should not rebuild, reconcile, lay out, or repaint 10,000 nodes.

This document proposes a staged refactor so CreamUI remains runnable after every major phase.

---

# 2. Current Architecture: Concrete Problems to Eliminate

This plan is based on the current repository architecture as of September 2026.

## 2.1 `Widget` is still fundamentally an ephemeral description

`crates/core/src/widget.rs` describes widgets as cheap immutable descriptions rebuilt on each reactive rerender.

The current contract exposes:

```rust
pub trait Widget {
    fn style(&self) -> Style;
    fn paint(&self, painter: &mut dyn Painter, rect: Rect);
    fn children(&mut self) -> Vec<BoxedWidget>;
    fn measure(&self) -> Option<MeasureFn>;
    // ...
}
```

This makes the natural unit of work a new Rust widget object, not a persistent UI node.

That is acceptable as a compatibility/front-end representation, but it should no longer be the engine's source of truth.

---

## 2.2 Reconciliation recursively touches the entire tree

`crates/core/src/scene.rs::reconcile` currently:

1. Calls `widget.children()`.
2. Reads `widget.style()`.
3. Reads `widget.measure()`.
4. Recurses over every child.
5. Calls `TaffyTree::set_style`.
6. Calls `TaffyTree::set_node_context`.
7. Reconstructs child-ID vectors.
8. Calls `TaffyTree::set_children`.

This happens even when the logical node survived and its layout-relevant state did not change.

Persistent `NodeId`s avoid deleting/recreating Taffy nodes, but they do not avoid walking the tree or invalidating Taffy's caches.

---

## 2.3 Reactivity is fine-grained internally but coarse-grained at the UI boundary

`creamui-reactive` correctly tracks `Signal` dependencies at the `EffectState` level.

However, the window runtime creates a root effect around the render/repaint closure.

Conceptually:

```text
any Signal read while building the window
    -> subscribes the window-wide effect
    -> reruns the window's UI builder
```

The reactive primitive is fine-grained, but its principal UI subscriber is too large.

The refactor must change the unit of subscription from "window rebuild" to "binding/component/runtime node update".

---

## 2.4 Layout cache is undermined by unconditional mutation

CreamUI already keeps one `TaffyTree` around, which is correct.

The problem is that retained Taffy nodes are repeatedly mutated even when values are unchanged.

The target behavior must be:

```rust
if old_layout_style != new_layout_style {
    taffy.set_style(node, new_layout_style)?;
}
```

not:

```rust
taffy.set_style(node, new_layout_style)?;
```

for every node on every reactive update.

The same rule applies to:

- child lists,
- measure functions / intrinsic measurement state,
- display mode,
- absolute positioning,
- overflow/clipping inputs,
- any future text-measurement context.

---

## 2.5 Paint and interaction extraction are coupled

`paint_instance` currently both:

- paints the node,
- resolves interaction state,
- builds hit regions,
- collects focusable regions,
- collects drag targets,
- calculates scroll overflow,
- stores cursor regions,
- stores hover callbacks,
- traverses children.

This makes "repaint" imply substantial scene reconstruction work.

In the target engine:

```text
layout geometry
event metadata
paint records
interaction state
compositor state
```

must be independently updateable artifacts.

---

## 2.6 The current GPU path is primarily GPU presentation, not GPU rasterization

The render crate explicitly states that tiny-skia rasterizes on CPU and the GPU uploads/composites the result.

This is a valid MVP architecture, but it makes large dynamic UIs expensive because CreamUI pays for:

- CPU rasterization,
- CPU framebuffer memory bandwidth,
- CPU -> GPU upload bandwidth,
- then GPU presentation.

The end-state GPU backend should consume UI primitives directly.

---

# 3. Architectural Target

The long-term architecture should be split into the following layers.

```text
Application / Components / JSX
            |
            v
+---------------------------+
| Reactive Ownership Layer  |
| Signal / Effect / Binding |
+---------------------------+
            |
            | Mutation
            v
+--------------------------------+
| Persistent Runtime Tree        |
| RuntimeNodeId                  |
| parent / children              |
| style                          |
| content                        |
| handlers                       |
| dirty flags                    |
+--------------------------------+
      |              |
      |              +--------------------+
      v                                   v
+-------------+                    +--------------+
| Layout Tree |                    | Event Index  |
|   Taffy     |                    | Hit testing  |
+-------------+                    +--------------+
      |
      v
+--------------------------------+
| Retained Paint Representation  |
| Paint fragments / display list |
+--------------------------------+
      |
      v
+--------------------------------+
| GPU Scene / Compositor         |
| quads / glyphs / images /      |
| clips / transforms / layers    |
+--------------------------------+
      |
      v
+-------------+
| wgpu Surface|
+-------------+
```

The primary rule is:

> A stage must not run merely because a previous stage ran. It runs only if its inputs changed.

Examples:

```text
background color:
    reactive -> node mutation -> PAINT dirty
    no layout

width:
    reactive -> node mutation -> LAYOUT + PAINT dirty

text content:
    reactive -> content mutation
             -> TEXT_SHAPE/MEASURE dirty
             -> LAYOUT only where geometry depends on it
             -> PAINT dirty

hover:
    event routing -> old hovered node / new hovered node
                  -> STYLE_STATE/PAINT dirty for at most those nodes

translate transform:
    COMPOSITE dirty only

opacity:
    ideally COMPOSITE dirty only
```

---

# 4. Dirty-State Model

Introduce this before the large runtime migration because the rest of the architecture depends on precise invalidation.

A possible representation:

```rust
bitflags::bitflags! {
    pub struct DirtyFlags: u16 {
        const NONE        = 0;
        const STRUCTURE   = 1 << 0;
        const STYLE       = 1 << 1;
        const MEASURE     = 1 << 2;
        const LAYOUT      = 1 << 3;
        const PAINT       = 1 << 4;
        const HIT_TEST    = 1 << 5;
        const COMPOSITE   = 1 << 6;
        const ACCESS      = 1 << 7;
    }
}
```

Do not treat these as synonyms.

A mutation classifier must decide exactly what downstream work is necessary.

Example:

```rust
fn invalidation_for_style_diff(
    old: &ComputedStyle,
    new: &ComputedStyle,
) -> DirtyFlags;
```

Eventually this should compare style groups rather than the entire `Style` struct.

Suggested split:

```rust
struct LayoutStyle { ... }
struct PaintStyle { ... }
struct TextStyle { ... }
struct InteractionStyle { ... }
struct CompositeStyle { ... }
```

This avoids treating a background change as a layout change just because both fields currently live inside one `Style`.

---

# 5. Performance Contracts

Before refactoring, define concrete contracts.

These should become benchmark assertions where practical.

## 5.1 Static tree

For a static 10,000-node tree after initial mount:

```text
root rebuilds per idle frame:       0
nodes reconciled per idle frame:    0
Taffy style writes:                 0
Taffy child-list writes:            0
text measurements:                  0
paint-record rebuilds:              0
CPU raster passes:                  0
```

If no animation or window damage is active, CreamUI should not request continuous redraws.

---

## 5.2 Single paint-only leaf update

For a background-color signal changing on one leaf in a 10,000-node tree:

```text
component/root rebuilds:            0
structure diffs:                    0
layout invalidations:               0
Taffy writes:                       0
paint-record rebuilds:              1
affected nodes:                     O(1)
```

GPU work may still traverse batches depending on renderer implementation, but CPU work must not scale with tree size.

---

## 5.3 Single text update

For changing text in one leaf:

```text
root rebuilds:                      0
text shaping/measurement:           1 logical run
layout dirtied:                     leaf + required ancestor chain
unrelated Taffy branches touched:   0
paint rebuild:                      text node
```

---

## 5.4 Hover transition

Moving the pointer within the same hovered widget:

```text
hover state changes:                0
paint invalidations:                0
layout:                             0
```

Moving from widget A to widget B:

```text
hover state changes:                A -> false, B -> true
paint/style invalidations:          <= 2 nodes plus explicit dependencies
layout:                             0 unless hover style changes layout
```

Layout-changing hover styles should be supported but treated as the exceptional slow path.

---

## 5.5 Scrolling

Normal scrolling should not rebuild child widgets or rerun Taffy layout.

Desired steady-state:

```text
scroll offset mutation
  -> transform/composite update
  -> clip remains retained
  -> hit-test transform updated
  -> visible paint/raster only if newly exposed content requires it
```

---

# 6. Phase 0 — Build a Performance Laboratory Before Refactoring

## Objective

Make current behavior measurable so every later phase has hard evidence.

Do not begin the large architectural rewrite without this phase.

## 6.1 Add a dedicated benchmark crate

Create:

```text
crates/bench/
    Cargo.toml
    src/
        lib.rs
        scenes.rs
        metrics.rs
    benches/
        tree_update.rs
        text_update.rs
        hover.rs
        scroll.rs
        layout.rs
        paint.rs
```

Or use an `examples/perf-*` family for interactive GPU benchmarks plus Criterion for CPU-only paths.

Recommended split:

```text
Criterion:
    pure runtime/layout/diff measurements

interactive perf harness:
    full winit/wgpu frame behavior
```

---

## 6.2 Add synthetic benchmark scenes

Implement reproducible scenes:

### `deep_tree`

```text
depth = 1000
one child per node
```

Tests ancestor invalidation behavior.

### `wide_tree`

```text
root
  10,000 leaf children
```

Tests linear scans and event-region behavior.

### `grid_10k`

A realistic nested Flex/Grid tree with roughly 10,000 nodes.

### `dashboard`

Mix:

- sidebar,
- toolbar,
- cards,
- labels,
- buttons,
- inputs,
- scroll views.

This should represent the desktop-environment use case CreamUI is moving toward.

### `chat_10k`

Long scrollable message list.

### `animated_cards`

Many static nodes plus 1, 10, 100 animated nodes.

---

## 6.3 Introduce engine counters

Create something like:

```rust
pub struct FrameMetrics {
    pub root_builds: u64,
    pub widget_objects_built: u64,
    pub reconcile_visits: u64,

    pub taffy_style_writes: u64,
    pub taffy_context_writes: u64,
    pub taffy_children_writes: u64,
    pub layout_runs: u64,
    pub measure_calls: u64,

    pub paint_nodes_visited: u64,
    pub paint_nodes_recorded: u64,
    pub hit_nodes_updated: u64,

    pub cpu_pixels_rasterized: u64,
    pub gpu_upload_bytes: u64,
    pub draw_calls: u64,

    pub damaged_rect_count: u64,
    pub damaged_pixel_area: u64,
}
```

Keep metrics compiled behind:

```text
feature = "perf-metrics"
```

or make counters extremely cheap in release.

---

## 6.4 Add timing spans

Instrument:

```text
reactive effects
UI build
reconciliation
Taffy mutation
Taffy compute_layout
event-index rebuild
paint traversal
paint recording
CPU raster
GPU upload
GPU render encoding
present
```

Use either:

- `tracing`,
- a minimal internal timing API,
- Tracy integration as an optional feature,
- or all three eventually.

Avoid `println!` profiling.

---

## 6.5 Record baseline numbers

Create:

```text
docs/performance/baseline.md
```

Record:

- machine,
- OS,
- GPU,
- release profile,
- resolution,
- DPI scale,
- backend,
- node count,
- update kind,
- CPU frame time,
- GPU time where available,
- memory,
- upload bytes.

The baseline becomes the regression reference.

---

## 6.6 Phase 0 exit criteria

- [ ] Reproducible 1k / 10k / 50k node benchmark scenes exist.
- [ ] CPU stage timings are visible.
- [ ] Node-visit counters exist.
- [ ] Taffy mutation counters exist.
- [ ] GPU upload byte count is measurable.
- [ ] Hover and scroll benchmarks exist.
- [ ] Baseline numbers are committed.

---

# 7. Phase 1 — Stop Performing Provably Unnecessary Work in the Current Architecture

## Objective

Extract performance from the existing architecture before replacing it.

This phase is intentionally conservative. It gives immediate improvements and validates assumptions.

---

## 7.1 Stop unconditional Taffy style writes

Current reconciliation stores:

```rust
style: crate::Style
```

inside `Instance`.

Use it.

Before calling `set_style`:

```rust
let new_layout_style = constrain_inflow(new_style.layout.clone());

if old.style.layout != new_layout_style {
    tree.set_style(old.node_id, new_layout_style.clone())?;
    metrics.taffy_style_writes += 1;
}
```

You may need to store the constrained layout style separately so comparison matches exactly what Taffy receives:

```rust
struct Instance {
    declared_style: Style,
    taffy_style: taffy::Style,
    // ...
}
```

Do not compare a pre-normalized style with a post-normalized one.

---

## 7.2 Stop unconditional child-list writes

Current code creates:

```rust
let child_ids: Vec<_> = ...
tree.set_children(old.node_id, &child_ids)
```

every time.

Instead retain previous child IDs or compare against `old.children`.

Only call Taffy when:

```text
length changed
identity changed
order changed
```

Temporary implementation:

```rust
let children_changed =
    old_child_ids.len() != new_child_ids.len()
    || old_child_ids.iter().zip(&new_child_ids).any(|(a, b)| a != b);

if children_changed {
    tree.set_children(...)?;
}
```

This remains O(children), but avoids Taffy invalidation.

Later phases remove the full scan.

---

## 7.3 Replace `MeasureFn` closure identity problems

`MeasureFn` is currently a boxed closure and is difficult to compare semantically.

Introduce a stable measurement identity.

Possible short-term API:

```rust
pub struct MeasureSpec {
    pub fingerprint: u64,
    pub func: MeasureFn,
}
```

or:

```rust
trait Measure {
    fn measure(...);
    fn fingerprint(&self) -> u64;
}
```

For text:

```text
hash(
    text,
    font face,
    font size,
    weight,
    wrap mode,
    letter spacing,
    relevant width constraints
)
```

Only update Taffy's node context when this fingerprint changed.

---

## 7.4 Add structural identity before full retained-tree migration

Current reconciliation is position-based.

Add an optional key:

```rust
pub enum WidgetKey {
    U64(u64),
    String(Arc<str>),
}
```

to the compatibility widget representation.

Then reconcile keyed children with a local map when a parent opts into keys.

Do not overengineer this phase into the final runtime.

The purpose is:

- avoid pathological reorder behavior now,
- define semantics that the later retained runtime will preserve.

---

## 7.5 Fix hover repaint semantics

Current pointer movement can request `repaint_light()` even when visual hover state did not change.

Change event routing to cache:

```rust
struct HoverTarget {
    node_or_region_id: ...,
    rect: Rect,
    callback: ...
}
```

On `CursorMoved`:

1. hit-test at new position,
2. determine new hover target,
3. compare to previous target,
4. if identical:
   - update pointer position,
   - do not repaint,
5. if changed:
   - call old leave callback,
   - call new enter callback,
   - invalidate only interaction paint.

Short-term compatibility path may still repaint the full scene when hover changes, but **no change must mean no repaint**.

---

## 7.6 Avoid full frame work for caret blink

Caret blink should not rebuild the widget tree and should not trigger layout.

It should become:

```text
focused text node overlay dirty
```

Short-term:

- use the existing light repaint path,
- restrict damage to the focused control if possible.

Long-term this disappears into retained paint/composite state.

---

## 7.7 Add no-op signal writes where appropriate

Current `Signal::set` always notifies.

Add:

```rust
impl<T: Clone + PartialEq + 'static> Signal<T> {
    pub fn set_if_changed(&self, value: T);
}
```

Do **not** silently change `set` semantics globally if callers may depend on explicit notification.

For internal controls, prefer `set_if_changed`.

---

## 7.8 Phase 1 exit criteria

On the current engine:

- [ ] Updating one unchanged UI state causes zero Taffy writes.
- [ ] Stable child lists do not call `set_children`.
- [ ] Moving the pointer inside one hover region causes no repaint.
- [ ] Keyed reorder behavior exists for large lists.
- [ ] Metrics prove fewer invalidations.
- [ ] No public API break is required yet.

---

# 8. Phase 2 — Introduce the Persistent Runtime Tree

## Objective

Create the structure that will eventually replace `Instance + BoxedWidget` as the engine's source of truth.

Do not immediately delete the old rendering path.

Build the new runtime beside it and mount old widgets through an adapter.

---

## 8.1 Create a dedicated runtime module

Recommended location:

```text
crates/core/src/runtime/
    mod.rs
    node.rs
    arena.rs
    mutation.rs
    dirty.rs
    mount.rs
    transaction.rs
```

If `core` becomes too broad, eventually split:

```text
crates/runtime
```

but do not create extra crates until module boundaries stabilize.

---

## 8.2 Define stable runtime node identity

Use generational IDs.

Example:

```rust
pub struct RuntimeNodeId {
    index: u32,
    generation: u32,
}
```

Back with:

- `slotmap`,
- `generational-arena`,
- or a small custom arena.

Requirements:

- stale IDs must be detectable,
- IDs must be cheap to copy,
- removing a subtree must invalidate old IDs,
- IDs should not expose pointers.

---

## 8.3 Define `RuntimeNode`

Possible initial shape:

```rust
pub struct RuntimeNode {
    pub id: RuntimeNodeId,

    pub parent: Option<RuntimeNodeId>,
    pub first_child: Option<RuntimeNodeId>,
    pub last_child: Option<RuntimeNodeId>,
    pub prev_sibling: Option<RuntimeNodeId>,
    pub next_sibling: Option<RuntimeNodeId>,

    pub kind: NodeKind,

    pub style: ComputedStyle,
    pub state: InteractionState,

    pub layout: LayoutState,
    pub paint: PaintState,
    pub events: EventState,

    pub dirty: DirtyFlags,

    pub owner: OwnerId,
}
```

Prefer linked child relationships or an arena-owned small vector depending on benchmark results.

Do not blindly store `Vec<RuntimeNodeId>` on every leaf if most nodes have zero or one child.

Possible representation:

```rust
enum Children {
    None,
    One(RuntimeNodeId),
    Many(SmallVec<[RuntimeNodeId; 4]>),
}
```

Benchmark before committing.

---

## 8.4 Define `NodeKind`

Avoid storing arbitrary `Box<dyn Widget>` as the final node payload.

Start with an enum for core primitives:

```rust
pub enum NodeKind {
    Container,
    Text(TextNode),
    Image(ImageNode),
    Custom(CustomNode),
}
```

Most high-level widgets should lower to these primitives.

Buttons, tabs, switches, sidebars, etc. should ideally be component-level structures composed from primitive runtime nodes rather than renderer-specific node kinds.

---

## 8.5 Separate declarative component type from runtime node type

A `Button` should remain a convenient public component.

But it should mount into primitives:

```text
Button
  -> Container node
      -> Text node
```

rather than requiring the renderer to permanently understand a `Button`.

This reduces renderer complexity and creates a browser-like separation:

```text
component model != render primitive model
```

---

## 8.6 Introduce mutation operations

Create a small, explicit mutation vocabulary.

Example:

```rust
pub enum Mutation {
    CreateNode {
        id: RuntimeNodeId,
        kind: NodeKind,
    },

    RemoveSubtree {
        root: RuntimeNodeId,
    },

    InsertBefore {
        parent: RuntimeNodeId,
        child: RuntimeNodeId,
        before: Option<RuntimeNodeId>,
    },

    SetLayoutStyle {
        node: RuntimeNodeId,
        style: LayoutStyle,
    },

    SetPaintStyle {
        node: RuntimeNodeId,
        style: PaintStyle,
    },

    SetText {
        node: RuntimeNodeId,
        text: Arc<str>,
    },

    SetEventHandlers {
        node: RuntimeNodeId,
        handlers: EventHandlers,
    },

    SetTransform {
        node: RuntimeNodeId,
        transform: Transform2D,
    },

    SetOpacity {
        node: RuntimeNodeId,
        opacity: f32,
    },
}
```

The exact enum can evolve, but mutation categories must preserve invalidation precision.

---

## 8.7 Apply mutations transactionally

Create:

```rust
pub struct RuntimeTransaction<'a> {
    runtime: &'a mut Runtime,
    touched: SmallVec<[RuntimeNodeId; 16]>,
}
```

Multiple changes in one event should become one commit.

Example:

```text
mouse drag
    set slider value
    update text label
    move thumb transform

-> one runtime transaction
-> dirty sets merged
-> one redraw request
```

---

## 8.8 Keep old `Widget` API through a compatibility mount adapter

Add:

```rust
fn mount_legacy_widget(
    widget: BoxedWidget,
    runtime: &mut Runtime,
    parent: RuntimeNodeId,
) -> RuntimeNodeId;
```

Initially this can translate an entire old widget subtree.

This allows:

- existing examples to work,
- widgets to migrate gradually,
- new retained primitives to coexist with legacy components.

Mark this path with metrics so you know when old full rebuild behavior remains.

---

## 8.9 Runtime invariants

Add debug assertions for:

```text
parent/child links are consistent
no node has two parents
removed generations are invalid
every layout node maps to a valid runtime node
owner subtree relationships are valid
dirty flags never reference deleted nodes
```

Add randomized mutation tests.

---

## 8.10 Phase 2 exit criteria

- [ ] Persistent `RuntimeNodeId` arena exists.
- [ ] Nodes survive across frames independently from `BoxedWidget`.
- [ ] Explicit mutation API exists.
- [ ] Dirty flags are stored per node.
- [ ] Existing widget trees can mount through a compatibility adapter.
- [ ] Runtime mutation tests exist.
- [ ] Renderer can inspect runtime nodes without asking widgets to rebuild themselves.

---

# 9. Phase 3 — Replace Window-Wide Reactivity with Reactive Ownership and Fine-Grained Bindings

## Objective

A `Signal` change must be able to mutate a specific runtime node without rebuilding the root component.

This is the highest-impact architectural phase.

---

## 9.1 Add reactive ownership

The current `EffectState` lifecycle is only tied to the returned `Effect` handle.

UI trees need ownership.

Introduce:

```rust
pub struct OwnerId(...);

pub struct ReactiveOwner {
    parent: Option<OwnerId>,
    children: Vec<OwnerId>,
    effects: Vec<EffectId>,
    cleanups: Vec<Box<dyn FnOnce()>>,
}
```

When a component/subtree is removed:

```text
remove RuntimeNode subtree
    -> dispose Owner subtree
    -> unsubscribe all effects
    -> release event closures
    -> release resource subscriptions
```

This is essential for dynamic branches and lists.

---

## 9.2 Separate "effect" from "UI rebuild"

The generic `create_effect` can remain.

But UI bindings should use a scheduler-aware form:

```rust
create_binding(owner, move |cx| {
    let value = signal.get();
    cx.set_text(node, value.to_string());
});
```

The binding should enqueue a mutation or directly update a transaction-safe runtime API.

It must not call `build_ui()`.

---

## 9.3 Introduce a scheduler

Current effects execute synchronously on notification unless batched.

For UI, create a scheduler that can coalesce mutations during one event-loop turn.

Possible model:

```rust
enum ReactivePriority {
    Immediate,
    UserInput,
    Animation,
    Normal,
}
```

Do not build a complex React-style concurrent scheduler yet.

The first scheduler only needs to guarantee:

1. signal writes enqueue dirty effects,
2. duplicate effects are deduplicated,
3. all effects flush before frame preparation,
4. runtime mutations are batched,
5. at most one redraw request is issued per window for the batch.

---

## 9.4 Window runtime becomes an owner, not a subscriber to all UI state

Replace the current conceptual model:

```rust
create_effect(move || {
    rebuild_whole_window();
});
```

with:

```text
WindowOwner
   |
   +-- ComponentOwner A
   |      +-- bindings...
   |
   +-- ComponentOwner B
          +-- bindings...
```

The window should subscribe only to runtime-level "frame needed" notifications.

---

## 9.5 Support structural effects

Some reactive expressions change tree shape:

```rust
if show_panel.get() {
    <Panel />
}
```

This should not rebuild the whole window.

Represent it as a structural binding anchored at a stable location:

```rust
struct BranchBinding {
    marker: RuntimeNodeId,
    active_owner: Option<OwnerId>,
    active_root: Option<RuntimeNodeId>,
}
```

When the condition changes:

```text
false -> true:
    mount branch subtree at marker

true -> false:
    remove branch subtree
    dispose branch owner
```

Only that branch changes.

---

## 9.6 Support keyed list ownership

Design keyed lists early.

Example:

```rust
for_each(items, key = |item| item.id, render = ...)
```

Maintain:

```rust
HashMap<Key, ListEntry> {
    owner,
    root_node,
    previous_index,
}
```

On update:

```text
retain matching keys
move existing runtime nodes
mount new keys
dispose removed keys
```

Do not identify list items by index.

---

## 9.7 Avoid accidental subscriptions during internal reads

Continue using a `peek` equivalent for runtime code.

Rules:

```text
component/binding expression:
    get()

renderer/layout/event internal logic:
    never get reactive signals directly
```

Signals should update runtime state before layout/render begins.

The renderer should operate on plain retained data, not reactive values.

This is crucial because it makes rendering deterministic and prevents renderer traversal from becoming part of dependency tracking.

---

## 9.8 Phase 3 exit criteria

- [ ] No window-wide effect subscribes to every signal read by the UI.
- [ ] A leaf signal can update one runtime node directly.
- [ ] Dynamic branches own/dispose their subscriptions.
- [ ] Keyed list entries preserve node identity.
- [ ] Multiple signal writes during one input event coalesce into one frame request.
- [ ] Removing a subtree leaves no signal subscribers behind.
- [ ] Benchmarks show update cost no longer scales with unrelated tree size.

---

# 10. Phase 4 — Rework JSX and Component Compilation Around Mount + Bind Semantics

## Objective

Preserve CreamUI's ergonomic JSX while changing what it generates.

Today JSX primarily expands to ordinary Rust widget builders.

The target JSX should generate persistent mounts plus reactive bindings.

---

## 10.1 Do not turn JSX into a virtual DOM

Avoid:

```text
JSX expression
   -> build VNode tree
   -> diff VNode tree every update
```

That would replace the current widget-tree rebuild with another tree rebuild.

Instead, compile static structure once.

Conceptually:

```rust
jsx! {
    <Flex gap={12.0}>
        <Text>{counter.get()}</Text>
    </Flex>
}
```

should become approximately:

```rust
let flex = cx.create_container();
cx.set_gap(flex, 12.0);

let text = cx.create_text("");
cx.append(flex, text);

cx.bind(move |cx| {
    cx.set_text(text, counter.get().to_string());
});

flex
```

Static JSX structure should not be regenerated when `counter` changes.

---

## 10.2 Introduce `MountCx`

Suggested API:

```rust
pub struct MountCx<'a> {
    runtime: &'a mut Runtime,
    owner: OwnerId,
    parent: RuntimeNodeId,
}
```

Operations:

```rust
cx.container(...)
cx.text(...)
cx.image(...)
cx.append(...)
cx.bind(...)
cx.branch(...)
cx.keyed(...)
cx.on_cleanup(...)
```

The proc macro can compile into these operations.

---

## 10.3 Categorize JSX expressions

The macro must understand several cases.

### Static expression

```rust
<Text font_size={14.0}>
```

Mount once.

### Reactive value expression

```rust
<Text>{counter.get()}</Text>
```

Create a binding.

### Event closure

```rust
<Button on_click={move || ...}>
```

Store handler in runtime event metadata.

### Conditional subtree

```rust
{if open.get() { jsx! { <Panel /> } } else { ... }}
```

Long-term macro support should lower this into branch ownership.

### Keyed iterable

Prefer an explicit helper/component initially rather than trying to infer arbitrary Rust iterators.

---

## 10.4 Preserve normal Rust components

`#[component]` should become a mount function rather than a function returning an ephemeral `BoxedWidget`.

Possible generated internal form:

```rust
fn MyComponent(
    cx: &mut MountCx,
    props: MyComponentProps,
) -> MountedView;
```

Public syntax can remain:

```rust
#[component]
fn MyComponent(props...) {
    jsx! { ... }
}
```

The proc macro can rewrite the signature internally.

---

## 10.5 Introduce a compatibility return type

During migration:

```rust
pub enum View {
    Mounted(MountedView),
    Legacy(BoxedWidget),
}
```

or equivalent trait-based conversion:

```rust
trait IntoView {
    fn mount(self, cx: &mut MountCx) -> MountedView;
}
```

This lets existing custom components keep working while migrated components become retained.

---

## 10.6 Do not force every prop to be reactive

Use explicit static/dynamic forms internally:

```rust
pub enum Prop<T> {
    Static(T),
    Dynamic(Binding<T>),
}
```

The macro can usually infer whether an expression is compiled as a one-shot value or a dependency-tracked closure.

Be careful: ordinary Rust expressions may read a `Signal` indirectly through helper functions.

Therefore the safest semantics may be:

- evaluate props during the binding setup,
- dependency tracking discovers signals dynamically,
- after first run, the binding owns the dependencies.

That keeps the current reactive model's useful dynamic dependency behavior.

---

## 10.7 Macro debugging support

Add an optional macro expansion/debug mode.

Useful outputs:

```text
CREAMUI_DUMP_JSX=1
```

or a devtools command that shows:

```text
component
owner ID
runtime node IDs
bindings
signals/dependencies
```

Fine-grained systems become difficult to debug without ownership visibility.

---

## 10.8 Phase 4 exit criteria

- [ ] Native JSX no longer requires whole-tree recreation for leaf expressions.
- [ ] Static JSX nodes mount once.
- [ ] Dynamic props create bindings.
- [ ] Conditional subtrees have ownership/disposal.
- [ ] Keyed lists have a first-class retained path.
- [ ] Existing `#[component]` ergonomics remain recognizable.
- [ ] Legacy widget-returning components still have an adapter during migration.

---

# 11. Phase 5 — Make Taffy a Truly Incremental Layout Backend

## Objective

Taffy should receive only real layout mutations.

The runtime tree becomes the source of truth; Taffy becomes a derived layout structure.

---

## 11.1 Add explicit runtime-to-Taffy mapping

Per runtime node:

```rust
pub struct LayoutState {
    pub taffy_node: Option<taffy::NodeId>,
    pub style_hash: u64,
    pub measure_key: Option<MeasureKey>,
    pub layout_rect: Rect,
    pub previous_layout_rect: Rect,
    pub epoch: u64,
}
```

Not every logical runtime node necessarily needs a Taffy node forever.

For example, future non-layout wrapper/component nodes may be flattened.

---

## 11.2 Mutate Taffy when applying runtime mutations

Do not wait for frame render to rescan the runtime tree.

Example:

```text
Mutation::SetLayoutStyle
    -> compare old/new
    -> Taffy set_style immediately or enqueue exact layout op
    -> mark LAYOUT
```

`Mutation::SetPaintStyle` must not touch Taffy.

---

## 11.3 Separate layout invalidation from layout execution

A mutation can mark layout dirty without immediately computing it.

At frame prepare:

```rust
if runtime.layout_dirty() {
    layout_engine.compute(...);
}
```

If no layout inputs changed:

```text
skip Taffy compute_layout completely
```

---

## 11.4 Cache intrinsic measurement

Text/image measurement often becomes a hidden hotspot.

Introduce:

```rust
struct MeasureCacheKey {
    content_id: ...,
    constraint_width: QuantizedF32,
    constraint_height: QuantizedF32,
    font_epoch: u64,
    scale_factor: ...,
}
```

Cache:

```rust
Size<f32>
```

Text measurement should eventually share results with the text shaping cache so measurement and painting do not shape the same string independently.

---

## 11.5 Track layout geometry changes after Taffy runs

Even when layout recomputes, not every node necessarily moves.

After layout:

```rust
if new_rect != old_rect {
    node.dirty |= PAINT | HIT_TEST;
    damage.add(old_rect);
    damage.add(new_rect);
}
```

If the renderer later supports transform-only repositioning for a subtree, some geometry changes can become compositor updates instead.

Do not optimize that prematurely.

---

## 11.6 Introduce layout epochs

Instead of clearing flags tree-wide:

```rust
runtime.layout_epoch += 1;
node.last_layout_epoch = runtime.layout_epoch;
```

Use epochs for devtools and debugging:

```text
"why was this node laid out?"
```

Store an optional invalidation reason in debug builds.

---

## 11.7 Layout benchmarks

Required tests:

```text
paint-only update:
    layout runs = 0

one leaf fixed-width text color update:
    layout runs = 0

one leaf text-content update:
    measurement only where needed

resize root viewport:
    expected broad layout recompute

move keyed item without style changes:
    child structure mutation only in affected parent
```

---

## 11.8 Phase 5 exit criteria

- [ ] Paint-only changes never call Taffy.
- [ ] Runtime mutations update Taffy precisely.
- [ ] Stable node contexts are not rewritten.
- [ ] Measurement is cached.
- [ ] Layout geometry changes produce targeted paint/hit-test invalidation.
- [ ] Layout metrics scale with affected branches rather than whole-tree mutation count.

---

# 12. Phase 6 — Decouple Event Routing and Hit Testing from Painting

## Objective

Pointer movement, focus, dragging, scrolling, and keyboard navigation must not require rebuilding interaction regions by repainting the tree.

---

## 12.1 Move event handlers into retained node state

Instead of extracting callbacks during paint:

```rust
pub struct EventState {
    pub on_click: Option<HandlerId>,
    pub on_click_at: Option<HandlerId>,
    pub on_hover: Option<HandlerId>,
    pub on_key: Option<HandlerId>,
    pub on_drag: Option<HandlerId>,
    pub on_drag_start: Option<HandlerId>,
    pub on_drag_end: Option<HandlerId>,
    pub on_scroll: Option<HandlerId>,
    pub cursor: Option<CursorIcon>,
    pub focusable: bool,
    pub pointer_events: PointerEvents,
}
```

Handlers can live in a separate arena so runtime nodes store compact IDs.

---

## 12.2 Maintain paint-order / z-order explicitly

Hit testing must know visual order independently from paint traversal.

Create retained ordering metadata:

```rust
paint_order: u64
stacking_context: StackingContextId
```

Do not rely on position inside temporary `Scene` vectors.

---

## 12.3 Start with a simple retained hit-test list

Do not begin with an R-tree unless profiling requires it.

First architecture:

```text
Vec<HitEntry>
```

where entries update only when:

- node geometry changes,
- visibility changes,
- z-order changes,
- handler/pointer-events state changes.

Pointer move can scan in reverse paint order.

For 10k interactive nodes this may already be sufficient.

---

## 12.4 Add a spatial index only when benchmarked necessary

Possible later structures:

- R-tree,
- BVH,
- quadtree,
- tile buckets.

A simple tile bucket grid may be ideal for desktop UI:

```text
window divided into 64x64 or 128x128 logical-pixel cells
each cell stores candidate node IDs
```

Updates remain cheap and pointer hit testing examines a small candidate set.

Benchmark before selecting.

---

## 12.5 Hover target becomes a node ID

Store:

```rust
hovered: Option<RuntimeNodeId>
pressed: Option<RuntimeNodeId>
focused: Option<RuntimeNodeId>
pointer_capture: Option<RuntimeNodeId>
```

Then pseudo-style invalidation is trivial.

Example:

```text
old hover != new hover
    -> invalidate state style on old
    -> invalidate state style on new
```

---

## 12.6 Add pointer capture

Dragging should not repeatedly search for the drag target.

On drag begin:

```text
pointer_capture = Some(node)
```

Subsequent move/up events go directly to that node until released.

This is more browser-like and makes sliders/text selection simpler.

---

## 12.7 Focus order

Replace focus indices that are only stable while scene shape stays unchanged.

Store focus by `RuntimeNodeId`.

Maintain a retained focus traversal list that is updated on structural/focusable changes.

This prevents focus identity from shifting because unrelated nodes were inserted before it.

---

## 12.8 Scroll routing

Route wheel events through the ancestor chain:

```text
hit leaf
   -> nearest scrollable ancestor
   -> attempt consume
   -> bubble remainder if desired
```

This becomes much cleaner once parent links are persistent.

---

## 12.9 Phase 6 exit criteria

- [ ] Paint no longer generates the authoritative event scene.
- [ ] Focus is node-ID based.
- [ ] Hover is node-ID based.
- [ ] Mouse movement with no hover transition performs zero painting.
- [ ] Dragging supports pointer capture.
- [ ] Event handlers survive paint skips.
- [ ] Hit-test geometry updates only when relevant nodes move/change.

---

# 13. Phase 7 — Introduce a Retained Paint Representation

## Objective

Stop asking every widget to issue drawing commands on every normal frame.

A node should have retained paint output until something affecting its pixels changes.

---

## 13.1 Introduce backend-independent paint primitives

Create a low-level primitive enum.

Example:

```rust
pub enum PaintPrimitive {
    Quad(QuadPrimitive),
    Border(BorderPrimitive),
    GlyphRun(GlyphRunPrimitive),
    Image(ImagePrimitive),
    Path(PathPrimitive),
    Shadow(ShadowPrimitive),
}
```

Also represent state operations:

```rust
pub enum PaintOp {
    PushClip(ClipId),
    PopClip,
    PushTransform(Transform2D),
    PopTransform,
    Primitive(PaintPrimitive),
}
```

Prefer structured scene data over a purely imperative painter API.

---

## 13.2 Migrate `Widget::paint` to `PaintCx` recording

Compatibility path:

```rust
widget.paint(&mut RecordingPainter, rect);
```

where `RecordingPainter` records primitives instead of rasterizing immediately.

This is a powerful transition strategy because existing widgets do not need to migrate all at once.

New retained nodes can generate primitives directly.

---

## 13.3 Store paint fragments per runtime node

Example:

```rust
pub struct PaintState {
    pub fragment: PaintFragmentId,
    pub bounds: Rect,
    pub dirty: bool,
    pub fingerprint: u64,
}
```

A paint fragment is immutable until replaced.

On a paint-only mutation:

```text
regenerate that node's fragment
replace old fragment
damage old bounds + new bounds
```

Static siblings are untouched.

---

## 13.4 Handle subtree effects carefully

Some visual behavior depends on children:

- clipping,
- opacity groups,
- transforms,
- shadows,
- filters,
- scroll transforms.

Do not flatten everything into node-local pixels.

Represent grouping explicitly in the retained scene:

```text
StackingContext
ClipNode
TransformNode
OpacityNode
```

This begins to resemble browser property trees.

A full Chromium-style property tree implementation is not required immediately, but the data model should leave room for it.

---

## 13.5 Introduce damage tracking centrally

Create:

```rust
pub struct DamageRegion {
    rects: SmallVec<[Rect; 8]>,
}
```

On change:

```text
node paint content changed:
    add old paint bounds
    add new paint bounds

node moved:
    add old bounds
    add new bounds

node removed:
    add old bounds

node inserted:
    add new bounds
```

Merge overlapping rectangles.

At some threshold, collapse to full-window damage:

```text
if damaged_area > N% of viewport
or rect_count > limit
    -> full damage
```

Tune with benchmarks.

---

## 13.6 Paint order should be retained

Maintain a scene list or ordering tree.

A single leaf color change should replace one fragment without rebuilding order.

Structural changes update only the relevant range.

---

## 13.7 Phase 7 exit criteria

- [ ] Existing widget painters can record primitives instead of rasterizing directly.
- [ ] Paint fragments persist across frames.
- [ ] One leaf paint change regenerates one fragment.
- [ ] Damage regions include old and new visual bounds.
- [ ] Event extraction is no longer mixed with paint recording.
- [ ] Static UI can present new frames without rerunning widget paint code.

---

# 14. Phase 8 — Replace the CPU-Framebuffer GPU Path with a Real GPU Scene Renderer

## Objective

Make the GPU rasterize UI primitives instead of merely presenting a tiny-skia bitmap.

This phase should begin only after retained invalidation exists. Otherwise CreamUI will simply move an inefficient full-scene renderer from CPU to GPU.

---

## 14.1 Keep two backends during migration

Target:

```text
RenderBackend::Cpu
RenderBackend::GpuScene
```

Legacy transitional backend:

```text
RenderBackend::CpuRasterGpuPresent
```

Do not delete tiny-skia immediately.

It remains useful for:

- reference screenshots,
- headless tests,
- fallback,
- regression comparison.

---

## 14.2 Create a GPU scene module

Possible structure:

```text
crates/render/src/gpu_scene/
    mod.rs
    renderer.rs
    scene.rs
    buffers.rs
    quads.rs
    images.rs
    text.rs
    clips.rs
    paths.rs
    compositor.rs
```

If this becomes large, split later into `creamui-gpu`.

---

## 14.3 GPU primitive pipeline: rounded quads first

Most UI is rectangles.

Implement an instanced quad pipeline supporting:

```text
position
size
background color
per-corner radius or common radius
border width
border color
opacity
transform
clip reference
```

Use fragment-shader distance functions for rounded corners/borders where appropriate.

One logical control should usually become one or a few instances, not custom geometry.

---

## 14.4 Batch aggressively

Batch by:

```text
pipeline
texture/sampler requirements
clip strategy
blend mode
```

Do not submit one draw call per widget.

Desired common case:

```text
thousands of rectangles
    -> one/few buffer uploads
    -> one/few draw calls
```

---

## 14.5 Incremental GPU instance updates

Assign stable GPU scene IDs:

```rust
GpuPrimitiveId
```

Maintain ranges/slots in GPU buffers.

When one primitive changes:

```text
update one CPU-side instance struct
queue.write_buffer only affected range
```

Avoid rebuilding/reuploading a giant vertex buffer for one leaf.

Later, use staging/belt strategies if needed.

---

## 14.6 Images

Introduce a texture manager:

```rust
ImageId
TextureId
```

Decoded image data should upload once.

A frame should reference texture IDs, not resend RGBA data.

For many small images/icons, consider texture arrays or atlases after profiling.

---

## 14.7 Clip implementation

Start with:

```text
rectangular clips -> scissor where possible
rounded/nested clips -> stencil or clip-mask strategy
```

Do not route every clip through expensive offscreen textures.

Represent clip hierarchy separately so multiple primitives share a clip node.

---

## 14.8 Shadows

Do not make shadow implementation block the primary renderer.

Migration order:

1. solid quad,
2. border,
3. rounded corners,
4. images,
5. text,
6. simple shadows,
7. complex path/filter effects.

CPU fallback can temporarily handle unsupported primitives only if compositing that fallback does not recreate the old full-window bottleneck.

A fallback should rasterize a bounded local surface, upload it as a texture, and cache it.

---

## 14.9 Consider Vello as a backend/reference, not necessarily the permanent architecture

Run a proof-of-concept backend using Vello before spending months on a custom path renderer.

Questions to benchmark:

- quad-heavy desktop UI throughput,
- text behavior,
- damage/incremental scene updates,
- startup cost,
- binary size,
- memory use,
- WASM implications,
- custom compositing flexibility.

CreamUI can still own the retained runtime and paint representation even if Vello performs rasterization.

Do not couple core runtime semantics to Vello types.

---

## 14.10 Phase 8 exit criteria

- [ ] Common UI primitives are rasterized by the GPU.
- [ ] Full CPU framebuffer upload is no longer the normal GPU path.
- [ ] Static images upload once.
- [ ] Quad updates can patch small GPU-buffer ranges.
- [ ] Draw calls scale by batches, not widgets.
- [ ] CPU backend remains available for testing/fallback.
- [ ] Visual regression tests compare GPU and CPU outputs.

---

# 15. Phase 9 — Rebuild the Text Pipeline for Retained Layout + GPU Rendering

## Objective

Text must not become the next dominant bottleneck after rectangles become cheap.

---

## 15.1 Separate four concepts

Do not treat "draw text" as one operation.

Separate:

```text
font resolution
text shaping
line layout / wrapping
glyph raster/cache
glyph rendering
```

Each has different cache keys and invalidation behavior.

---

## 15.2 Introduce `TextRunId`

Runtime text node:

```rust
pub struct TextNode {
    pub text: Arc<str>,
    pub style: TextStyle,
    pub shaped: Option<ShapedTextId>,
}
```

Shaping cache key:

```text
text content
font face
font size
weight
style
script/language/direction
letter spacing
wrap constraints where applicable
```

---

## 15.3 Choose shaping technology explicitly

CreamUI should support correct non-trivial text eventually.

Evaluate:

- `cosmic-text`,
- `rustybuzz` + `swash`,
- another focused Rust shaping stack.

Do not keep a simplistic glyph-by-glyph model as the permanent API.

The retained scene should store glyph runs, not just strings.

---

## 15.4 Glyph atlas

GPU text path:

```text
glyph cache miss
    -> rasterize glyph once
    -> upload to atlas

frame
    -> glyph instances reference atlas coordinates
```

Support separate atlases if needed:

```text
monochrome alpha/SDF
color emoji
```

Do not rasterize every glyph every frame.

---

## 15.5 Shared measurement and rendering data

Text measurement must use the same shaped result that rendering uses.

Bad:

```text
Taffy measure -> shape string
paint -> shape same string again
```

Target:

```text
TextLayoutCache
    -> measured size
    -> positioned glyphs
    -> paint primitive
```

---

## 15.6 Text editing fast paths

Text inputs/editors require:

- caret geometry,
- selection geometry,
- composition/IME eventually,
- partial text changes.

Do not invalidate the entire editor component for a caret blink.

Caret:

```text
small overlay primitive
COMPOSITE/PAINT dirty only
```

Selection:

```text
selection rect primitives + glyph color mode
```

---

## 15.7 Phase 9 exit criteria

- [ ] Shaped text is cached.
- [ ] Text measurement shares shaped data with painting.
- [ ] Glyphs use a GPU atlas.
- [ ] Static text causes zero rasterization after warmup.
- [ ] Caret blinking does not reshape text or rerun layout.
- [ ] Text update benchmarks are included in CI/perf runs.

---

# 16. Phase 10 — Introduce a Compositor and Property-Only Updates

## Objective

Allow movement, opacity, scrolling, and selected animations without rerasterizing content.

---

## 16.1 Introduce retained property nodes

At minimum:

```rust
TransformNode
ClipNode
OpacityNode
```

A paint fragment references these property nodes.

This lets:

```text
content unchanged
transform changed
```

update only a property buffer.

---

## 16.2 Scrolling becomes transform-first

Scroll view structure:

```text
ScrollView clip
    -> content transform = translate(0, -scroll_y)
        -> child paint fragments
```

Normal scroll event:

```text
set scroll_y
-> transform dirty
-> hit-test transform/version dirty
-> no layout
-> no child paint rebuild
```

Newly exposed raster work depends on renderer strategy, but the scene itself remains retained.

---

## 16.3 Opacity animation

If a subtree's content is stable:

```text
opacity 0.0 -> 1.0
```

should avoid repainting child content.

Property node update only.

---

## 16.4 Layer promotion becomes explicit and evidence-based

Replace the current heuristic where paint behavior itself discovers animation and promotes cached layers.

The runtime should know:

```text
this subtree has transform animation
this subtree has opacity animation
this subtree uses filter requiring isolation
```

Then the compositor can decide whether to isolate it.

Track layer memory.

Promotion policy should consider:

```text
surface area
animation duration
expected repaint cost
memory budget
frequency
```

---

## 16.5 Phase 10 exit criteria

- [ ] Scroll offset updates do not rebuild/layout child subtrees.
- [ ] Transform-only animations avoid paint-record regeneration.
- [ ] Opacity-only animations avoid child rerasterization where possible.
- [ ] Layer promotion is owned by compositor logic, not inferred indirectly from painter calls.
- [ ] Layer memory is visible in devtools.

---

# 17. Phase 11 — Virtualization for Large Lists, Tables, Trees, and Desktop Surfaces

## Objective

Even a very fast retained engine should not instantiate 100,000 invisible rows.

---

## 17.1 Add a first-class `VirtualList`

API concept:

```rust
VirtualList::new(item_count)
    .estimate_height(28.0)
    .key(|index| ...)
    .item(|index| ...)
```

Runtime should mount only:

```text
visible rows
+ configurable overscan
```

---

## 17.2 Variable-height support

Maintain:

```text
estimated heights
measured heights
prefix sums / Fenwick tree
```

This allows:

```text
scroll offset -> item range
item height update -> efficient offset correction
```

---

## 17.3 Recycle cautiously

Retained identity and state can conflict with row recycling.

Prefer keyed mount/unmount semantics first.

Only add recycling pools if allocations become measurable.

Do not reuse an owner across unrelated keys unless all local state is explicitly reset.

---

## 17.4 Virtual table

Separate:

```text
row virtualization
column virtualization
sticky headers
selection state
```

The current table widget should eventually lower to this infrastructure rather than creating all cells.

---

## 17.5 Virtual tree

Tree view needs:

```text
flatten visible expanded nodes
key by stable tree ID
virtualize flattened sequence
```

Expand/collapse should alter only the relevant range.

---

## 17.6 Phase 11 exit criteria

- [ ] 100k logical list items do not mean 100k mounted runtime nodes.
- [ ] Scroll remains stable under variable row heights.
- [ ] Table/tree widgets have virtualization paths.
- [ ] Memory usage scales with visible content, not total model size.

---

# 18. Phase 12 — Resource Pipeline and Background Work

## Objective

Keep the UI thread focused on reactive mutations, layout coordination, scene preparation, and event routing.

---

## 18.1 Move image decode off the UI thread

Pipeline:

```text
resource request
    -> background decode
    -> ResourceReady message
    -> UI runtime mutation
    -> texture upload
```

Do not decode large PNG/JPEG/WebP images inside frame preparation.

---

## 18.2 Font/glyph work

Glyph rasterization can eventually move off-thread if atlas synchronization is designed carefully.

Start with synchronous cache misses.

Only parallelize after profiling proves it matters.

---

## 18.3 Avoid multithreading the runtime tree prematurely

The runtime tree should initially remain single-threaded.

This keeps:

- signal semantics simple,
- mutation ordering deterministic,
- event handling predictable.

Background workers should communicate through messages/resources, not mutate UI nodes directly.

---

## 18.4 Optional render thread later

A future architecture may use:

```text
UI thread
    -> immutable SceneDelta
render thread
    -> GPU resources / submit
```

Do not do this until:

- runtime invalidation is correct,
- scene deltas are explicit,
- CPU profiling shows render submission blocks the UI thread enough to justify complexity.

---

# 19. Phase 13 — ABI, FFI, Dynamic Runtime, and WASM Migration

CreamUI already has:

```text
crates/abi
crates/ffi
crates/dynamic
WASM demo/render path
```

The refactor must not accidentally make these impossible to support.

---

## 19.1 Define an ABI-v2 around handles and mutations

The retained runtime naturally maps to handle-based FFI.

Example:

```c
CuiNode cui_create_node(...);
void cui_insert_child(CuiNode parent, CuiNode child, ...);
void cui_set_text(CuiNode node, const char* text);
void cui_set_style(CuiNode node, ...);
void cui_remove_node(CuiNode node);
```

This is more efficient than passing complete reconstructed widget trees across FFI.

---

## 19.2 Keep ABI-v1 as an adapter temporarily

ABI-v1 can construct a legacy description and mount it through the compatibility layer.

Mark deprecated only after:

- examples migrate,
- dynamic runtime migrates,
- external users have a transition path.

---

## 19.3 WASM

The persistent runtime should be backend-agnostic.

For WASM:

```text
same runtime
same Taffy layout
same retained paint scene
wgpu/WebGPU backend where supported
```

Avoid designing the GPU scene around native-only synchronization assumptions.

---

# 20. Phase 14 — Devtools Must Explain Performance, Not Just Inspect Widgets

## Objective

Fine-grained retained engines are much easier to optimize when the engine can answer "why did this node update?"

---

## 20.1 Runtime tree inspector

Display:

```text
RuntimeNodeId
NodeKind
owner/component
parent
children
layout rect
dirty flags
paint bounds
hover/focus/pressed state
Taffy node ID
paint fragment ID
compositor layer
```

---

## 20.2 Invalidation tracing

In debug/perf builds record:

```rust
enum InvalidationReason {
    SignalBinding,
    StyleMutation,
    TextMutation,
    StructureMutation,
    ViewportResize,
    HoverEnter,
    HoverLeave,
    FocusChange,
    Animation,
    ResourceReady,
}
```

Devtools:

```text
Node #481
PAINT dirty because:
  SignalBinding effect #93 changed background.color
```

---

## 20.3 Frame timeline

Display stage timings:

```text
reactive flush       0.08 ms
mutation commit      0.03 ms
layout               0.00 ms
paint recording      0.04 ms
scene upload         0.02 ms
GPU                   ...
```

Also display:

```text
nodes mutated
nodes laid out
paint fragments rebuilt
damage area
GPU bytes uploaded
draw calls
glyph cache misses
image uploads
```

---

## 20.4 Visual overlays

Add toggles:

```text
show repaint damage
show layout invalidation
show compositor layers
show hit regions
show clip bounds
show virtualized range
```

This will prevent future regressions from becoming mysterious.

---

# 21. Phase 15 — Remove the Legacy Whole-Tree Architecture

Do this only after the new runtime powers the main examples.

---

## 21.1 Delete root reconciliation from the normal path

`Renderer::render(root: BoxedWidget, ...)` should no longer be the principal frame API.

Replace with something conceptually like:

```rust
renderer.prepare_frame(&mut runtime, viewport);
renderer.render(&runtime, surface);
```

The application does not submit a fresh root tree every frame.

---

## 21.2 Retire `Instance`

Anything still stored in `Instance` should have moved into:

```text
RuntimeNode
LayoutState
PaintState
CompositorState
EventState
```

Delete `Instance` only when no compatibility adapter depends on it.

---

## 21.3 Retire frame-time `children()` extraction

`Widget::children(&mut self) -> Vec<BoxedWidget>` should eventually become legacy-only.

New components mount children once through `MountCx`.

---

## 21.4 Retire whole-tree `Scene` reconstruction

Replace current transient `Scene` hit-region vectors with retained event structures.

---

## 21.5 Make CPU raster explicitly a fallback/reference backend

The primary documented backend becomes GPU scene rendering.

---

# 22. Suggested Internal API End State

A possible developer-facing runtime API beneath JSX:

```rust
pub struct UiRuntime {
    nodes: NodeArena,
    owners: OwnerArena,
    handlers: HandlerArena,

    layout: LayoutEngine,
    paint: PaintScene,
    compositor: Compositor,
    events: EventIndex,

    scheduler: ReactiveScheduler,
    damage: DamageRegion,
}
```

Frame:

```rust
impl UiRuntime {
    pub fn flush_reactivity(&mut self);
    pub fn commit_mutations(&mut self);
    pub fn update_layout(&mut self, viewport: Size);
    pub fn update_paint_scene(&mut self);
    pub fn update_event_index(&mut self);
    pub fn build_frame_delta(&mut self) -> FrameDelta;
}
```

Renderer:

```rust
pub trait SceneRenderer {
    fn apply_delta(&mut self, delta: FrameDelta);
    fn render(
        &mut self,
        target: &SurfaceTarget,
        damage: &DamageRegion,
    ) -> Result<(), RenderError>;
}
```

A `FrameDelta` should contain changes, not a reconstructed world:

```rust
pub struct FrameDelta {
    pub inserted_primitives: Vec<...>,
    pub updated_primitives: Vec<...>,
    pub removed_primitives: Vec<...>,

    pub updated_transforms: Vec<...>,
    pub updated_clips: Vec<...>,

    pub texture_uploads: Vec<...>,
    pub damage: DamageRegion,
}
```

---

# 23. Suggested Crate Evolution

Do not split everything immediately.

A reasonable progression:

```text
creamui-reactive
    signals
    effects
    owners
    scheduler

creamui-core
    styles
    geometry
    runtime tree
    mutation model
    layout adapter
    event metadata
    paint primitive definitions

creamui-render
    window/event loop
    CPU reference renderer
    GPU scene renderer
    resource upload
    compositor implementation

creamui-widgets
    high-level components lowered to core primitives

creamui-macros
    JSX -> mount/bind code generation
    #[component]

creamui-devtools
    retained runtime inspector
    invalidation/perf overlays
```

Only later, if compile times or dependency boundaries justify it:

```text
creamui-runtime
creamui-layout
creamui-scene
creamui-gpu
creamui-text
```

Avoid creating many tiny crates merely because the architecture diagram has many boxes.

---

# 24. Migration Strategy for Existing Widgets

Do not rewrite every widget before the architecture works.

Use three migration classes.

## Class A — Primitive

Migrate first:

```text
RawView
RawText
Image
basic Flex/Grid container
```

They become direct runtime primitives.

---

## Class B — Composite control

Examples:

```text
Button
Checkbox
Switch
Slider
Tabs
Sidebar
```

Lower to primitive nodes plus event bindings.

Example:

```text
Button
  Container
    background based on state
    border
    cursor
    click handler
    Text child
```

---

## Class C — Complex stateful widgets

Migrate last:

```text
TextInput
TextArea
Select
Table
Tree
Date/Time picker
Popover/Dialog
```

These depend heavily on:

- focus,
- keyboard routing,
- portals/absolute positioning,
- text measurement,
- scrolling,
- overlays.

Do not use them to validate the first retained-runtime prototype.

---

# 25. Recommended Implementation Order Inside Widgets

1. `RawView`
2. `RawText`
3. `Flex`
4. `Grid`
5. `Image`
6. `Button`
7. `Checkbox`
8. `Switch`
9. `Slider`
10. `Progress`
11. `ScrollView`
12. `Tabs`
13. `Sidebar`
14. `List`
15. `TextInput`
16. `TextArea`
17. `Popover`
18. `Dialog`
19. `Select`
20. `Table`
21. `Tree`
22. Pickers

At each step, compare legacy and retained rendering visually.

---

# 26. Testing Strategy

## 26.1 Runtime mutation tests

Test:

```text
insert
remove
move
replace
keyed reorder
owner disposal
stale ID detection
nested branch replacement
```

Use randomized operation sequences.

---

## 26.2 Invalidation tests

Given:

```text
old runtime state
mutation
```

assert exact flags.

Example:

```rust
set_background(...)
assert_eq!(dirty, PAINT);

set_width(...)
assert!(dirty.contains(LAYOUT | PAINT));

set_transform(...)
assert_eq!(dirty, COMPOSITE);
```

These tests are extremely important.

---

## 26.3 Layout mutation tests

Wrap Taffy in a counting adapter in tests.

Assert:

```text
paint-only mutation:
    set_style = 0
    set_children = 0

child insertion:
    set_children = 1 for affected parent
```

---

## 26.4 Golden rendering tests

Render known scenes through:

```text
CPU reference
GPU renderer
```

Compare with tolerance.

Include:

- radii,
- borders,
- nested clips,
- text,
- images,
- opacity,
- transforms,
- scroll.

---

## 26.5 Performance regression tests

Do not make CI depend on absolute millisecond thresholds across random machines.

Use structural assertions:

```text
single-leaf color update:
    reconciled_nodes <= 1
    layout_runs == 0
    paint_fragments_rebuilt == 1
```

Then run hardware timing benchmarks separately.

Structural metrics are much more stable.

---

# 27. Performance Targets

Treat these as engineering targets, not promises.

On a reasonable desktop system in release mode:

## 10k-node static dashboard

```text
idle CPU:
    effectively zero when no frame requested

single leaf paint mutation:
    CPU update << 1 ms

single text mutation:
    ideally < 1 ms for runtime/layout preparation

hover transition:
    << 1 ms CPU
```

## 60/120 Hz interaction

CreamUI should aim for:

```text
60 Hz frame budget:   16.67 ms
120 Hz frame budget:   8.33 ms
```

But the runtime/layout/scene preparation portion should generally consume a small fraction of that.

For ordinary localized interactions:

```text
reactive + runtime + layout + scene CPU:
    target under ~1 ms
```

on medium-sized UIs.

---

# 28. Things Not to Do

## 28.1 Do not optimize full-tree reconciliation forever

Making `reconcile()` 3x faster still leaves an O(N) operation on a one-node update.

It is useful only as a transitional optimization.

---

## 28.2 Do not build a traditional virtual DOM

CreamUI already controls its Rust proc-macro syntax.

Use that advantage to compile JSX into mount/bind operations.

A VDOM would reintroduce diff costs the new architecture is trying to remove.

---

## 28.3 Do not rewrite Taffy before proving it is the bottleneck

The current usage pattern is invalidating Taffy's own retained caches.

Fix the integration first.

---

## 28.4 Do not immediately introduce a render thread

Cross-thread synchronization can hide architectural inefficiency while greatly increasing complexity.

First make one-thread CPU work proportional to change size.

---

## 28.5 Do not make every widget a GPU texture/layer

That creates:

- memory pressure,
- texture management overhead,
- extra compositing,
- expensive invalidation behavior.

Normal UI should be batches of primitives.

Layers should be selective.

---

## 28.6 Do not couple runtime nodes to wgpu

`creamui-core` should define scene primitives and state in backend-neutral terms.

The CPU renderer, wgpu renderer, WASM path, screenshot renderer, and future backends should consume the same retained scene model.

---

## 28.7 Do not let widgets read Signals during paint

Reactive values must resolve into runtime state before layout/paint.

Otherwise dependency tracking becomes traversal-dependent again.

---

# 29. Practical Git Branch / Milestone Breakdown

A possible sequence that keeps PRs reviewable:

```text
perf/00-baseline
perf/01-taffy-diff
perf/02-hover-noop
runtime/01-node-arena
runtime/02-mutations
runtime/03-dirty-flags
reactive/01-owners
reactive/02-ui-scheduler
runtime/04-bindings
jsx/01-mount-context
jsx/02-dynamic-bindings
jsx/03-branches
jsx/04-keyed-lists
layout/01-runtime-taffy
events/01-retained-targets
events/02-node-focus
events/03-hit-index
paint/01-recording-painter
paint/02-fragments
paint/03-damage
gpu/01-quads
gpu/02-images
text/01-shaping
gpu/03-glyph-atlas
gpu/04-clips
compositor/01-transforms
compositor/02-scroll
widgets/retained-primitives
widgets/retained-controls
perf/02-virtual-list
runtime/remove-legacy-render
```

Avoid one enormous branch containing every architectural change.

The refactor is giant; the commits do not need to be.

---

# 30. Proposed Milestones

## Milestone A — Current engine stops wasting obvious work

Contains Phases 0–1.

Result:

```text
same architecture
measurably fewer Taffy invalidations
no unnecessary hover redraws
real performance telemetry
```

This can ship independently.

---

## Milestone B — Persistent runtime exists behind legacy widgets

Contains Phase 2.

Result:

```text
stable RuntimeNodeId
mutation model
dirty flags
legacy adapter
```

The user-facing API barely changes.

---

## Milestone C — Fine-grained CreamUI

Contains Phases 3–5.

This is the architectural turning point.

Result:

```text
Signal -> binding -> RuntimeNode mutation

not

Signal -> rebuild window tree
```

At this milestone, CreamUI should already become dramatically more scalable even before the GPU renderer is replaced.

---

## Milestone D — Retained interaction and paint

Contains Phases 6–7.

Result:

```text
events independent from paint
paint fragments retained
damage tracking
```

This creates the correct substrate for a modern renderer.

---

## Milestone E — Real GPU renderer

Contains Phases 8–10.

Result:

```text
no normal full CPU framebuffer
GPU primitive scene
glyph/image caches
compositor transforms
```

This is where CreamUI begins to behave like a purpose-built native UI engine rather than a CPU canvas presented through wgpu.

---

## Milestone F — Large applications

Contains Phases 11–15.

Result:

```text
virtualization
resource pipeline
ABI v2
high-value devtools
legacy path removed
```

At this point the architecture should be suitable for the desktop environment / shell use case.

---

# 31. First Concrete Refactor Sprint

If starting immediately, the first implementation sprint should **not** begin with the GPU renderer.

Do this:

## Task 1 — Add metrics

Instrument:

```text
reconcile visits
Taffy setters
measure calls
paint visits
root build count
repaint reason
```

---

## Task 2 — Add benchmark scene

Create:

```text
examples/perf-tree
```

Controls:

```text
1k / 10k / 50k nodes

buttons:
    mutate one leaf color
    mutate one leaf text
    mutate root style
    reorder list item
    toggle subtree

automatic:
    pointer movement test
    scroll stress
```

Overlay current metrics.

---

## Task 3 — Stop unchanged Taffy writes

Modify current `Instance` reconciliation.

Prove with counters:

```text
leaf paint-only change
    Taffy style writes dramatically reduced
```

---

## Task 4 — Stop no-op hover repaints

Cache hover identity.

Prove:

```text
1000 cursor move events inside same control
    redraw requests ~= 0 after initial entry
```

---

## Task 5 — Add `RuntimeNodeId + NodeArena`

No renderer integration yet.

Test arena semantics completely.

---

## Task 6 — Add `Mutation + DirtyFlags`

Implement mutation classification and tests.

---

## Task 7 — Mount one primitive retained subtree

Make one experimental example use:

```text
RuntimeNode Container
RuntimeNode Text
```

without recreating a `BoxedWidget` on state change.

---

## Task 8 — Bind one Signal directly to retained text

Target proof:

```rust
let counter = Signal::new(0);
```

changing `counter` updates one persistent text node.

Metrics must show:

```text
root builds:            0
tree reconciliation:    0
unrelated nodes visited:0
```

Do not proceed to large JSX changes until this proof works.

That proof validates the entire direction.

---

# 32. Critical Proof-of-Concept Benchmark

Build this before committing to the full rewrite.

Scene:

```text
root Flex
  10,000 Text/Block nodes
```

Only node `8,421` reads:

```rust
Signal<u32>
```

Pressing Space increments it.

Compare:

## Legacy path

Measure:

```text
root build time
10k widget allocations/constructions
reconcile visits
Taffy setters
layout
paint
upload
```

## Retained prototype

Measure:

```text
effect run
Mutation::SetText
text measurement
layout invalidation
paint-fragment replacement
damage
```

The retained prototype must demonstrate that the CPU cost of incrementing node 8,421 is approximately independent of whether the tree contains:

```text
1,000
10,000
50,000
```

unrelated nodes.

If it does not, inspect the runtime architecture before continuing.

---

# 33. Definition of "Done"

The refactor is complete when this statement is true:

> CreamUI's normal update cost is primarily proportional to changed UI state, not total mounted UI size.

Concretely:

- Signals no longer rebuild entire windows.
- JSX static structure mounts once.
- Runtime nodes have persistent identity.
- Layout mutations are precise.
- Paint data is retained.
- Hit testing is retained.
- Hover does not repaint unchanged state.
- Scroll does not relayout/rebuild descendants.
- GPU backend rasterizes UI primitives directly.
- Text shaping/glyphs are cached.
- Large lists are virtualized.
- Devtools expose invalidation causes and frame costs.
- The old `BoxedWidget -> reconcile entire root -> CPU raster framebuffer -> upload framebuffer` path is no longer the primary rendering architecture.

---

# 34. Final Target Mental Model

CreamUI should stop thinking in terms of:

```text
"render the application again"
```

and instead think in terms of:

```text
"apply these mutations to the persistent UI world"
```

The window is not a function result that gets rebuilt.

It is a persistent scene.

Components mount structure into that scene.

Signals mutate properties of that structure.

Layout reacts only to geometry-affecting mutations.

Paint reacts only to pixel-affecting mutations.

The compositor reacts only to compositing-property mutations.

The GPU receives deltas, not an entirely new image of the application.

That is the direction that gives CreamUI a realistic path from an early native Rust UI MVP to a renderer capable of supporting large desktop applications and eventually a full Linux desktop shell without inheriting browser-sized runtime overhead.
