use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{run, WindowHandle, WindowOptions};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::layout::{Align, Justify};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let handle = Rc::new(RefCell::new(None::<WindowHandle>));
    let on_ready = handle.clone();

    run(
        WindowOptions {
            title: "CreamUI — Transparent window".into(),
            width: 520,
            height: 330,
            transparent: true,
            theme: Theme::dark(),
            ..Default::default()
        },
        Color::rgba(0, 0, 0, 0),
        move |window| *on_ready.borrow_mut() = Some(window),
        move |size: Size| -> BoxedWidget {
            let theme = use_theme();
            let minimize_handle = handle.clone();
            let maximize_handle = handle.clone();
            let close_handle = handle.clone();

            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} size={(size.width, size.height)} padding={28.0} justify={Justify::Center} align={Align::Center} background={Color::rgba(0, 0, 0, 0)}>
                    <Flex direction={FlexDirection::Column} size={(430.0, 230.0)} gap={16.0} padding={24.0} background={theme.surface_elevated} corner_radius={22.0}>
                        <Text font_size={24.0}>"Transparent window"</Text>
                        <Text color={theme.text_secondary}>"The space around this rounded panel is transparent."</Text>
                        <Flex grow={1.0} />
                        <Flex gap={8.0} justify={Justify::End}>
                            <Button on_click={move || {
                                if let Some(window) = minimize_handle.borrow().as_ref() {
                                    window.minimize();
                                }
                            }}>"Minimize"</Button>
                            <Button on_click={move || {
                                if let Some(window) = maximize_handle.borrow().as_ref() {
                                    window.maximize();
                                }
                            }}>"Maximize"</Button>
                            <Button on_click={move || {
                                if let Some(window) = close_handle.borrow().as_ref() {
                                    window.close();
                                }
                            }}>"Close"</Button>
                        </Flex>
                    </Flex>
                </Flex>
            })
        },
    );
}
