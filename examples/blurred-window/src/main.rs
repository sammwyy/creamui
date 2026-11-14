use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{run, BlurRegion, WindowHandle, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{Align, Justify};
use creamui_widgets::{AppTitlebar, Button, ButtonSize, ButtonState, ButtonVariant};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let handle = Rc::new(RefCell::new(None::<WindowHandle>));
    let on_ready = handle.clone();

    run(
        WindowOptions {
            title: "CreamUI — Blurred window".into(),
            width: 520,
            height: 380,
            decorations: false,
            transparent: true,
            // `Content` must be requested at creation to enable per-frame
            // translucent-background tracking.
            blur: Some(BlurRegion::Window),
            theme: Theme::dark(),
            ..Default::default()
        },
        Color::rgba(0, 0, 0, 0),
        move |window| *on_ready.borrow_mut() = Some(window),
        move |size: Size| -> BoxedWidget {
            let theme = use_theme();
            let panel = theme.surface_elevated;
            let translucent = Color::rgba(panel.r, panel.g, panel.b, 140);

            let titlebar: BoxedWidget = Box::new(AppTitlebar::new(size.width).height(36.0));

            let blur_button = |label: &'static str, region: Option<BlurRegion>| {
                let window = handle.clone();
                Box::new(Button::styled(
                    ButtonVariant::Secondary,
                    ButtonSize::Xs,
                    label,
                    ButtonState::Normal,
                    move || {
                        if let Some(window) = window.borrow().as_ref() {
                            window.set_blur_region(region.clone());
                        }
                    },
                )) as BoxedWidget
            };

            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} size={(size.width, size.height)} background={Color::rgba(0, 0, 0, 0)}>
                    {titlebar}
                    <Flex direction={FlexDirection::Column} grow={1.0} justify={Justify::Center} align={Align::Center}>
                        <Flex direction={FlexDirection::Column} size={(400.0, 170.0)} gap={14.0} padding={20.0} background={translucent} corner_radius={18.0}>
                            <Text font_size={22.0}>"Blurred window"</Text>
                            <Flex grow={1.0} />
                            <Flex gap={6.0}>
                                {blur_button("On", Some(BlurRegion::Window))}
                                {blur_button("Off", None)}
                            </Flex>
                        </Flex>
                    </Flex>
                </Flex>
            })
        },
    );
}
