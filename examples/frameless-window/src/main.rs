use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_render::{run, WindowHandle, WindowOptions};
use creamui_theme::{use_theme, Theme};
use creamui_widgets::layout::{Align, Justify};
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let handle = Rc::new(RefCell::new(None::<WindowHandle>));
    let on_ready = handle.clone();

    run(
        WindowOptions {
            title: "CreamUI — Frameless window".into(),
            width: 640,
            height: 400,
            decorations: false,
            resizable: true,
            theme: Theme::dark(),
            ..Default::default()
        },
        Theme::dark().surface,
        move |window| *on_ready.borrow_mut() = Some(window),
        move |size: Size| -> BoxedWidget {
            let theme = use_theme();
            let minimize_handle = handle.clone();
            let maximize_handle = handle.clone();
            let close_handle = handle.clone();

            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} size={(size.width, size.height)} background={theme.surface}>
                    <Flex align={Align::Center} justify={Justify::Between} padding={12.0} background={theme.surface_elevated}>
                        <CUIWindowDragArea grow={1.0}>
                            <Text font_size={16.0}>"CreamUI — Frameless window"</Text>
                        </CUIWindowDragArea>
                        <Flex gap={8.0}>
                            <Button on_click={move || {
                                if let Some(window) = minimize_handle.borrow().as_ref() {
                                    window.minimize();
                                }
                            }}>"—"</Button>
                            <Button on_click={move || {
                                if let Some(window) = maximize_handle.borrow().as_ref() {
                                    window.maximize();
                                }
                            }}>"□"</Button>
                            <Button on_click={move || {
                                if let Some(window) = close_handle.borrow().as_ref() {
                                    window.close();
                                }
                            }}>"×"</Button>
                        </Flex>
                    </Flex>
                    <Flex direction={FlexDirection::Column} grow={1.0} gap={12.0} padding={32.0} justify={Justify::Center} align={Align::Center}>
                        <Text font_size={28.0}>"No system frame"</Text>
                        <Text color={theme.text_secondary}>"The title bar and window buttons are regular CreamUI widgets."</Text>
                    </Flex>
                </Flex>
            })
        },
    );
}
