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

#[cfg(target_arch = "wasm32")]
mod browser;
mod clipboard;
pub use clipboard::read as read_clipboard;
mod components;
mod controller;
pub mod elevation;
mod file_prompt;
mod selected_file;
pub use components::{
    Choice, Icon, IconImage, IconSource, NavigationItem, Surface, SurfaceRole, Symbol,
};
pub use creamui_core::Styled;
pub use file_prompt::FilePickerController;
pub use selected_file::SelectedFile;
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
    page_items, AlertStatus, CalendarColors, CalendarNavigation, CanvasPaint,
    ContextMenuController, ContextMenuItem, ContextMenuPane, ContextMenuStyle, DateTime, PageItem,
    PaginationItemStyles, RawAccordion, RawAlert, RawButton, RawCalendar, RawCanvas, RawCarousel,
    RawCheckbox, RawCollapsible, RawColorPicker, RawContextMenu, RawDateTimePicker,
    RawDescriptionList, RawDropdownButton, RawEmptyState, RawField, RawFilePicker, RawForm,
    RawInputGroup, RawKbd, RawLink, RawListView, RawMarquee, RawNumberInput, RawPagination, RawPre,
    RawQuote, RawRating, RawScrollView, RawScrollbar, RawSheet, RawSidebar, RawSkeleton, RawSlider,
    RawSpinner, RawSplitView, RawStatusBar, RawStepper, RawSwitch, RawTab, RawTable, RawTabs,
    RawTag, RawText, RawTextArea, RawTextInput, RawToast, RawToastHost, RawToolbar, RawTooltip,
    RawTranslate, RawView, RawVirtualList, RawVirtualTable, SheetEdge, SplitAxis, SplitState,
    StepState, TabIndicatorSide, TableColumn, TextSelection, ToastController, ToastEntry,
    ToastStatus, ToolbarCommand, ToolbarController, TooltipController, TooltipPosition,
    VirtualListState,
};
pub use themed::{
    nested_sidebar, tab_styles, Accordion, Alert, AlertDialog, AlertVariant, AppTitlebar, Avatar,
    Badge, Button, ButtonSize, ButtonState, ButtonVariant, Calendar, CaptionButtonView,
    CaptionStyle, Card, Carousel, Checkbox, Collapsible, ColorPicker, ComboBox, CommandPalette,
    CommandPaletteController, CommandPaletteItem, ContextMenu, DateInput, DateTimePicker,
    DescriptionList, Dialog, DialogContent, DialogFooter, DialogHeader, DialogMessage,
    DialogPlacement, DropdownButton, EmptyState, Field, FilePicker, FilePrompt, Form, Heading,
    InputGroup, Kbd, Link, ListBox, ListView, MenuBar, MenuColors, MenuItem, MenuPopup,
    ModalActions, NumberInput, Overlay, Pagination, Popover, Pre, ProgressBar, ProgressRing, Quote,
    Radio, RadioGroup, Rating, ScrollView, SegmentedControl, Select, Sheet, Sidebar, SidebarItem,
    SidebarNode, SidebarSeparator, Skeleton, Slider, Spinner, StatusBar, Stepper, Switch, Tab,
    TabColors, TabSizing, Table, Tabs, Tag, Text, TextArea, TextInput, TextSize, TimeInput, Toast,
    ToastHost, ToggleButton, Toolbar, Tooltip, TreeNode, TreeView, VirtualList, VirtualTable,
    VirtualTreeView,
};
