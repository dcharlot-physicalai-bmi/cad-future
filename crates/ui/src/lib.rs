//! `physical-ui` — immediate-mode UI framework for OpenIE.
//!
//! Renders with wgpu; no external UI library dependencies.
//! Ported from game-studio's `studio-ui` and adapted for CAD/engineering use.

mod context;
pub mod draw;
pub mod font;
mod renderer;
pub mod theme;
pub mod widgets;
pub mod workspace;
pub mod viewport_header;
pub mod toast;
pub mod shortcut_help;
pub mod perf_hud;
pub mod transform_input;
pub mod viewport_labels;
pub mod menu_bar;
pub mod mate_dialog;
pub mod property_panel;
pub mod timeline;
pub mod marking_menu;
pub mod context_toolbar;
pub mod status_bar_enhanced;
pub mod workspace_switcher;
pub mod flyout_toolbar;
pub mod confirmation_corner;
pub mod snap_indicator;
pub mod color_picker;
pub mod breadcrumb_bar;
pub mod progress_overlay;
pub mod dimension_overlay;
pub mod notification_center;
pub mod quick_access_bar;
pub mod selection_info;
pub mod constraint_icons;
pub mod appearance_browser;
pub mod annotation_tools;
pub mod viewport_splitter;
pub mod feature_tree;
pub mod section_view;
pub mod measure_tool;
pub mod bom_table;
pub mod drawing_view;
// Phase 6
pub mod exploded_view;
pub mod render_settings;
pub mod assembly_browser;
pub mod reference_geometry;
pub mod gdt_panel;
// Phase 7
pub mod sketch_tools;
pub mod hole_wizard;
pub mod sheet_metal;
pub mod pattern_dialog;
pub mod export_dialog;
// Phase 8
pub mod collaboration;
pub mod version_history;
pub mod preferences;
pub mod shortcut_editor;
pub mod data_management;

pub use context::{UiContext, UiStyle};
pub use draw::{DrawList, UiVertex};
pub use font::FontAtlas;
pub use renderer::UiRenderer;
pub use theme::{ThemeMode, ThemeColors};
pub use widgets::{
    TextInputState, DropdownState, ScrollState,
    TooltipState, ContextMenuState, MenuItem,
    CommandPaletteState, Command,
    StatusBarInfo, Toolbar, ToolButton, ToolbarOrientation,
    OutlinerState, OutlinerAction,
};
pub use workspace::{Workspace, PanelKind, LayoutNode, SplitAxis, ResolvedPanel, Rect};
pub use viewport_header::{ViewportHeader, HeaderButton};
pub use toast::{ToastManager, ToastLevel};
pub use shortcut_help::ShortcutHelp;
pub use perf_hud::PerfHud;
pub use transform_input::{TransformInput, TransformMode, AxisConstraint};
pub use viewport_labels::ViewportLabels;
pub use menu_bar::{MenuBar, Menu, MenuItemEntry};
pub use mate_dialog::MateDialog;
pub use property_panel::{PropertyPanel, PropertySection, PropertyEntry};
pub use timeline::{Timeline, TimelineEntry};
pub use marking_menu::{MarkingMenu, MarkingEntry, MarkingSlot};
pub use context_toolbar::{ContextToolbar, ContextButton};
pub use status_bar_enhanced::{EnhancedStatusBar, UnitSystem, SelectionFilter};
pub use workspace_switcher::{WorkspaceSwitcher, WorkspaceMode};
pub use flyout_toolbar::{FlyoutToolbar, FlyoutButton, FlyoutItem};
pub use confirmation_corner::{ConfirmationCorner, OperationType};
pub use snap_indicator::{SnapIndicator, SnapAxis};
pub use color_picker::{ColorPicker, Color};
pub use breadcrumb_bar::{BreadcrumbBar, BreadcrumbSegment};
pub use progress_overlay::{ProgressOverlay, ProgressKind};
pub use dimension_overlay::{DimensionOverlay, DimensionLabel, DimensionKind};
pub use notification_center::{NotificationCenter, Notification, NotificationLevel};
pub use quick_access_bar::{QuickAccessBar, QuickButton};
pub use selection_info::{SelectionInfo, SelectionProperties};
pub use constraint_icons::{ConstraintIcons, ConstraintIcon, ConstraintKind};
pub use appearance_browser::{AppearanceBrowser, AppearanceEntry, AppearanceCategory};
pub use annotation_tools::{AnnotationTools, Annotation, AnnotationType};
pub use viewport_splitter::{ViewportSplitter, ViewportLayout, ViewportPane, PanePreset};
pub use feature_tree::{FeatureTree, Feature, FeatureKind, FeatureStatus};
pub use section_view::{SectionView, SectionPlane};
pub use measure_tool::{MeasureTool, Measurement, MeasureKind};
pub use bom_table::{BomTable, BomRow, BomSortColumn};
pub use drawing_view::{DrawingView, DrawingViewEntry, ProjectionType, SheetSize, TitleBlock};
// Phase 6
pub use exploded_view::{ExplodedView, ExplodeStep, ExplodeDirection};
pub use render_settings::{RenderSettings, EnvironmentPreset, ToneMapping, RenderQuality};
pub use assembly_browser::{AssemblyBrowser, ComponentNode, ComponentKind};
pub use reference_geometry::{ReferenceGeometry, RefGeomItem, RefGeomType, PlaneDefinition};
pub use gdt_panel::{GdtPanel, FeatureControlFrame, GdtCharacteristic, MaterialCondition};
// Phase 7
pub use sketch_tools::{SketchTools, SketchTool};
pub use hole_wizard::{HoleWizard, HoleType, HoleEnd, ThreadStandard, MetricSize};
pub use sheet_metal::{SheetMetal, BendEntry, BendRelief, BendStatus};
pub use pattern_dialog::{PatternDialog, PatternType};
pub use export_dialog::{ExportDialog, ExportFormat, StlOptions, StepOptions};
// Phase 8
pub use collaboration::{Collaboration, Collaborator, Comment};
pub use version_history::{VersionHistory, VersionEntry, VersionKind};
pub use preferences::{Preferences, PrefCategory, UnitPreset};
pub use shortcut_editor::{ShortcutEditor, KeyBinding};
pub use data_management::{DataManagement, ManagedDocument, LifecycleState, CheckoutStatus};
