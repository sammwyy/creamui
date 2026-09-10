//! Declarative JSX syntax for CreamUI's Rust widget API.
//!
//! `jsx!` is deliberately a thin syntax layer: it emits calls to
//! the native widget API and does not own state, rendering, or an ABI.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    braced, parse::Parse, parse::ParseStream, parse_macro_input, Error, Expr, FnArg, Ident, ItemFn,
    LitStr, Pat, Result, Token,
};

fn crate_path(facade_module: &str, standalone_crate: &str) -> TokenStream2 {
    if let Ok(found) = crate_name("creamui") {
        let module = format_ident!("{facade_module}");
        return match found {
            FoundCrate::Itself => quote!(crate::#module),
            FoundCrate::Name(name) => {
                let facade = format_ident!("{name}");
                quote!(::#facade::#module)
            }
        };
    }
    let standalone = format_ident!("{standalone_crate}");
    quote!(::#standalone)
}

fn widgets_path() -> TokenStream2 {
    crate_path("widgets", "creamui_widgets")
}

fn core_path() -> TokenStream2 {
    crate_path("core", "creamui_core")
}

fn jsx_path() -> TokenStream2 {
    crate_path("jsx_runtime", "creamui_jsx")
}

fn image_path() -> TokenStream2 {
    crate_path("image", "creamui_image")
}

fn dynamic_path() -> TokenStream2 {
    crate_path("dynamic", "creamui_dynamic")
}

fn apply_jsx_style_property(name: &str, output: TokenStream2, value: Expr) -> TokenStream2 {
    let core = core_path();
    match name {
        "border" => quote!({ let (color, width) = #value; #core::Styled::border(#output, color, width) }),
        "outline" => quote!({ let (color, width) = #value; #core::Styled::outline(#output, color, width) }),
        _ => {
            let method = format_ident!("{name}");
            quote!(#core::Styled::#method(#output, #value))
        }
    }
}

macro_rules! jsx_common_style_methods {
    ($( $variant:ident($value:ty) => $name:literal |$target:ident, $field:ident| $apply:block => $builder:ident($( $argument:ident: $argument_type:ty ),*) |$style:ident| $body:block; )*) => {
        fn is_common_style_prop(name: &str) -> bool {
            matches!(name, "style" | "hover_style" | "pressed_style" | "focus_style" | "disabled_style")
                || matches!(name, $( stringify!($builder) )|*)
        }

        fn apply_common_style(&self, mut output: TokenStream2) -> Result<TokenStream2> {
            if let Some(value) = self.prop("style")? {
                let core = core_path();
                output = quote!({
                    let __creamui_style = #value;
                    #core::Styled::with_style(#output, __creamui_style)
                });
            }
            $(
                if let Some(value) = self.prop(stringify!($builder))? {
                    output = apply_jsx_style_property(stringify!($builder), output, value);
                }
            )*
            for name in ["hover_style", "pressed_style", "focus_style", "disabled_style"] {
                if let Some(value) = self.prop(name)? {
                    let core = core_path();
                    let method = format_ident!("{name}");
                    output = quote!(#core::Styled::#method(#output, #value));
                }
            }
            Ok(output)
        }
    };
}

/// Builds a CreamUI widget using JSX-like syntax.
///
/// See the workspace README for the supported component and prop mapping.
#[proc_macro]
pub fn jsx(input: TokenStream) -> TokenStream {
    let element = parse_macro_input!(input as Element);
    match element.expand() {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Builds a widget tree through `creamui-dynamic`, the `dlopen`-backed ABI
/// client. It accepts the same tags as [`jsx`], but every intrinsic requires
/// `ctx={...}` and expands to ABI calls instead of native widget builders.
#[proc_macro]
pub fn abi_jsx(input: TokenStream) -> TokenStream {
    let element = parse_macro_input!(input as Element);
    match element.expand_dynamic() {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Turns a normal Rust function into a JSX component.
///
/// The function's named arguments become fields in a generated `NameProps`
/// struct. `<Name value={...}/>` then expands to `Name(NameProps { value })`.
#[proc_macro_attribute]
pub fn component(_attribute: TokenStream, input: TokenStream) -> TokenStream {
    let function = parse_macro_input!(input as ItemFn);
    match expand_component(function) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expand_component(function: ItemFn) -> Result<TokenStream2> {
    if function.sig.receiver().is_some() {
        return Err(Error::new_spanned(
            function.sig,
            "`#[component]` only supports free functions",
        ));
    }
    if function.sig.inputs.is_empty() {
        return Err(Error::new_spanned(
            function.sig,
            "a JSX component needs at least one named prop",
        ));
    }
    let function_name = &function.sig.ident;
    let props_name = format_ident!("{}Props", function_name);
    let visibility = &function.vis;
    let attributes = &function.attrs;
    let output = &function.sig.output;
    let block = &function.block;
    let mut fields = Vec::new();
    let mut bindings = Vec::new();
    for argument in &function.sig.inputs {
        let FnArg::Typed(argument) = argument else {
            unreachable!("receiver checked above");
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(Error::new_spanned(
                &argument.pat,
                "component props must be simple identifiers",
            ));
        };
        let name = &pattern.ident;
        let ty = &argument.ty;
        fields.push(quote!(pub #name: #ty));
        bindings.push(quote!(#name));
    }
    Ok(quote! {
        #visibility struct #props_name { #( #fields, )* }
        #( #attributes )*
        #[allow(non_snake_case)]
        #visibility fn #function_name(props: #props_name) #output {
            let #props_name { #( #bindings, )* } = props;
            #block
        }
    })
}

struct Attribute {
    name: Ident,
    value: Expr,
}

enum Child {
    Element(Element),
    Expression(Expr),
    Text(LitStr),
}

struct Element {
    tag: Ident,
    attributes: Vec<Attribute>,
    children: Vec<Child>,
}

impl Parse for Element {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![<]>()?;
        let tag: Ident = input.parse()?;
        let mut attributes = Vec::new();

        while !input.peek(Token![>]) && !input.peek(Token![/]) {
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let content;
            braced!(content in input);
            attributes.push(Attribute {
                name,
                value: content.parse()?,
            });
        }

        if input.peek(Token![/]) {
            input.parse::<Token![/]>()?;
            input.parse::<Token![>]>()?;
            return Ok(Element {
                tag,
                attributes,
                children: Vec::new(),
            });
        }
        input.parse::<Token![>]>()?;

        let mut children = Vec::new();
        while !is_closing_tag(input) {
            if input.peek(Token![<]) {
                children.push(Child::Element(input.parse()?));
            } else if input.peek(syn::token::Brace) {
                let content;
                braced!(content in input);
                children.push(Child::Expression(content.parse()?));
            } else if input.peek(LitStr) {
                children.push(Child::Text(input.parse()?));
            } else {
                return Err(input.error("JSX children must be an element, a Rust expression in `{...}`, or a string literal"));
            }
        }

        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let closing_tag: Ident = input.parse()?;
        if closing_tag != tag {
            return Err(Error::new_spanned(
                closing_tag,
                format!("expected closing tag `</{tag}>`"),
            ));
        }
        input.parse::<Token![>]>()?;
        Ok(Element {
            tag,
            attributes,
            children,
        })
    }
}

fn is_closing_tag(input: ParseStream<'_>) -> bool {
    if !input.peek(Token![<]) {
        return false;
    }
    let fork = input.fork();
    let _ = fork.parse::<Token![<]>();
    fork.peek(Token![/])
}

impl Element {
    fn is_native_intrinsic(&self) -> bool {
        matches!(
            self.tag.to_string().as_str(),
            "Block"
                | "CUIWindowDragArea"
                | "RawView"
                | "Flex"
                | "Grid"
                | "GridItem"
                | "RawText"
                | "RawButton"
                | "ScrollView"
                | "Text"
                | "Heading"
                | "Button"
                | "Checkbox"
                | "TextArea"
                | "TextInput"
                | "Slider"
                | "ColorPicker"
                | "DateTimePicker"
                | "DateInput"
                | "TimeInput"
                | "Image"
        )
    }

    fn prop(&self, name: &str) -> Result<Option<Expr>> {
        let mut result = None;
        for attribute in &self.attributes {
            if attribute.name == name {
                if result.is_some() {
                    return Err(Error::new_spanned(
                        &attribute.name,
                        format!("duplicate `{name}` prop"),
                    ));
                }
                result = Some(attribute.value.clone());
            }
        }
        Ok(result)
    }

    fn required_prop(&self, name: &str) -> Result<Expr> {
        self.prop(name)?.ok_or_else(|| {
            Error::new_spanned(
                &self.tag,
                format!("`{}` requires a `{name}` prop", self.tag),
            )
        })
    }

    fn reject_unknown_props(&self, allowed: &[&str]) -> Result<()> {
        for attribute in &self.attributes {
            if !allowed.iter().any(|allowed| attribute.name == allowed)
                && !Self::is_common_style_prop(&attribute.name.to_string())
            {
                return Err(Error::new_spanned(
                    &attribute.name,
                    format!(
                        "`{}` does not support the `{}` prop",
                        self.tag, attribute.name
                    ),
                ));
            }
        }
        Ok(())
    }

    creamui_core::creamui_style_property_schema!(jsx_common_style_methods);

    fn text_child(&self) -> Result<TokenStream2> {
        if self.children.len() != 1 {
            return Err(Error::new_spanned(
                &self.tag,
                format!("`{}` requires exactly one text child", self.tag),
            ));
        }
        match &self.children[0] {
            Child::Expression(expression) => Ok(quote!(#expression)),
            Child::Text(text) => Ok(quote!(#text)),
            Child::Element(element) => Err(Error::new_spanned(
                &element.tag,
                format!("`{}` accepts text, not child elements", self.tag),
            )),
        }
    }

    fn container_children(&self, initial: TokenStream2) -> Result<TokenStream2> {
        let jsx = jsx_path();
        let mut output = initial;
        for child in &self.children {
            let child = match child {
                Child::Element(element) => {
                    let expanded = element.expand()?;
                    if element.is_native_intrinsic() {
                        quote!(::std::boxed::Box::new(#expanded))
                    } else {
                        quote!(#jsx::IntoWidget::into_widget(#expanded))
                    }
                }
                Child::Expression(expression) => {
                    quote!(#jsx::IntoWidget::into_widget(#expression))
                }
                Child::Text(text) => {
                    return Err(Error::new_spanned(
                        text,
                        format!("`{}` cannot contain bare text; use `<Text>`", self.tag),
                    ))
                }
            };
            output = quote!(#output.child(#child));
        }
        Ok(output)
    }

    fn expand(&self) -> Result<TokenStream2> {
        let widgets = widgets_path();
        let core = core_path();
        let image = image_path();
        let output = match self.tag.to_string().as_str() {
            "Block" => {
                self.reject_unknown_props(&[
                    "style",
                    "size",
                    "padding",
                    "padding_xy",
                    "margin",
                    "fill",
                    "grow",
                    "background",
                    "corner_radius",
                    "children",
                ])?;
                let mut output = quote!(#widgets::layout::Block::new());
                if let Some(size) = self.prop("size")? {
                    output = quote!({
                        let (width, height) = #size;
                        #output.size(width, height)
                    });
                }
                if let Some(padding) = self.prop("padding")? {
                    output = quote!(#output.padding(#padding));
                }
                if let Some(padding_xy) = self.prop("padding_xy")? {
                    output = quote!({
                        let (horizontal, vertical) = #padding_xy;
                        #output.padding_xy(horizontal, vertical)
                    });
                }
                if let Some(margin) = self.prop("margin")? {
                    output = quote!(#output.margin(#margin));
                }
                if let Some(fill) = self.prop("fill")? {
                    output = quote!(if #fill { #output.fill() } else { #output });
                }
                if let Some(grow) = self.prop("grow")? {
                    output = quote!(#output.grow(#grow));
                }
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "Flex" => {
                self.reject_unknown_props(&[
                    "direction",
                    "gap",
                    "gap_x",
                    "gap_y",
                    "align",
                    "justify",
                    "align_content",
                    "wrap",
                    "padding",
                    "padding_xy",
                    "size",
                    "full_width",
                    "full_height",
                    "fill",
                    "grow",
                    "shrink",
                    "basis",
                    "align_self",
                    "background",
                    "corner_radius",
                    "children",
                ])?;
                let mut output = quote!(#widgets::layout::Flex::row());
                if let Some(direction) = self.prop("direction")? {
                    output = quote!(#output.direction(#direction));
                }
                if let Some(gap) = self.prop("gap")? {
                    output = quote!(#output.gap(#gap));
                }
                if let Some(gap) = self.prop("gap_x")? {
                    output = quote!(#output.gap_x(#gap));
                }
                if let Some(gap) = self.prop("gap_y")? {
                    output = quote!(#output.gap_y(#gap));
                }
                if let Some(align) = self.prop("align")? {
                    output = quote!(#output.align(#align));
                }
                if let Some(justify) = self.prop("justify")? {
                    output = quote!(#output.justify(#justify));
                }
                if let Some(align_content) = self.prop("align_content")? {
                    output = quote!(#output.align_content(#align_content));
                }
                if let Some(wrap) = self.prop("wrap")? {
                    output = quote!(#output.wrap(#wrap));
                }
                if let Some(padding) = self.prop("padding")? {
                    output = quote!(#output.padding(#padding));
                }
                if let Some(padding_xy) = self.prop("padding_xy")? {
                    output = quote!({
                        let (horizontal, vertical) = #padding_xy;
                        #output.padding_xy(horizontal, vertical)
                    });
                }
                if let Some(size) = self.prop("size")? {
                    output = quote!({
                        let (width, height) = #size;
                        #output.size(width, height)
                    });
                }
                if let Some(full_width) = self.prop("full_width")? {
                    output = quote!(if #full_width { #output.full_width() } else { #output });
                }
                if let Some(full_height) = self.prop("full_height")? {
                    output = quote!(if #full_height { #output.full_height() } else { #output });
                }
                if let Some(fill) = self.prop("fill")? {
                    output = quote!(if #fill { #output.fill() } else { #output });
                }
                if let Some(grow) = self.prop("grow")? {
                    output = quote!(#output.grow(#grow));
                }
                if let Some(shrink) = self.prop("shrink")? {
                    output = quote!(#output.shrink(#shrink));
                }
                if let Some(basis) = self.prop("basis")? {
                    output = quote!(#output.basis(#basis));
                }
                if let Some(align_self) = self.prop("align_self")? {
                    output = quote!(#output.align_self(#align_self));
                }
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "Grid" => {
                self.reject_unknown_props(&[
                    "columns",
                    "rows",
                    "template_columns",
                    "template_rows",
                    "gap",
                    "gap_x",
                    "gap_y",
                    "justify",
                    "align_content",
                    "flow",
                    "padding",
                    "padding_xy",
                    "size",
                    "fill",
                    "grow",
                    "background",
                    "corner_radius",
                    "children",
                ])?;
                let mut output = quote!(#widgets::layout::Grid::new());
                if let Some(columns) = self.prop("columns")? {
                    output = quote!(#output.columns(#columns));
                }
                if let Some(rows) = self.prop("rows")? {
                    output = quote!(#output.rows(#rows));
                }
                if let Some(tracks) = self.prop("template_columns")? {
                    output = quote!(#output.template_columns(#tracks));
                }
                if let Some(tracks) = self.prop("template_rows")? {
                    output = quote!(#output.template_rows(#tracks));
                }
                if let Some(gap) = self.prop("gap")? {
                    output = quote!(#output.gap(#gap));
                }
                if let Some(gap) = self.prop("gap_x")? {
                    output = quote!(#output.gap_x(#gap));
                }
                if let Some(gap) = self.prop("gap_y")? {
                    output = quote!(#output.gap_y(#gap));
                }
                if let Some(justify) = self.prop("justify")? {
                    output = quote!(#output.justify(#justify));
                }
                if let Some(align_content) = self.prop("align_content")? {
                    output = quote!(#output.align_content(#align_content));
                }
                if let Some(flow) = self.prop("flow")? {
                    output = quote!(#output.flow(#flow));
                }
                if let Some(padding) = self.prop("padding")? {
                    output = quote!(#output.padding(#padding));
                }
                if let Some(padding_xy) = self.prop("padding_xy")? {
                    output = quote!({
                        let (horizontal, vertical) = #padding_xy;
                        #output.padding_xy(horizontal, vertical)
                    });
                }
                if let Some(size) = self.prop("size")? {
                    output = quote!({
                        let (width, height) = #size;
                        #output.size(width, height)
                    });
                }
                if let Some(fill) = self.prop("fill")? {
                    output = quote!(if #fill { #output.fill() } else { #output });
                }
                if let Some(grow) = self.prop("grow")? {
                    output = quote!(#output.grow(#grow));
                }
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "GridItem" => {
                self.reject_unknown_props(&[
                    "column",
                    "row",
                    "column_span",
                    "row_span",
                    "children",
                ])?;
                let column = self.prop("column")?;
                let row = self.prop("row")?;
                let mut output = quote!(#widgets::layout::GridItem::new());
                match (column, row) {
                    (Some(column), Some(row)) => output = quote!(#output.at(#column, #row)),
                    (Some(column), None) => output = quote!(#output.column(#column)),
                    (None, Some(row)) => output = quote!(#output.row(#row)),
                    (None, None) => {}
                }
                if let Some(span) = self.prop("column_span")? {
                    output = quote!(#output.column_span(#span));
                }
                if let Some(span) = self.prop("row_span")? {
                    output = quote!(#output.row_span(#span));
                }
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "RawView" => {
                self.reject_unknown_props(&["style", "background", "corner_radius", "children"])?;
                let output = quote!(#widgets::raw::RawView::new(#core::Style::default()));
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "CUIWindowDragArea" => {
                self.reject_unknown_props(&[
                    "style",
                    "size",
                    "padding",
                    "padding_xy",
                    "margin",
                    "fill",
                    "grow",
                    "background",
                    "corner_radius",
                    "children",
                ])?;
                let mut output = quote!(#widgets::CUIWindowDragArea::new());
                if let Some(size) = self.prop("size")? {
                    output = quote!({
                        let (width, height) = #size;
                        #output.size(width, height)
                    });
                }
                if let Some(padding) = self.prop("padding")? {
                    output = quote!(#output.padding(#padding));
                }
                if let Some(padding_xy) = self.prop("padding_xy")? {
                    output = quote!({
                        let (horizontal, vertical) = #padding_xy;
                        #output.padding_xy(horizontal, vertical)
                    });
                }
                if let Some(margin) = self.prop("margin")? {
                    output = quote!(#output.margin(#margin));
                }
                if let Some(fill) = self.prop("fill")? {
                    output = quote!(if #fill { #output.fill() } else { #output });
                }
                if let Some(grow) = self.prop("grow")? {
                    output = quote!(#output.grow(#grow));
                }
                if let Some(children) = self.prop("children")? {
                    if !self.children.is_empty() {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`children` cannot be combined with nested JSX children",
                        ));
                    }
                    Ok(quote!(#output.with_children(#children)))
                } else {
                    self.container_children(output)
                }
            }
            "RawText" => {
                self.reject_unknown_props(&["color", "font_size", "align", "style"])?;
                let text = self.text_child()?;
                let mut output = quote!(#widgets::raw::RawText::unstyled(#text));
                if let Some(align) = self.prop("align")? {
                    output = quote!(#core::Styled::text_align(#output, #align));
                }
                Ok(output)
            }
            "RawButton" => {
                self.reject_unknown_props(&[
                    "style",
                    "on_click",
                    "background",
                    "hover_style",
                    "pressed_style",
                    "corner_radius",
                    "disabled",
                ])?;
                let on_click = self.required_prop("on_click")?;
                let mut output =
                    quote!(#widgets::raw::RawButton::new(#core::Style::default(), #on_click));
                if let Some(disabled) = self.prop("disabled")? {
                    output = quote!(#output.disabled(#disabled));
                }
                self.container_children(output)
            }
            "ScrollView" => {
                self.reject_unknown_props(&["style", "scroll_y", "on_scroll"])?;
                let scroll_y = self.required_prop("scroll_y")?;
                let on_scroll = self.required_prop("on_scroll")?;
                self.container_children(
                    quote!(#widgets::themed::ScrollView::new(#core::layout::Style::default(), #scroll_y, #on_scroll)),
                )
            }
            "Text" => {
                self.reject_unknown_props(&[
                    "font_size",
                    "font_family",
                    "size",
                    "secondary",
                    "align",
                    "color",
                    "style",
                ])?;
                let text = self.text_child()?;
                let mut output = if let Some(secondary) = self.prop("secondary")? {
                    quote!(if #secondary { #widgets::themed::Text::secondary(#text) } else { #widgets::themed::Text::new(#text) })
                } else {
                    quote!(#widgets::themed::Text::new(#text))
                };
                if let Some(size) = self.prop("size")? {
                    output = quote!(#output.size(#size));
                }
                if let Some(align) = self.prop("align")? {
                    output = quote!(#core::Styled::text_align(#output, #align));
                }
                Ok(output)
            }
            "Heading" => {
                self.reject_unknown_props(&["size", "align", "color", "style", "font_family"])?;
                let text = self.text_child()?;
                let mut output = if let Some(size) = self.prop("size")? {
                    quote!(#widgets::themed::Heading::sized(#size, #text))
                } else {
                    quote!(#widgets::themed::Heading::new(#text))
                };
                if let Some(align) = self.prop("align")? {
                    output = quote!(#core::Styled::text_align(#output, #align));
                }
                Ok(output)
            }
            "Button" => {
                self.reject_unknown_props(&["on_click", "style", "disabled"])?;
                let on_click = self.required_prop("on_click")?;
                let label = self.text_child()?;
                let mut output = quote!(#widgets::themed::Button::new(#label, #on_click));
                if let Some(disabled) = self.prop("disabled")? {
                    output = quote!(#output.disabled(#disabled));
                }
                Ok(output)
            }
            "Checkbox" => {
                self.reject_unknown_props(&["checked", "on_click"])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`Checkbox` cannot have children",
                    ));
                }
                let checked = self.required_prop("checked")?;
                let on_click = self.required_prop("on_click")?;
                Ok(quote!(#widgets::themed::Checkbox::new(#checked, #on_click)))
            }
            "TextInput" => {
                self.reject_unknown_props(&[
                    "controller",
                    "value",
                    "on_change",
                    "style",
                    "placeholder",
                    "clipboard_enabled",
                    "font_family",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`TextInput` cannot have children",
                    ));
                }
                let mut output = match (self.prop("controller")?, self.prop("value")?, self.prop("on_change")?) {
                    (Some(controller), None, None) => quote!(#widgets::themed::TextInput::controlled(#controller)),
                    (None, Some(value), Some(on_change)) => quote!(#widgets::themed::TextInput::new(#value, #on_change)),
                    (None, None, None) => return Err(Error::new_spanned(&self.tag, "`TextInput` requires either a `controller` prop or both `value` and `on_change`")),
                    _ => return Err(Error::new_spanned(&self.tag, "`TextInput`'s `controller` prop cannot be combined with `value`/`on_change`")),
                };
                if let Some(placeholder) = self.prop("placeholder")? {
                    output = quote!(#output.placeholder(#placeholder));
                }
                if let Some(enabled) = self.prop("clipboard_enabled")? {
                    output = quote!(#output.clipboard_enabled(#enabled));
                }
                Ok(output)
            }
            "TextArea" => {
                self.reject_unknown_props(&[
                    "controller",
                    "value",
                    "on_change",
                    "style",
                    "placeholder",
                    "selection_background",
                    "selection_text_color",
                    "alternating_line_background",
                    "active_line_background",
                    "corner_radius",
                    "border_width",
                    "cursor",
                    "on_cursor_change",
                    "selection",
                    "on_selection_change",
                    "selection_background",
                    "selection_text_color",
                    "on_ctrl_o",
                    "clipboard_enabled",
                    "wrap",
                    "font_family",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`TextArea` cannot have children",
                    ));
                }
                let controller = self.prop("controller")?;
                if let Some(controller) = &controller {
                    for name in [
                        "value",
                        "on_change",
                        "cursor",
                        "on_cursor_change",
                        "selection",
                        "on_selection_change",
                    ] {
                        if self.prop(name)?.is_some() {
                            return Err(Error::new_spanned(&self.tag, format!("`TextArea`'s `controller` prop cannot be combined with `{name}`")));
                        }
                    }
                    let _ = controller;
                }
                let mut output = if let Some(controller) = &controller {
                    quote!(#widgets::themed::TextArea::controlled(#controller))
                } else {
                    let value = self.required_prop("value")?;
                    let on_change = self.required_prop("on_change")?;
                    quote!(#widgets::themed::TextArea::new(#value, #on_change))
                };
                if let Some(placeholder) = self.prop("placeholder")? {
                    output = quote!(#output.placeholder(#placeholder));
                }
                if let Some(color) = self.prop("alternating_line_background")? {
                    output = quote!(#output.alternating_line_background(#color));
                }
                if let Some(color) = self.prop("active_line_background")? {
                    output = quote!(#output.active_line_background(#color));
                }
                if let Some(width) = self.prop("border_width")? {
                    output = quote!(#output.border_width(#width));
                }
                match (self.prop("cursor")?, self.prop("on_cursor_change")?) {
                    (Some(cursor), Some(on_change)) => {
                        output = quote!(#output.cursor(#cursor, #on_change))
                    }
                    (None, None) => {}
                    _ => {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`TextArea` requires both `cursor` and `on_cursor_change`",
                        ))
                    }
                }
                match (self.prop("selection")?, self.prop("on_selection_change")?) {
                    (Some(selection), Some(on_change)) => {
                        output = quote!(#output.selection(#selection, #on_change))
                    }
                    (None, None) => {}
                    _ => {
                        return Err(Error::new_spanned(
                            &self.tag,
                            "`TextArea` requires both `selection` and `on_selection_change`",
                        ))
                    }
                }
                if let Some(color) = self.prop("selection_background")? {
                    output = quote!(#output.selection_background(#color));
                }
                if let Some(color) = self.prop("selection_text_color")? {
                    output = quote!(#output.selection_text_color(#color));
                }
                if let Some(callback) = self.prop("on_ctrl_o")? {
                    output = quote!(#output.on_ctrl_o(#callback));
                }
                if let Some(enabled) = self.prop("clipboard_enabled")? {
                    output = quote!(#output.clipboard_enabled(#enabled));
                }
                if let Some(wrap) = self.prop("wrap")? {
                    output = quote!(#output.wrap(#wrap));
                }
                Ok(output)
            }
            "Slider" => {
                self.reject_unknown_props(&["value", "on_change", "style"])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`Slider` cannot have children",
                    ));
                }
                let value = self.required_prop("value")?;
                let on_change = self.required_prop("on_change")?;
                Ok(quote!(#widgets::themed::Slider::new(#value, #on_change)))
            }
            "ColorPicker" => {
                self.reject_unknown_props(&[
                    "controller",
                    "value",
                    "on_change",
                    "style",
                    "popup_width",
                    "disabled",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`ColorPicker` cannot have children",
                    ));
                }
                let controller = self.required_prop("controller")?;
                let value = self.required_prop("value")?;
                let on_change = self.required_prop("on_change")?;
                let mut output = quote!(#widgets::themed::ColorPicker::controlled(#value, #controller, #on_change));
                if let Some(width) = self.prop("popup_width")? {
                    output = quote!(#output.popup_width(#width));
                }
                if let Some(disabled) = self.prop("disabled")? {
                    output = quote!(#output.disabled(#disabled));
                }
                Ok(output)
            }
            "Image" => {
                self.reject_unknown_props(&["data", "style", "fit", "corner_radius"])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`Image` cannot have children",
                    ));
                }
                let data = self.required_prop("data")?;
                let mut output = quote!(#image::Image::new(#data));
                if let Some(fit) = self.prop("fit")? {
                    output = quote!(#output.fit(#fit));
                }
                Ok(output)
            }
            "DateTimePicker" | "DateInput" | "TimeInput" => {
                self.reject_unknown_props(&[
                    "controller",
                    "style",
                    "show_date",
                    "show_time",
                    "minute_step",
                    "popup_width",
                    "disabled",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "picker inputs cannot have children",
                    ));
                }
                let controller = self.required_prop("controller")?;
                let tag = self.tag.to_string();
                if tag != "DateTimePicker"
                    && (self.prop("show_date")?.is_some() || self.prop("show_time")?.is_some())
                {
                    return Err(Error::new_spanned(&self.tag, "`show_date`/`show_time` belong to `DateTimePicker`; use the matching DateInput or TimeInput tag"));
                }
                let constructor = match tag.as_str() {
                    "DateTimePicker" => {
                        quote!(#widgets::themed::DateTimePicker::controlled(#controller))
                    }
                    "DateInput" => {
                        quote!(#widgets::themed::DateInput::controlled(#controller))
                    }
                    _ => {
                        quote!(#widgets::themed::TimeInput::controlled(#controller))
                    }
                };
                let mut output = constructor;
                if let Some(show) = self.prop("show_date")? {
                    output = quote!(#output.show_date(#show));
                }
                if let Some(show) = self.prop("show_time")? {
                    output = quote!(#output.show_time(#show));
                }
                if let Some(step) = self.prop("minute_step")? {
                    output = quote!(#output.minute_step(#step));
                }
                if let Some(width) = self.prop("popup_width")? {
                    output = quote!(#output.popup_width(#width));
                }
                if let Some(disabled) = self.prop("disabled")? {
                    output = quote!(#output.disabled(#disabled));
                }
                Ok(output)
            }
            _ => return self.expand_user_component(),
        }?;
        self.apply_common_style(output)
    }

    fn expand_user_component(&self) -> Result<TokenStream2> {
        let jsx = jsx_path();
        let tag = &self.tag;
        let children = self
            .children
            .iter()
            .map(|child| match child {
                Child::Element(element) => {
                    let expanded = element.expand()?;
                    if element.is_native_intrinsic() {
                        Ok(quote!(::std::boxed::Box::new(#expanded)))
                    } else {
                        Ok(quote!(#jsx::IntoWidget::into_widget(#expanded)))
                    }
                }
                Child::Expression(expression) => {
                    Ok(quote!(#jsx::IntoWidget::into_widget(#expression)))
                }
                Child::Text(text) => Err(Error::new_spanned(
                    text,
                    "custom components cannot contain bare text; use `<Text>`",
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        if self.attributes.is_empty() {
            return if children.is_empty() {
                Ok(quote!(#tag()))
            } else {
                let props_name = format_ident!("{}Props", tag);
                Ok(quote!(#tag(#props_name { children: vec![ #( #children, )* ] })))
            };
        }
        if self.attributes.len() == 1 && self.attributes[0].name == "props" {
            if !children.is_empty() {
                return Err(Error::new_spanned(
                    &self.tag,
                    "`props` cannot be combined with JSX children; put `children` in the explicit props value",
                ));
            }
            let props = &self.attributes[0].value;
            return Ok(quote!(#tag(#props)));
        }
        if self
            .attributes
            .iter()
            .any(|attribute| attribute.name == "props")
        {
            return Err(Error::new_spanned(
                &self.tag,
                "`props` cannot be combined with named component props",
            ));
        }
        let props_name = format_ident!("{}Props", tag);
        let fields = self.attributes.iter().map(|attribute| {
            let name = &attribute.name;
            let value = &attribute.value;
            quote!(#name: #value)
        });
        if children.is_empty() {
            Ok(quote!(#tag(#props_name { #( #fields, )* })))
        } else {
            Ok(quote!(#tag(#props_name { #( #fields, )* children: vec![ #( #children, )* ] })))
        }
    }

    fn dynamic_container_children(&self, initial: TokenStream2) -> Result<TokenStream2> {
        let mut output = initial;
        for child in &self.children {
            let child = match child {
                Child::Element(element) => element.expand_dynamic()?,
                Child::Expression(expression) => quote!(#expression),
                Child::Text(text) => {
                    return Err(Error::new_spanned(
                        text,
                        format!("`{}` cannot contain bare text; use `<Text>`", self.tag),
                    ))
                }
            };
            output = quote!(#output.child(#child));
        }
        Ok(output)
    }

    fn expand_dynamic(&self) -> Result<TokenStream2> {
        let dynamic = dynamic_path();
        match self.tag.to_string().as_str() {
            "Block" => {
                self.reject_unknown_props(&["ctx", "style", "background", "corner_radius"])?;
                let ctx = self.required_prop("ctx")?;
                let style = self.required_prop("style")?;
                let mut output = quote!(#dynamic::block_styled(#ctx, #style));
                if let Some(background) = self.prop("background")? {
                    output = quote!(#output.background(#background));
                }
                if let Some(radius) = self.prop("corner_radius")? {
                    output = quote!(#output.corner_radius(#radius));
                }
                self.dynamic_container_children(output)
            }
            "ScrollView" => {
                self.reject_unknown_props(&["ctx", "theme", "style", "scroll_y", "on_scroll"])?;
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let style = self.required_prop("style")?;
                let scroll_y = self.required_prop("scroll_y")?;
                let on_scroll = self.required_prop("on_scroll")?;
                self.dynamic_container_children(
                    quote!(#dynamic::scroll_view(#ctx, #theme, #style, #scroll_y, #on_scroll)),
                )
            }
            "Text" => {
                self.reject_unknown_props(&["ctx", "theme", "font_size", "secondary"])?;
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let text = self.text_child()?;
                let secondary = self.prop("secondary")?;
                let font_size = self.prop("font_size")?;
                match (secondary, font_size) {
                    (Some(_), Some(_)) => Err(Error::new_spanned(
                        &self.tag,
                        "the ABI Text component cannot combine `secondary` and `font_size` yet",
                    )),
                    (Some(secondary), None) => Ok(
                        quote!(if #secondary { #dynamic::themed_text_secondary(#ctx, #theme, &(#text)) } else { #dynamic::themed_text(#ctx, #theme, &(#text)) }),
                    ),
                    (None, Some(font_size)) => {
                        Ok(quote!(#dynamic::themed_text_sized(#ctx, #theme, &(#text), #font_size)))
                    }
                    (None, None) => Ok(quote!(#dynamic::themed_text(#ctx, #theme, &(#text)))),
                }
            }
            "Button" => {
                self.reject_unknown_props(&["ctx", "theme", "on_click"])?;
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let on_click = self.required_prop("on_click")?;
                let label = self.text_child()?;
                Ok(quote!(#dynamic::button(#ctx, #theme, &(#label), #on_click)))
            }
            "Checkbox" => {
                self.reject_unknown_props(&["ctx", "theme", "checked", "on_click"])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`Checkbox` cannot have children",
                    ));
                }
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let checked = self.required_prop("checked")?;
                let on_click = self.required_prop("on_click")?;
                Ok(quote!(#dynamic::checkbox(#ctx, #theme, #checked, #on_click)))
            }
            "TextInput" => {
                self.reject_unknown_props(&[
                    "ctx",
                    "theme",
                    "style",
                    "value",
                    "on_change",
                    "placeholder",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`TextInput` cannot have children",
                    ));
                }
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let style = self.required_prop("style")?;
                let value = self.required_prop("value")?;
                let on_change = self.required_prop("on_change")?;
                let mut output =
                    quote!(#dynamic::text_input(#ctx, #theme, #style, &(#value), #on_change));
                if let Some(placeholder) = self.prop("placeholder")? {
                    output = quote!(#output.placeholder(#theme, &(#placeholder)));
                }
                Ok(output)
            }
            "TextArea" => {
                self.reject_unknown_props(&[
                    "ctx",
                    "theme",
                    "style",
                    "value",
                    "on_change",
                    "placeholder",
                ])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`TextArea` cannot have children",
                    ));
                }
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let style = self.required_prop("style")?;
                let value = self.required_prop("value")?;
                let on_change = self.required_prop("on_change")?;
                let mut output =
                    quote!(#dynamic::text_area(#ctx, #theme, #style, &(#value), #on_change));
                if let Some(placeholder) = self.prop("placeholder")? {
                    output = quote!(#output.area_placeholder(#theme, &(#placeholder)));
                }
                match (
                    self.prop("selection_background")?,
                    self.prop("selection_text_color")?,
                ) {
                    (Some(background), Some(text)) => {
                        output = quote!(#output.area_selection_colors(#background, #text));
                    }
                    (None, None) => {}
                    _ => return Err(Error::new_spanned(&self.tag, "dynamic `TextArea` requires both `selection_background` and `selection_text_color`")),
                }
                Ok(output)
            }
            "Slider" => {
                self.reject_unknown_props(&["ctx", "theme", "style", "value", "on_change"])?;
                if !self.children.is_empty() {
                    return Err(Error::new_spanned(
                        &self.tag,
                        "`Slider` cannot have children",
                    ));
                }
                let ctx = self.required_prop("ctx")?;
                let theme = self.required_prop("theme")?;
                let style = self.required_prop("style")?;
                let value = self.required_prop("value")?;
                let on_change = self.required_prop("on_change")?;
                Ok(quote!(#dynamic::slider(#ctx, #theme, #style, #value, #on_change)))
            }
            _ => self.expand_dynamic_user_component(),
        }
    }

    fn expand_dynamic_user_component(&self) -> Result<TokenStream2> {
        let tag = &self.tag;
        let children = self
            .children
            .iter()
            .map(|child| match child {
                Child::Element(element) => element.expand_dynamic(),
                Child::Expression(expression) => Ok(quote!(#expression)),
                Child::Text(text) => Err(Error::new_spanned(
                    text,
                    "custom ABI components cannot contain bare text; use `<Text>`",
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        if self.attributes.is_empty() {
            return if children.is_empty() {
                Ok(quote!(#tag()))
            } else {
                let props_name = format_ident!("{}Props", tag);
                Ok(quote!(#tag(#props_name { children: vec![ #( #children, )* ] })))
            };
        }
        if self.attributes.len() == 1 && self.attributes[0].name == "props" {
            if !children.is_empty() {
                return Err(Error::new_spanned(
                    &self.tag,
                    "`props` cannot be combined with JSX children",
                ));
            }
            let props = &self.attributes[0].value;
            return Ok(quote!(#tag(#props)));
        }
        if self
            .attributes
            .iter()
            .any(|attribute| attribute.name == "props")
        {
            return Err(Error::new_spanned(
                &self.tag,
                "`props` cannot be combined with named component props",
            ));
        }
        let props_name = format_ident!("{}Props", tag);
        let fields = self.attributes.iter().map(|attribute| {
            let name = &attribute.name;
            let value = &attribute.value;
            quote!(#name: #value)
        });
        if children.is_empty() {
            Ok(quote!(#tag(#props_name { #( #fields, )* })))
        } else {
            Ok(quote!(#tag(#props_name { #( #fields, )* children: vec![ #( #children, )* ] })))
        }
    }
}
