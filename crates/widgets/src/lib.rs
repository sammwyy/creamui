//! Headless and themed widgets built on `creamui-core`.
//!
//! - [`raw`] contains fully unstyled ("headless") widgets like [`raw::RawButton`].
//! - [`themed`] contains styled wrappers like [`themed::Button`] that read
//!   their geometry from a [`creamui_theme::Theme`] and colours from its
//!   independently swappable [`creamui_theme::ColorScheme`].
//! - [`layout`] has convenience constructors for flex/grid layout styles.

macro_rules! impl_styled_inner {
    ($type:ty) => {
        impl creamui_core::Styled for $type {
            fn set_style(&mut self, style: creamui_core::Style) {
                creamui_core::Styled::set_style(&mut self.inner, style);
            }
        }
    };
}

macro_rules! impl_styled_field {
    ($type:ty) => {
        impl creamui_core::Styled for $type {
            fn set_style(&mut self, style: creamui_core::Style) {
                self.style = style;
            }
        }
    };
}

mod components;
mod controller;
pub use components::{Choice, Icon, NavigationItem, Surface, SurfaceRole, Symbol};
pub use creamui_core::Styled;
pub mod layout;
pub use layout::CUIWindowDragArea;
pub mod raw;
mod text_metrics;
pub mod themed;

pub use controller::{
    AutoScrollController, ColorPickerController, DateTimeController, ScrollController,
    SelectController, TabController, TextController, TreeController,
};
pub use raw::{
    DateTime, RawButton, RawCheckbox, RawColorPicker, RawDateTimePicker, RawFilePicker, RawLink,
    RawListView, RawPre, RawQuote, RawScrollView, RawScrollbar, RawSidebar, RawSlider, RawSpinner,
    RawSwitch, RawTab, RawTable, RawTabs, RawText, RawTextArea, RawTextInput, RawView,
    TabIndicatorSide, TableColumn, TextSelection,
};
pub use themed::{
    tab_styles, AlertDialog, Avatar, Badge, Button, ButtonSize, ButtonState, ButtonVariant, Card,
    Checkbox, ColorPicker, ComboBox, DateInput, DateTimePicker, Dialog, FilePicker, Heading, Link,
    ListBox, ListView, MenuBar, MenuColors, MenuItem, MenuPopup, Overlay, Popover, Pre,
    ProgressBar, ProgressRing, Quote, Radio, RadioGroup, ScrollView, SegmentedControl, Select,
    Sidebar, SidebarItem, SidebarSeparator, Slider, Spinner, Switch, Tab, TabColors, TabSizing,
    Table, Tabs, Text, TextArea, TextInput, TextSize, TimeInput, TreeNode, TreeView,
    TypingIndicator,
};
