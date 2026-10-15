use super::{DispatchState, NativeWindow, WindowId};
use smithay_client_toolkit::reexports::{
    client::{globals::GlobalList, protocol::wl_seat::WlSeat, Connection, Dispatch, QueueHandle},
    protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
        zwp_keyboard_shortcuts_inhibit_manager_v1::{self, ZwpKeyboardShortcutsInhibitManagerV1},
        zwp_keyboard_shortcuts_inhibitor_v1::{self, ZwpKeyboardShortcutsInhibitorV1},
    },
};
use std::collections::HashMap;

pub(super) struct ShortcutInhibition {
    manager: ZwpKeyboardShortcutsInhibitManagerV1,
    inhibitors: HashMap<WindowId, ZwpKeyboardShortcutsInhibitorV1>,
}

impl ShortcutInhibition {
    pub(super) fn bind(globals: &GlobalList, qh: &QueueHandle<DispatchState>) -> Option<Self> {
        Some(Self {
            manager: globals.bind(qh, 1..=1, ()).ok()?,
            inhibitors: HashMap::new(),
        })
    }

    pub(super) fn apply(
        &mut self,
        windows: &HashMap<WindowId, NativeWindow>,
        seat: Option<&WlSeat>,
        requests: Vec<(WindowId, bool)>,
        qh: &QueueHandle<DispatchState>,
    ) {
        for (id, inhibit) in requests {
            if !inhibit {
                self.forget(id);
                continue;
            }
            if self.inhibitors.contains_key(&id) {
                continue;
            }
            if let Some((window, seat)) = windows.get(&id).zip(seat) {
                self.inhibitors.insert(
                    id,
                    self.manager
                        .inhibit_shortcuts(window.surface(), seat, qh, ()),
                );
            }
        }
    }

    pub(super) fn forget(&mut self, id: WindowId) {
        if let Some(inhibitor) = self.inhibitors.remove(&id) {
            inhibitor.destroy();
        }
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitManagerV1,
        _: zwp_keyboard_shortcuts_inhibit_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for DispatchState {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        _: zwp_keyboard_shortcuts_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
