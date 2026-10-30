use crate::RgbaImage;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageEffects {
    pub quarter_turns: u8,
    pub color_matrix: [[f32; 4]; 3],
}
impl Default for ImageEffects {
    fn default() -> Self {
        Self {
            quarter_turns: 0,
            color_matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        }
    }
}
impl ImageEffects {
    pub fn normalized(mut self) -> Self {
        self.quarter_turns %= 4;
        if self
            .color_matrix
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            self.color_matrix = Self::default().color_matrix;
        }
        self
    }
    pub fn has_color_transform(self) -> bool {
        self.color_matrix != Self::default().color_matrix
    }
    pub fn transform_pixel(self, pixel: &mut [u8]) {
        let source = [
            pixel[0] as f32,
            pixel[1] as f32,
            pixel[2] as f32,
            pixel[3] as f32,
        ];
        for (channel, row) in pixel[..3].iter_mut().zip(self.color_matrix) {
            *channel = row
                .into_iter()
                .zip(source)
                .map(|(a, b)| a * b)
                .sum::<f32>()
                .clamp(0.0, source[3])
                .round() as u8;
        }
    }
    pub fn apply(self, image: &RgbaImage) -> RgbaImage {
        let effects = self.normalized();
        if effects == Self::default() {
            return image.clone();
        }
        let (width, height) = (image.width(), image.height());
        let (out_width, out_height) = if effects.quarter_turns.is_multiple_of(2) {
            (width, height)
        } else {
            (height, width)
        };
        let source = image.pixels();
        let mut pixels = vec![0; source.len()];
        for y in 0..height {
            for x in 0..width {
                let (dx, dy) = match effects.quarter_turns {
                    1 => (height - 1 - y, x),
                    2 => (width - 1 - x, height - 1 - y),
                    3 => (y, width - 1 - x),
                    _ => (x, y),
                };
                let from = (y as usize * width as usize + x as usize) * 4;
                let to = (dy as usize * out_width as usize + dx as usize) * 4;
                pixels[to..to + 4].copy_from_slice(&source[from..from + 4]);
                effects.transform_pixel(&mut pixels[to..to + 4]);
            }
        }
        RgbaImage::new(out_width, out_height, pixels)
            .expect("image effects preserve the pixel count")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_and_premultiplied_colors_preserve_source_and_alpha() {
        let image = RgbaImage::new(2, 1, vec![128, 0, 0, 128, 0, 255, 0, 255]).unwrap();
        let effects = ImageEffects {
            quarter_turns: 1,
            color_matrix: [
                [-1.0, 0.0, 0.0, 1.0],
                [0.0, -1.0, 0.0, 1.0],
                [0.0, 0.0, -1.0, 1.0],
            ],
        };
        let result = effects.apply(&image);
        assert_eq!((result.width(), result.height()), (1, 2));
        assert_eq!(&*result.pixels(), &[0, 128, 128, 128, 255, 0, 255, 255]);
        assert_eq!(&image.pixels()[..4], &[128, 0, 0, 128]);
        assert_eq!(ImageEffects::default().apply(&image).id(), image.id());
        let invalid = ImageEffects {
            color_matrix: [[f32::NAN; 4]; 3],
            ..Default::default()
        };
        assert_eq!(invalid.normalized(), ImageEffects::default());
    }
}
