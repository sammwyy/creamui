use super::*;
/// A five-step type scale shared by [`Text`] and [`Heading`], in the spirit
/// of Tailwind's `text-xs`..`text-xl` steps or HTML's h5..h1 headings: `Xs`
/// is smallest, `Xl` is largest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Xl,
    Lg,
    Md,
    Sm,
    Xs,
}

impl TextSize {
    /// Point size for body copy ([`Text`]).
    fn text_px(self) -> f32 {
        match self {
            TextSize::Xl => 20.0,
            TextSize::Lg => 17.0,
            TextSize::Md => 14.0,
            TextSize::Sm => 12.0,
            TextSize::Xs => 11.0,
        }
    }

    /// Point size for block headings ([`Heading`]), noticeably larger than
    /// the body scale at every step since a heading needs to read as a
    /// heading even at its smallest (`Xs`, an h5-equivalent).
    fn heading_px(self) -> f32 {
        match self {
            TextSize::Xl => 28.0, // h1
            TextSize::Lg => 22.0, // h2
            TextSize::Md => 18.0, // h3
            TextSize::Sm => 15.0, // h4
            TextSize::Xs => 13.0, // h5
        }
    }
}

/// Themed body text using the theme's primary text color. Centered by
/// default (handy for standalone labels and captions); call [`Text::align`]
/// for left/right-aligned copy.
pub struct Text {
    inner: RawText,
}
impl_styled_inner!(Text);

impl Text {
    pub fn new(text: impl Into<String>) -> Self {
        let theme = use_theme();
        Text {
            inner: RawText::new(text, theme.text_primary, theme.typography.body)
                .font_family(theme.font_family),
        }
    }

    /// Same as [`Text::new`] but using the theme's secondary (muted) text color.
    pub fn secondary(text: impl Into<String>) -> Self {
        let theme = use_theme();
        Text {
            inner: RawText::new(text, theme.text_secondary, theme.typography.body)
                .font_family(theme.font_family),
        }
    }

    /// Sets the font size from the shared [`TextSize`] scale (`Md` matches
    /// the 14px default from [`Text::new`]).
    pub fn size(mut self, size: TextSize) -> Self {
        self.inner.style.typography.font_size = Some(size.text_px());
        self
    }
}

impl Widget for Text {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        self.inner.measure()
    }
}

/// A themed block heading, the h1-h5 equivalent of [`Text`]: same five-step
/// [`TextSize`] scale, but left-aligned by default (a heading reads as a
/// block-level title, not a centered caption) and sized up so even its
/// smallest step (`Xs`, an h5) still reads as a heading next to body copy.
pub struct Heading {
    inner: RawText,
}
impl_styled_inner!(Heading);

impl Heading {
    /// A heading at an explicit [`TextSize`] step.
    pub fn sized(size: TextSize, text: impl Into<String>) -> Self {
        let theme = use_theme();
        Heading {
            inner: RawText::new(
                text,
                theme.text_primary,
                match size {
                    TextSize::Xl => theme.typography.title,
                    TextSize::Md => theme.typography.section,
                    _ => size.heading_px(),
                },
            )
            .bold(true)
            .text_align(TextAlign::Start)
            .font_family(theme.font_family),
        }
    }

    /// Shorthand for [`Heading::sized`] with [`TextSize::Md`] (an
    /// h3-equivalent), a reasonable default for a section heading.
    pub fn new(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Md, text)
    }

    /// h1-equivalent: [`TextSize::Xl`].
    pub fn xl(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Xl, text)
    }

    /// h2-equivalent: [`TextSize::Lg`].
    pub fn lg(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Lg, text)
    }

    /// h3-equivalent: [`TextSize::Md`].
    pub fn md(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Md, text)
    }

    /// h4-equivalent: [`TextSize::Sm`].
    pub fn sm(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Sm, text)
    }

    /// h5-equivalent: [`TextSize::Xs`].
    pub fn xs(text: impl Into<String>) -> Self {
        Self::sized(TextSize::Xs, text)
    }
}

impl Widget for Heading {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        self.inner.paint(painter, rect);
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        self.inner.measure()
    }
}
