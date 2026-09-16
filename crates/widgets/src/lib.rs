//! Headless and themed widgets built on `creamui-core`.
//!
//! - [`raw`] contains fully unstyled ("headless") widgets like [`raw::RawButton`].
//! - [`themed`] contains styled wrappers like [`themed::Button`] that read
//!   their geometry from a [`creamui_theme::Theme`] and colours from its
//!   independently swappable [`creamui_theme::ColorScheme`].
//! - [`layout`] has convenience constructors for flex/grid layout styles.

// Both macros route through `Style::merged_over` rather than a wholesale
// field replace: a caller-supplied `style` prop is usually a bare
// `layout::Style` widened to `Style` with every paint/typography field
// unset, and assigning it directly would erase colors, fonts and state
// patches the component already baked in at construction from the theme.
macro_rules! impl_styled_inner {
    ($type:ty) => {
        impl creamui_core::Styled for $type {
            fn set_style(&mut self, style: creamui_core::Style) {
                let merged = creamui_core::Widget::style(&self.inner).merged_over(style);
                creamui_core::Styled::set_style(&mut self.inner, merged);
            }
        }
    };
}

macro_rules! impl_styled_field {
    ($type:ty) => {
        impl creamui_core::Styled for $type {
            fn set_style(&mut self, style: creamui_core::Style) {
                self.style = self.style.clone().merged_over(style);
            }
        }
    };
}

mod components;
mod controller;
pub use components::{
    Choice, Icon, IconImage, IconSource, NavigationItem, Surface, SurfaceRole, Symbol,
};
pub use creamui_core::Styled;
pub mod layout;
pub use layout::CUIWindowDragArea;
pub mod raw;
mod text_metrics;
pub use text_metrics::{clamp_to_lines, row_height_family};
pub mod themed;

pub use controller::{
    AutoScrollController, ColorPickerController, DateTimeController, ScrollController,
    SelectController, SidebarNavController, TabController, TextController, TreeController,
};
pub use raw::{
    DateTime, RawButton, RawCheckbox, RawColorPicker, RawDateTimePicker, RawFilePicker, RawLink,
    RawListView, RawMarquee, RawPre, RawQuote, RawScrollView, RawScrollbar, RawSidebar, RawSlider,
    RawSpinner, RawSwitch, RawTab, RawTable, RawTabs, RawText, RawTextArea, RawTextInput,
    RawTranslate, RawView, RawVirtualList, TabIndicatorSide, TableColumn, TextSelection,
    VirtualListState,
};
pub use themed::{
    nested_sidebar, tab_styles, AlertDialog, Avatar, Badge, Button, ButtonSize, ButtonState,
    ButtonVariant, Card, Checkbox, ColorPicker, ComboBox, DateInput, DateTimePicker, Dialog,
    FilePicker, Heading, Link, ListBox, ListView, MenuBar, MenuColors, MenuItem, MenuPopup,
    Overlay, Popover, Pre, ProgressBar, ProgressRing, Quote, Radio, RadioGroup, ScrollView,
    SegmentedControl, Select, Sidebar, SidebarItem, SidebarNode, SidebarSeparator, Slider, Spinner,
    Switch, Tab, TabColors, TabSizing, Table, Tabs, Text, TextArea, TextInput, TextSize, TimeInput,
    TreeNode, TreeView, TypingIndicator,
};
