pub mod cheatsheet;
pub mod containers;
pub mod images;
pub mod inspect_viewer;
pub mod networks;
pub mod system;
pub mod topology;
pub mod volumes;

// the drill-in views (logs, shell, stats, search)
pub mod logs_viewer;
pub mod search_filter;
pub mod shell_executor;
pub mod stats_viewer;

pub use cheatsheet::CheatSheet;
pub use containers::EnhancedContainersTab;
pub use images::ImagesTab;
pub use networks::NetworksTab;
pub use system::SystemTab;
pub use volumes::VolumesTab;

pub use logs_viewer::LogsViewer;
pub use search_filter::AdvancedSearch;
pub use shell_executor::ShellExecutor;
pub use stats_viewer::StatsViewer;
