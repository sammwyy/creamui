use creamui_core::{BoxedWidget, Painter, Rect, Style, Widget};
use creamui_reactive::{create_effect, Effect};
use creamui_router::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

struct Empty;
impl Widget for Empty {
    fn style(&self) -> Style { Style::default() }
    fn paint(&self, _: &mut dyn Painter, _: Rect) {}
}
fn empty() -> BoxedWidget { Box::new(Empty) }
struct Fixture { router: Router, _effect: Effect, observed: Rc<RefCell<String>> }
thread_local! { static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) }; }
#[wasm_bindgen]
pub fn start() {
    let router = Router::builder("/ignored").route("/users/:id", empty).build().unwrap();
    let observed = Rc::new(RefCell::new(String::new()));
    let effect = RouterProvider::new(router.clone()).render(|| create_effect({
        let observed = observed.clone();
        move || *observed.borrow_mut() = use_location().url()
    }));
    FIXTURE.with(|slot| *slot.borrow_mut() = Some(Fixture { router, _effect: effect, observed }));
}
fn router() -> Router { FIXTURE.with(|slot| slot.borrow().as_ref().unwrap().router.clone()) }
#[wasm_bindgen]
pub fn navigate(to: &str) -> Result<(), JsValue> { router().navigate(to).map_err(|e| JsValue::from_str(&e.to_string())) }
#[wasm_bindgen]
pub fn replace(to: &str) { router().replace(to).unwrap(); }
#[wasm_bindgen]
pub fn back() { router().back().unwrap(); }
#[wasm_bindgen]
pub fn forward() { router().forward().unwrap(); }
#[wasm_bindgen]
pub fn current_url() -> String { router().url() }
#[wasm_bindgen]
pub fn observed_url() -> String { FIXTURE.with(|slot| slot.borrow().as_ref().unwrap().observed.borrow().clone()) }
#[wasm_bindgen]
pub fn param_id() -> String { router().params().get("id").cloned().unwrap_or_default() }
#[wasm_bindgen]
pub fn can_forward() -> bool { router().can_go_forward() }
#[wasm_bindgen]
pub fn dispose() { FIXTURE.with(|slot| slot.borrow_mut().take()); }
