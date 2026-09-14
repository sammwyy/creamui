//! CreamUI's performance laboratory: reproducible benchmark scenes and
//! engine counters used to measure reconcile/layout/paint cost.
//!
//! - [`scenes`] builds the synthetic widget trees benchmarked in `benches/`.
//! - [`metrics`] reads the counters `creamui-core`/`creamui-render` record
//!   under the `perf-metrics` feature.
//! - [`painter`] provides the painters those benchmarks render into.

pub mod metrics;
pub mod painter;
pub mod scenes;
