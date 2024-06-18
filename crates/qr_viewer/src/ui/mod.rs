pub mod bottom_panel;
pub mod navigation_panels;
pub mod top_panel;

pub(crate) use {
    self::bottom_panel::bottom_panel, self::navigation_panels::navigation_panels,
    self::top_panel::top_panel,
};
