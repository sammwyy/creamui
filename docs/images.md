# Images

`creamui-image` decodes an asset once into `ImageData` and reuses it across widget-tree rebuilds.

```toml
creamui-image = { version = "0.1", features = ["jpeg", "webp"] }
```

```rust
use creamui_image::{Image, ImageData, ImageFit};

let avatar = ImageData::from_bytes(include_bytes!("avatar.webp"))?;
let view = Image::new(avatar)
    .fit(ImageFit::Cover)
    .corner_radius(48.0);
```

Supported fits are `Fill`, `Contain`, `Cover`, and `None`. Rounded corners are clipped by the renderer, so square images can become avatars or circular thumbnails without preprocessing the file.

Run `cargo run -p images` in this repository for a PNG, JPEG, and WebP gallery.

## Temporary rendering effects

A custom image viewport can call `Painter::draw_image_effects` with an existing
`RgbaImage`, optional tint, opacity and `creamui_core::ImageEffects`. The effects
contain clockwise quarter turns (`0`–`3`) and a 3×4 RGB color matrix. The caller
supplies the final destination rectangle, swapping the source aspect ratio for
odd quarter turns.

```rust
use creamui_core::ImageEffects;

let inverted = ImageEffects {
    quarter_turns: 1,
    color_matrix: [
        [-1.0, 0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0, 1.0],
        [0.0, 0.0, -1.0, 1.0],
    ],
};
painter.draw_image_effects(rotated_bounds, &image, None, 1.0, inverted);
```

Each matrix row multiplies premultiplied `[red, green, blue, alpha]`; the fourth
coefficient is therefore a straight-RGB bias scaled by source alpha. Output RGB is
clamped to `[0, alpha]`, and alpha is preserved. Tint is applied before the matrix
and overall opacity after it. The default effect is identity; invalid matrix
coefficients fall back to identity and quarter turns are normalized modulo four.

SceneRecorder and retained paint commands keep the original image identity.
GPU rendering transforms UVs and applies non-identity matrices in the fragment
shader without uploading a new image. Identity matrices retain the regular image
sampling path. The CPU renderer rotates sampling geometry and filters only the
visible destination region, preserving premultiplied transparency. A generic
Painter fallback materializes transformed pixels when a backend does not override
this operation.

Retained `runtime::ImagePrimitive` records now include `effects` and `opacity`;
custom primitive constructors should use `ImageEffects::default()` and `1.0` to
preserve their previous rendering.
