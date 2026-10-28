# creamui-fonts

Font registry, loading, and the `use_font()` hook for CreamUI.

CreamUI resolves its default `"system-ui"` family from the operating system when text is first used. It tries the native UI font first (`Segoe UI` on Windows, Helvetica/Arial on macOS, and Liberation/DejaVu/Noto Sans on Linux), without embedding a font in every app. Register additional fonts and resolve a CSS-style family stack against them:

```rust
use creamui_fonts::{include_font, register_bytes, use_font, FontWeight};

register_bytes("Inter", FontWeight::Regular, include_font!("./Inter-Regular.ttf").to_vec()).unwrap();
register_bytes("Inter", FontWeight::Bold, include_font!("./Inter-Bold.ttf").to_vec()).unwrap();

// Inside a window's build_ui — see creamui_reactive::with_context_scope:
let font = use_font("Inter, system-ui");
```

`register_file` loads from disk at runtime instead. Files are memory-mapped and parsed on demand, so only the tables and glyphs actually used become resident. `layout` shapes and wraps text with a face; it is what both widget measurement and the renderer use. `resolve` (family, weight) -> face is what `use_font` calls under the hood, for non-hook call sites.

Fallback searches preserve font-index order and complete or partial cluster
coverage. Rejected font mappings are released after their coverage is inspected.
The per-thread fallback cache retains up to 32 selected files and 32 MiB of font
file data, plus 1,024 coverage records within 8 MiB. Coverage ranges let later
searches skip rejected files without reopening them. Character lookups have a
4,096-entry limit and use weak face references; registry changes invalidate
their results. Oversized selected fonts remain usable by the requesting layout
without entering the file cache. Registered faces and layouts retain their own
ownership independently of these fallback-cache limits.

On Linux, measure font mappings and resident pages with:

```bash
cargo run --release -p creamui-fonts --example fallback_memory
```

The probe samples `/proc/self/smaps` after loading the primary face, searching
for unsupported characters, and shaping mixed scripts. It reports mapping count,
mapped size, RSS, and PSS for font files separately from the rest of the process.
Timing samples include the first coverage scan and later repeated/distinct
missing-character searches.

Part of [CreamUI](https://github.com/sammwyy/creamui), licensed under Apache-2.0.
