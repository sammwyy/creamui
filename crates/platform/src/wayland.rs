use crate::{InputSerial, LogicalSize, PopupOptions, PopupPlacement};
use smithay_client_toolkit::{
    compositor::{Surface, SurfaceData},
    error::GlobalError,
    globals::ProvidesBoundGlobal,
    reexports::{
        client::{
            protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_surface},
            Dispatch, QueueHandle,
        },
        protocols::xdg::shell::client::{
            xdg_popup::{self, XdgPopup},
            xdg_positioner, xdg_surface, xdg_wm_base,
        },
    },
    seat::pointer::PointerEventKind,
    shell::xdg::{
        popup::{Popup, PopupData},
        XdgPositioner,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopupPositioner {
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub anchor_width: f32,
    pub anchor_height: f32,
    pub size: LogicalSize,
    pub placement: PopupPlacement,
}

impl PopupPositioner {
    pub fn from_popup(options: &PopupOptions, size: LogicalSize) -> Self {
        Self {
            anchor_x: options.anchor_x,
            anchor_y: options.anchor_y,
            anchor_width: options.anchor_width,
            anchor_height: options.anchor_height,
            size,
            placement: options.placement,
        }
    }

    pub fn apply(self, positioner: &xdg_positioner::XdgPositioner) {
        positioner.set_size(round_size(self.size.width), round_size(self.size.height));
        positioner.set_anchor_rect(
            self.anchor_x.floor() as i32,
            self.anchor_y.floor() as i32,
            round_size(self.anchor_width as f64),
            round_size(self.anchor_height as f64),
        );
        match self.placement {
            PopupPlacement::Above => {
                positioner.set_anchor(xdg_positioner::Anchor::Top);
                positioner.set_gravity(xdg_positioner::Gravity::Top);
            }
            PopupPlacement::Below => {
                positioner.set_anchor(xdg_positioner::Anchor::Bottom);
                positioner.set_gravity(xdg_positioner::Gravity::Bottom);
            }
        }
        positioner.set_constraint_adjustment(
            xdg_positioner::ConstraintAdjustment::SlideX
                | xdg_positioner::ConstraintAdjustment::SlideY
                | xdg_positioner::ConstraintAdjustment::FlipX
                | xdg_positioner::ConstraintAdjustment::FlipY,
        );
        positioner.set_reactive();
    }
}

pub fn grab_popup(popup: &XdgPopup, seat: &WlSeat, serial: InputSerial) {
    popup.grab(seat, serial.0);
}

pub fn press_serial(event: &PointerEventKind) -> Option<InputSerial> {
    match event {
        PointerEventKind::Press { serial, .. } => Some(InputSerial(*serial)),
        _ => None,
    }
}

pub fn create_popup<D>(
    parent: &xdg_surface::XdgSurface,
    geometry: PopupPositioner,
    qh: &QueueHandle<D>,
    compositor: &impl ProvidesBoundGlobal<WlCompositor, 6>,
    wm_base: &(impl ProvidesBoundGlobal<xdg_wm_base::XdgWmBase, 5>
          + ProvidesBoundGlobal<xdg_wm_base::XdgWmBase, 6>),
    grab: Option<(&WlSeat, InputSerial)>,
) -> Result<Popup, GlobalError>
where
    D: Dispatch<wl_surface::WlSurface, SurfaceData>
        + Dispatch<xdg_surface::XdgSurface, PopupData>
        + Dispatch<xdg_popup::XdgPopup, PopupData>
        + 'static,
{
    let positioner = XdgPositioner::new(wm_base)?;
    geometry.apply(&positioner);
    let surface = Surface::new(compositor, qh)?;
    let popup = Popup::from_surface(Some(parent), &positioner, qh, surface, wm_base)?;
    if let Some((seat, serial)) = grab {
        grab_popup(popup.xdg_popup(), seat, serial);
    }
    popup.wl_surface().commit();
    Ok(popup)
}

fn round_size(value: f64) -> i32 {
    value.ceil().max(1.0).min(i32::MAX as f64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_popup_sizes_up_to_valid_protocol_dimensions() {
        assert_eq!(round_size(0.0), 1);
        assert_eq!(round_size(12.1), 13);
    }

    #[test]
    fn reads_only_a_pointer_press_serial() {
        assert_eq!(
            press_serial(&PointerEventKind::Press {
                time: 0,
                button: 1,
                serial: 42,
            }),
            Some(InputSerial(42))
        );
        assert_eq!(
            press_serial(&PointerEventKind::Release {
                time: 0,
                button: 1,
                serial: 42,
            }),
            None
        );
    }
}
