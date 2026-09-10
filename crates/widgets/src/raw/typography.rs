use super::*;

/// An unstyled blockquote: a solid color bar along the left edge of its own
/// box, with no opinion on the indent needed to clear it — bake that into
/// `style`'s left padding (taffy insets children by it automatically; the
/// bar itself is painted against the widget's full, unpadded rect).
pub struct RawQuote {
    pub style: Style,
    pub bar_color: Color,
    pub bar_width: f32,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub children: Vec<BoxedWidget>,
}

impl RawQuote {
    pub fn new(style: Style, bar_color: Color, bar_width: f32) -> Self {
        RawQuote {
            style,
            bar_color,
            bar_width,
            background: None,
            corner_radius: 0.0,
            children: Vec::new(),
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn child(mut self, widget: BoxedWidget) -> Self {
        self.children.push(widget);
        self
    }

    pub fn with_children(mut self, widgets: Vec<BoxedWidget>) -> Self {
        self.children = widgets;
        self
    }
}

impl Widget for RawQuote {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(background) = self.background {
            painter.fill_rect(rect, background, self.corner_radius);
        }
        painter.fill_rect(
            Rect {
                x: rect.x,
                y: rect.y,
                width: self.bar_width,
                height: rect.height,
            },
            self.bar_color,
            0.0,
        );
    }

    fn children(&mut self) -> Vec<BoxedWidget> {
        std::mem::take(&mut self.children)
    }
}

/// An unstyled preformatted text block: renders `text` verbatim (whitespace
/// and line breaks intact) on its own background rect, inset by `padding`
/// logical pixels on every side. No monospace face ships with CreamUI, so
/// column alignment is only approximate unless the application registers one.
pub struct RawPre {
    pub style: Style,
    pub text: String,
    pub color: Color,
    pub font_size: f32,
    pub family: Option<String>,
    pub background: Option<Color>,
    pub corner_radius: f32,
    pub padding: f32,
}

impl RawPre {
    pub fn new(style: Style, text: impl Into<String>, color: Color, font_size: f32) -> Self {
        RawPre {
            style,
            text: text.into(),
            color,
            font_size,
            family: None,
            background: None,
            corner_radius: 0.0,
            padding: 0.0,
        }
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }
}

impl Widget for RawPre {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        if let Some(background) = self.background {
            painter.fill_rect(rect, background, self.corner_radius);
        }
        let inset = Rect {
            x: rect.x + self.padding,
            y: rect.y + self.padding,
            width: (rect.width - self.padding * 2.0).max(0.0),
            height: (rect.height - self.padding * 2.0).max(0.0),
        };
        painter.fill_text_font(
            inset,
            &self.text,
            self.color,
            self.font_size,
            TextAlign::Start,
            self.family.as_deref(),
            false,
            false,
        );
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        let text = self.text.clone();
        let font_size = self.font_size;
        let padding = self.padding;
        let family = self.family.clone();
        Some(Box::new(move |known_dimensions, available_space| {
            let max_width = match (known_dimensions.width, available_space.width) {
                (Some(w), _) => w,
                (None, creamui_core::layout::AvailableSpace::Definite(w)) => w,
                (None, _) => crate::text_metrics::unbounded_width(),
            };
            let content_width = (max_width - padding * 2.0).max(1.0);
            let natural_height = crate::text_metrics::content_height_family(
                &text,
                font_size,
                content_width,
                family.as_deref(),
            ) + padding * 2.0;
            creamui_core::layout::Size {
                width: known_dimensions.width.unwrap_or(max_width),
                height: known_dimensions.height.unwrap_or(natural_height),
            }
        }))
    }
}

/// An unstyled clickable line of text — a hyperlink with no color or
/// underline opinion of its own beyond what's passed in.
pub struct RawLink {
    pub style: Style,
    pub text: String,
    pub color: Color,
    pub hover_color: Option<Color>,
    pub font_size: f32,
    pub align: TextAlign,
    pub underline: bool,
    pub family: Option<String>,
    pub on_click: Rc<dyn Fn()>,
    pub disabled: bool,
}

impl RawLink {
    pub fn new(
        style: Style,
        text: impl Into<String>,
        color: Color,
        font_size: f32,
        on_click: impl Fn() + 'static,
    ) -> Self {
        RawLink {
            style,
            text: text.into(),
            color,
            hover_color: None,
            font_size,
            align: TextAlign::Start,
            underline: true,
            family: None,
            on_click: Rc::new(on_click),
            disabled: false,
        }
    }

    pub fn hover_color(mut self, color: Color) -> Self {
        self.hover_color = Some(color);
        self
    }

    pub fn underline(mut self, underline: bool) -> Self {
        self.underline = underline;
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }
}

impl Widget for RawLink {
    fn style(&self) -> creamui_core::Style {
        self.style.clone().into()
    }

    fn paint(&self, painter: &mut dyn Painter, rect: Rect) {
        let color = if !self.disabled && painter.hovered(rect) {
            self.hover_color.unwrap_or(self.color)
        } else {
            self.color
        };
        painter.fill_text_font(
            rect,
            &self.text,
            color,
            self.font_size,
            self.align,
            self.family.as_deref(),
            false,
            false,
        );
        super::draw_text_decorations(
            painter,
            rect,
            &self.text,
            self.font_size,
            self.family.as_deref(),
            false,
            self.align,
            color,
            self.underline,
            false,
        );
    }

    fn measure(&self) -> Option<creamui_core::MeasureFn> {
        let text = self.text.clone();
        let font_size = self.font_size;
        let family = self.family.clone();
        Some(Box::new(move |known_dimensions, available_space| {
            let max_width = match (known_dimensions.width, available_space.width) {
                (Some(w), _) => w,
                (None, creamui_core::layout::AvailableSpace::Definite(w)) => w,
                (None, _) => crate::text_metrics::unbounded_width(),
            };
            let (natural_width, natural_height) = crate::text_metrics::measure_family(
                &text,
                font_size,
                max_width,
                family.as_deref(),
                false,
            );
            creamui_core::layout::Size {
                width: known_dimensions.width.unwrap_or(natural_width),
                height: known_dimensions.height.unwrap_or(natural_height),
            }
        }))
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        if self.disabled {
            return None;
        }
        Some(activate_on_key(self.on_click.clone()))
    }

    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        if self.disabled {
            None
        } else {
            Some(self.on_click.clone())
        }
    }

    fn cursor_icon(&self) -> Option<CursorIcon> {
        Some(if self.disabled {
            CursorIcon::NotAllowed
        } else {
            CursorIcon::Pointer
        })
    }
}
