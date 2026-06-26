use wasm_bindgen::prelude::*;

use physical_input::{InputMap, KeyCode, MouseButton, TouchState};
use physical_ui::{
    UiContext, UiRenderer, Workspace, DrawList, ThemeMode, ThemeColors,
    StatusBarInfo, Toolbar, ToolButton, ToolbarOrientation,
    CommandPaletteState, Command, ContextMenuState, MenuItem, TooltipState,
    OutlinerState, OutlinerAction, ViewportHeader, HeaderButton,
    ToastManager, ToastLevel, ShortcutHelp,
    PerfHud, TransformInput, TransformMode, AxisConstraint, ViewportLabels,
    MenuBar, Menu, MenuItemEntry, MateDialog,
    PropertyPanel, PropertySection, PropertyEntry,
    Timeline, TimelineEntry,
    MarkingMenu, MarkingEntry, MarkingSlot,
    ContextToolbar, ContextButton,
    EnhancedStatusBar, UnitSystem,
    WorkspaceSwitcher, WorkspaceMode,
    FlyoutToolbar, FlyoutButton, FlyoutItem,
    ConfirmationCorner, OperationType,
    SnapIndicator, SnapAxis,
    ColorPicker,
    BreadcrumbBar, BreadcrumbSegment,
    ProgressOverlay,
    DimensionOverlay, DimensionLabel,
    NotificationCenter, NotificationLevel,
    QuickAccessBar, QuickButton,
    SelectionInfo, SelectionProperties,
    ConstraintIcons, ConstraintIcon, ConstraintKind,
    AppearanceBrowser,
    AnnotationTools, Annotation,
    ViewportSplitter, ViewportLayout,
    FeatureTree, Feature, FeatureKind, FeatureStatus,
    SectionView, SectionPlane,
    MeasureTool, Measurement, MeasureKind,
    BomTable, BomRow,
    DrawingView, TitleBlock,
    // Phase 6
    ExplodedView, ExplodeStep, ExplodeDirection,
    RenderSettings,
    AssemblyBrowser, ComponentNode, ComponentKind,
    ReferenceGeometry, RefGeomItem, RefGeomType,
    GdtPanel, FeatureControlFrame, GdtCharacteristic,
    // Phase 7
    SketchTools, SketchTool,
    HoleWizard, HoleType,
    SheetMetal, BendEntry,
    PatternDialog, PatternType,
    ExportDialog, ExportFormat,
    // Phase 8
    Collaboration, Collaborator, Comment,
    VersionHistory, VersionEntry, VersionKind,
    Preferences,
    ShortcutEditor,
    DataManagement, ManagedDocument, LifecycleState,
};
use physical_viewport::orbit_camera::OrbitCamera;
use physical_viewport::grid::{GridRenderer, GridUniforms};
use physical_viewport::{
    ForwardRenderer, ForwardFrameInput, MeshRegistry, MaterialStore, Material,
    Scene, SceneNode, Gizmo, GizmoMode, GizmoRenderer, WireframeRenderer,
    Measurement, MeasurementOverlay, UndoStack, Action,
    AxesIndicator, NavCube, ViewPreset, ShadingMode,
    OutlineRenderer, ClipPlane,
    MateOp, MateSystem, MateConstraint, compute_mate,
};
use physical_viewport::mesh_registry::builtin;

use physical_cascade::{self as cascade, Value};
use physical_units::*;

use glam::{Mat4, Vec3};

/// Raw input state accumulated between frames.
struct InputState {
    mouse_x: f32,
    mouse_y: f32,
    mouse_down: bool,
    left_just_pressed: bool,
    left_dragging: bool,
    drag_start_x: f32,
    drag_start_y: f32,
    drag_start_transform: Option<Mat4>,
    right_just_pressed: bool,
    /// Box select: start corner for marquee
    box_select_start: Option<(f32, f32)>,
}

/// Demo scene state — a selection of CAD primitives with engineering materials.
struct DemoScene {
    scene: Scene,
    materials: MaterialStore,
    selected_material_idx: usize,
}

impl DemoScene {
    fn new() -> Self {
        let mut materials = MaterialStore::new();

        let _aluminum_id = materials.add(Material::aluminum());
        let _steel_id = materials.add(Material::steel());
        let _titanium_id = materials.add(Material::titanium());
        let _brass_id = materials.add(Material::brass());
        let _plastic_id = materials.add(Material::plastic_white());

        let mut scene = Scene::new();

        scene.add(SceneNode::new("Cube", builtin::CUBE, 1,
            Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0))));

        scene.add(SceneNode::new("Cylinder", builtin::CYLINDER, 2,
            Mat4::from_translation(Vec3::new(3.0, 0.5, 0.0))));

        scene.add(SceneNode::new("Sphere", builtin::SPHERE, 3,
            Mat4::from_translation(Vec3::new(-3.0, 0.5, 0.0))
                * Mat4::from_scale(Vec3::splat(1.5))));

        scene.add(SceneNode::new("Torus", builtin::TORUS, 4,
            Mat4::from_translation(Vec3::new(0.0, 1.0, -3.0))
                * Mat4::from_scale(Vec3::splat(2.0))));

        scene.add(SceneNode::new("Icosphere", builtin::ICOSPHERE, 5,
            Mat4::from_translation(Vec3::new(0.0, 0.5, 3.0))));

        Self {
            scene,
            materials,
            selected_material_idx: 0,
        }
    }
}

const MATERIAL_IDS: &[&str] = &[
    "6061-T6", "7075-T6", "1018", "4140", "304", "316", "Ti-6Al-4V",
];

/// Build the default command palette entries.
fn default_commands() -> Vec<Command> {
    vec![
        Command::new("Undo", "Edit").with_shortcut("Ctrl+Z"),
        Command::new("Redo", "Edit").with_shortcut("Ctrl+Shift+Z"),
        Command::new("Select All", "Edit").with_shortcut("Ctrl+A"),
        Command::new("Deselect", "Edit").with_shortcut("Esc"),
        Command::new("Delete", "Edit").with_shortcut("Del"),
        Command::new("Duplicate", "Edit").with_shortcut("Ctrl+D"),
        Command::new("Toggle Snap", "Edit").with_shortcut("Ctrl+Shift+S"),
        Command::new("Zoom to Fit", "View").with_shortcut("Home"),
        Command::new("Front View", "View").with_shortcut("Num1"),
        Command::new("Right View", "View").with_shortcut("Num3"),
        Command::new("Top View", "View").with_shortcut("Num7"),
        Command::new("Isometric View", "View").with_shortcut("Num0"),
        Command::new("Move Tool", "Tool").with_shortcut("G"),
        Command::new("Rotate Tool", "Tool").with_shortcut("R"),
        Command::new("Scale Tool", "Tool").with_shortcut("S"),
        Command::new("Next Material", "Material"),
        Command::new("Cycle Theme", "View"),
        Command::new("Toggle Wireframe", "View").with_shortcut("Z"),
        Command::new("Toggle Ortho", "View").with_shortcut("Num5"),
        Command::new("Toggle Measurements", "View").with_shortcut("M"),
        Command::new("Toggle Labels", "View").with_shortcut("N"),
        Command::new("Toggle Section Plane", "View").with_shortcut("Ctrl+Shift+C"),
        Command::new("Cycle Clip Axis", "View").with_shortcut("Ctrl+Shift+X"),
        Command::new("Toggle Perf HUD", "View").with_shortcut("Ctrl+Shift+P"),
        Command::new("Toggle Grid", "View"),
    ]
}

/// Build the application menu bar.
fn default_menu_bar() -> MenuBar {
    MenuBar::new(vec![
        Menu::new("File", vec![
            MenuItemEntry::action("New Scene", "file.new").with_shortcut("Ctrl+N"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Export STL", "file.export_stl"),
            MenuItemEntry::action("Export STEP", "file.export_step").disabled(),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Screenshot", "file.screenshot"),
        ]),
        Menu::new("Edit", vec![
            MenuItemEntry::action("Undo", "edit.undo").with_shortcut("Ctrl+Z"),
            MenuItemEntry::action("Redo", "edit.redo").with_shortcut("Ctrl+Shift+Z"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Select All", "edit.select_all").with_shortcut("Ctrl+A"),
            MenuItemEntry::action("Deselect", "edit.deselect").with_shortcut("Esc"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Duplicate", "edit.duplicate").with_shortcut("Ctrl+D"),
            MenuItemEntry::action("Delete", "edit.delete").with_shortcut("Del"),
        ]),
        Menu::new("View", vec![
            MenuItemEntry::action("Front", "view.front").with_shortcut("1"),
            MenuItemEntry::action("Back", "view.back").with_shortcut("2"),
            MenuItemEntry::action("Right", "view.right").with_shortcut("3"),
            MenuItemEntry::action("Left", "view.left").with_shortcut("4"),
            MenuItemEntry::action("Top", "view.top").with_shortcut("7"),
            MenuItemEntry::action("Bottom", "view.bottom").with_shortcut("8"),
            MenuItemEntry::action("Isometric", "view.iso").with_shortcut("9"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Toggle Ortho", "view.ortho").with_shortcut("5"),
            MenuItemEntry::action("Cycle Shading", "view.shading").with_shortcut("Z"),
            MenuItemEntry::action("Toggle Grid", "view.grid"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Measurements", "view.measurements").with_shortcut("M"),
            MenuItemEntry::action("Object Labels", "view.labels").with_shortcut("N"),
            MenuItemEntry::action("Section Plane", "view.section").with_shortcut("Ctrl+Shift+C"),
            MenuItemEntry::action("Perf HUD", "view.perf").with_shortcut("Ctrl+Shift+P"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Zoom to Fit", "view.fit").with_shortcut("Home"),
        ]),
        Menu::new("Insert", vec![
            MenuItemEntry::action("Cube", "insert.cube"),
            MenuItemEntry::action("Sphere", "insert.sphere"),
            MenuItemEntry::action("Cylinder", "insert.cylinder"),
            MenuItemEntry::action("Torus", "insert.torus"),
            MenuItemEntry::action("Icosphere", "insert.icosphere"),
        ]),
        Menu::new("Modify", vec![
            MenuItemEntry::action("Move", "modify.move").with_shortcut("G"),
            MenuItemEntry::action("Rotate", "modify.rotate").with_shortcut("R"),
            MenuItemEntry::action("Scale", "modify.scale").with_shortcut("S"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Reset Transform", "modify.reset"),
            MenuItemEntry::action("Toggle Snap", "modify.snap").with_shortcut("Ctrl+Shift+S"),
        ]),
        Menu::new("Mate", vec![
            MenuItemEntry::action("Mate / Constrain...", "mate.open").with_shortcut("Ctrl+M"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Stack On Top", "mate.stack_top"),
            MenuItemEntry::action("Concentric", "mate.concentric"),
            MenuItemEntry::action("Align X", "mate.align_x"),
            MenuItemEntry::action("Align Z", "mate.align_z"),
            MenuItemEntry::action("Flush +X", "mate.flush_px"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("Solve Constraints", "mate.solve"),
        ]),
        Menu::new("Help", vec![
            MenuItemEntry::action("Keyboard Shortcuts", "help.shortcuts").with_shortcut("F1"),
            MenuItemEntry::action("Command Palette", "help.palette").with_shortcut("Ctrl+K"),
            MenuItemEntry::separator(),
            MenuItemEntry::action("About OpenIE", "help.about"),
        ]),
    ])
}

/// Build the default toolbar buttons.
fn default_toolbar() -> Toolbar {
    let mut tb = Toolbar::new(ToolbarOrientation::Vertical);
    tb.add(ToolButton::new("S", "Select").with_shortcut("Q"));
    tb.add(ToolButton::new("G", "Move").with_shortcut("G"));
    tb.add(ToolButton::new("R", "Rotate").with_shortcut("R"));
    tb.add(ToolButton::new("E", "Scale").with_shortcut("S"));
    tb.buttons[0].active = true; // Select active by default
    tb
}

/// The main application — owns GPU state, camera, UI, input, scene, and cascade.
#[wasm_bindgen]
pub struct CadApp {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    camera: OrbitCamera,
    input: InputMap,
    touch_state: TouchState,
    raw_input: InputState,

    ui_ctx: UiContext,
    ui_renderer: UiRenderer,
    grid_renderer: GridRenderer,
    forward_renderer: ForwardRenderer,
    wireframe_renderer: WireframeRenderer,
    gizmo_renderer: GizmoRenderer,
    mesh_registry: MeshRegistry,
    workspace: Workspace,

    demo: DemoScene,
    selected_node: Option<usize>,
    selection: Vec<usize>,
    undo_stack: UndoStack,

    // Snap
    snap_to_grid: bool,
    grid_snap_size: f32,

    // Viewport overlays
    gizmo: Gizmo,
    axes_indicator: AxesIndicator,
    nav_cube: NavCube,
    measurements: MeasurementOverlay,

    // UI systems
    toolbar: Toolbar,
    status_bar: StatusBarInfo,
    command_palette: CommandPaletteState,
    context_menu: ContextMenuState,
    tooltip: TooltipState,
    outliner: OutlinerState,
    viewport_header: ViewportHeader,
    toasts: ToastManager,
    shortcut_help: ShortcutHelp,
    perf_hud: PerfHud,
    transform_input: TransformInput,
    viewport_labels: ViewportLabels,

    // Menu and mating
    menu_bar: MenuBar,
    mate_dialog: MateDialog,
    mate_system: MateSystem,

    // Phase 1 SOTA features
    property_panel: PropertyPanel,
    timeline: Timeline,
    marking_menu: MarkingMenu,
    context_toolbar: ContextToolbar,
    enhanced_status: EnhancedStatusBar,

    // Phase 2 SOTA features
    workspace_switcher: WorkspaceSwitcher,
    flyout_toolbar: FlyoutToolbar,
    confirmation_corner: ConfirmationCorner,
    snap_indicator: SnapIndicator,
    color_picker: ColorPicker,

    // Phase 3 SOTA features
    breadcrumb_bar: BreadcrumbBar,
    progress_overlay: ProgressOverlay,
    dimension_overlay: DimensionOverlay,
    notification_center: NotificationCenter,
    quick_access_bar: QuickAccessBar,

    // Phase 4 SOTA features
    selection_info: SelectionInfo,
    constraint_icons: ConstraintIcons,
    appearance_browser: AppearanceBrowser,
    annotation_tools: AnnotationTools,
    viewport_splitter: ViewportSplitter,
    feature_tree: FeatureTree,
    section_view: SectionView,
    measure_tool: MeasureTool,
    bom_table: BomTable,
    drawing_view: DrawingView,

    // Phase 6 SOTA features
    exploded_view: ExplodedView,
    render_settings: RenderSettings,
    assembly_browser: AssemblyBrowser,
    reference_geometry: ReferenceGeometry,
    gdt_panel: GdtPanel,

    // Phase 7 SOTA features
    sketch_tools: SketchTools,
    hole_wizard: HoleWizard,
    sheet_metal: SheetMetal,
    pattern_dialog: PatternDialog,
    export_dialog: ExportDialog,

    // Phase 8 SOTA features
    collaboration: Collaboration,
    version_history: VersionHistory,
    preferences: Preferences,
    shortcut_editor: ShortcutEditor,
    data_management: DataManagement,

    // Renderers
    outline_renderer: OutlineRenderer,
    clip_plane: ClipPlane,

    shading_mode: ShadingMode,

    theme_mode: ThemeMode,
    theme: ThemeColors,

    dpr: f32,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
impl CadApp {
    pub async fn new(canvas: web_sys::HtmlCanvasElement, dpr: f32) -> Result<CadApp, JsValue> {
        console_error_panic_hook::set_once();

        let width = (canvas.client_width() as f32 * dpr) as u32;
        let height = (canvas.client_height() as f32 * dpr) as u32;

        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        let instance = wgpu::Instance::new(desc);

        let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
        let surface = instance
            .create_surface(surface_target)
            .map_err(|e| JsValue::from_str(&format!("Surface error: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Adapter error: {e}")))?;

        let mut device_desc = wgpu::DeviceDescriptor::default();
        device_desc.label = Some("openie-device");
        device_desc.required_limits = wgpu::Limits::downlevel_webgl2_defaults();
        let (device, queue) = adapter
            .request_device(&device_desc)
            .await
            .map_err(|e| JsValue::from_str(&format!("Device error: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &surface_config);

        let ui_renderer = UiRenderer::new(&device, &queue, surface_format);
        let grid_renderer = GridRenderer::new(&device, surface_format);
        let forward_renderer = ForwardRenderer::new(&device, surface_format, width, height);
        let wireframe_renderer = WireframeRenderer::new(&device, surface_format);
        let outline_renderer = OutlineRenderer::new(&device, surface_format);
        let gizmo_renderer = GizmoRenderer::new(&device, surface_format);
        let mesh_registry = MeshRegistry::new(&device);
        let camera = OrbitCamera::new(Vec3::new(0.0, 2.0, 0.0), 12.0);
        let input = InputMap::cad_preset();

        let mut workspace = Workspace::new();
        workspace.resolve(width as f32, height as f32);

        // Default to Auto theme — resolves based on time of day
        let theme_mode = ThemeMode::Auto;
        let hour = js_sys::Date::new_0().get_hours();
        let theme = theme_mode.resolve(hour);

        let mut ui_ctx = UiContext::new();
        ui_ctx.apply_theme(&theme);

        Ok(CadApp {
            device,
            queue,
            surface,
            surface_config,
            camera,
            input,
            touch_state: TouchState::new(),
            raw_input: InputState {
                mouse_x: 0.0, mouse_y: 0.0, mouse_down: false,
                left_just_pressed: false, left_dragging: false,
                drag_start_x: 0.0, drag_start_y: 0.0,
                drag_start_transform: None,
                right_just_pressed: false,
                box_select_start: None,
            },
            ui_ctx,
            ui_renderer,
            grid_renderer,
            forward_renderer,
            wireframe_renderer,
            gizmo_renderer,
            mesh_registry,
            workspace,
            demo: DemoScene::new(),
            selected_node: None,
            selection: Vec::new(),
            undo_stack: UndoStack::new(),
            snap_to_grid: true,
            grid_snap_size: 0.5,
            gizmo: Gizmo::new(),
            axes_indicator: AxesIndicator::new(),
            nav_cube: NavCube::new(),
            measurements: {
                let mut m = MeasurementOverlay::new();
                // Demo: distance between cube and cylinder
                m.add(Measurement::distance(
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(3.0, 1.0, 0.0),
                ));
                // Demo: height of sphere
                m.add(Measurement::distance(
                    Vec3::new(-3.0, 0.0, 0.0),
                    Vec3::new(-3.0, 1.5, 0.0),
                ).with_label("1.50").with_color([0.3, 1.0, 0.5, 1.0]));
                m
            },
            toolbar: default_toolbar(),
            status_bar: StatusBarInfo::new(),
            command_palette: CommandPaletteState::new(default_commands()),
            context_menu: ContextMenuState::new(),
            tooltip: TooltipState::new(),
            outliner: OutlinerState::new(),
            viewport_header: ViewportHeader::new(),
            toasts: ToastManager::new(),
            shortcut_help: ShortcutHelp::new(),
            perf_hud: PerfHud::new(),
            transform_input: TransformInput::new(),
            viewport_labels: ViewportLabels::new(),
            menu_bar: default_menu_bar(),
            mate_dialog: MateDialog::new(),
            mate_system: MateSystem::new(),
            property_panel: PropertyPanel::new(),
            timeline: {
                let mut tl = Timeline::new();
                // Seed timeline with initial scene objects
                tl.push(TimelineEntry::new("Cube", "#"));
                tl.push(TimelineEntry::new("Cyl", "O"));
                tl.push(TimelineEntry::new("Sphere", "@"));
                tl.push(TimelineEntry::new("Torus", "~"));
                tl.push(TimelineEntry::new("Ico", "*"));
                tl
            },
            marking_menu: MarkingMenu::new(),
            context_toolbar: ContextToolbar::new(),
            enhanced_status: EnhancedStatusBar::new(),
            workspace_switcher: WorkspaceSwitcher::new(),
            flyout_toolbar: {
                let mut ft = FlyoutToolbar::new(true);
                ft.add(FlyoutButton::new("Select", "tool.select", "S"));
                ft.add(FlyoutButton::new("Move", "modify.move", "G"));
                ft.add(FlyoutButton::new("Insert", "insert", "+").with_flyout(vec![
                    FlyoutItem::new("Cube", "insert.cube", "#"),
                    FlyoutItem::new("Sphere", "insert.sphere", "@"),
                    FlyoutItem::new("Cylinder", "insert.cylinder", "O"),
                    FlyoutItem::separator(),
                    FlyoutItem::new("Torus", "insert.torus", "~"),
                    FlyoutItem::new("Icosphere", "insert.icosphere", "*"),
                ]));
                ft.add(FlyoutButton::new("Modify", "modify", "M").with_flyout(vec![
                    FlyoutItem::new("Move", "modify.move", "G"),
                    FlyoutItem::new("Rotate", "modify.rotate", "R"),
                    FlyoutItem::new("Scale", "modify.scale", "S"),
                    FlyoutItem::separator(),
                    FlyoutItem::new("Reset", "modify.reset", "0"),
                ]));
                ft
            },
            confirmation_corner: ConfirmationCorner::new(),
            snap_indicator: SnapIndicator::new(),
            color_picker: ColorPicker::new(),
            breadcrumb_bar: {
                let mut bb = BreadcrumbBar::new();
                bb.push(BreadcrumbSegment::new("Assembly", "#", 0));
                bb.push(BreadcrumbSegment::new("Design", "@", 1).active());
                bb
            },
            progress_overlay: ProgressOverlay::new(),
            dimension_overlay: {
                let mut d = DimensionOverlay::new();
                // Demo dimensions matching existing measurements
                d.add(DimensionLabel::linear(
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(3.0, 1.0, 0.0),
                    "3.00 m",
                ));
                d.add(DimensionLabel::linear(
                    Vec3::new(-3.0, 0.0, 0.0),
                    Vec3::new(-3.0, 1.5, 0.0),
                    "1.50 m",
                ).with_color([0.3, 1.0, 0.5, 0.85]));
                d
            },
            notification_center: {
                let mut nc = NotificationCenter::new();
                nc.info("Scene loaded — 5 objects", "System", 0.0);
                nc
            },
            quick_access_bar: {
                let mut qab = QuickAccessBar::new();
                qab.set_defaults();
                qab
            },
            selection_info: SelectionInfo::new(),
            constraint_icons: {
                let mut ci = ConstraintIcons::new();
                // Demo: show a few constraint indicators
                ci.add(ConstraintIcon::new([0.0, 1.2, 0.0], ConstraintKind::Fixed));
                ci.add(ConstraintIcon::new([3.0, 1.2, 0.0], ConstraintKind::Horizontal));
                ci.add(ConstraintIcon::new([1.5, 1.0, 0.0], ConstraintKind::Distance)
                    .with_value("3.00 m").between(0, 1));
                ci
            },
            appearance_browser: {
                let mut ab = AppearanceBrowser::new();
                ab.load_defaults();
                ab
            },
            annotation_tools: AnnotationTools::new(),
            viewport_splitter: ViewportSplitter::new(),
            feature_tree: {
                let mut ft = FeatureTree::new();
                ft.add(Feature::new(0, "Origin", FeatureKind::Origin));
                ft.add(Feature::new(0, "Sketch1", FeatureKind::Sketch).with_params("XY Plane"));
                ft.add(Feature::new(0, "Extrude1", FeatureKind::Extrude).with_params("D = 25mm"));
                ft.add(Feature::new(0, "Fillet1", FeatureKind::Fillet).with_params("R = 2mm"));
                ft.add(Feature::new(0, "Sketch2", FeatureKind::Sketch).with_params("Front Plane"));
                ft.add(Feature::new(0, "Hole1", FeatureKind::Hole).with_params("M6 x 1.0"));
                ft.add(Feature::new(0, "Chamfer1", FeatureKind::Chamfer).with_params("1mm x 45°"));
                ft.add(Feature::new(0, "Pattern1", FeatureKind::Pattern).with_params("Circular, 6x"));
                ft
            },
            section_view: SectionView::new(),
            measure_tool: MeasureTool::new(),
            bom_table: {
                let mut bom = BomTable::new();
                bom.add(BomRow::new(0, "PN-001-A", "Main Bracket", 1).with_material("6061-T6", 245.0));
                bom.add(BomRow::new(0, "PN-002-A", "Drive Shaft", 2).with_material("4140", 680.0));
                bom.add(BomRow::new(0, "PN-003-A", "Bearing Housing", 4).with_material("304", 120.0));
                bom.add(BomRow::new(0, "PN-004-A", "Cover Plate", 2).with_material("6061-T6", 85.0));
                bom.add(BomRow::new(0, "STD-M6-20", "M6x20 Socket Head", 12).with_material("12.9", 4.5));
                bom
            },
            drawing_view: DrawingView::new(),

            // Phase 6
            exploded_view: {
                let mut ev = ExplodedView::new();
                ev.add(ExplodeStep::new("Main Bracket", ExplodeDirection::PosZ, 80.0));
                ev.add(ExplodeStep::new("Drive Shaft", ExplodeDirection::PosX, 60.0));
                ev.add(ExplodeStep::new("Bearing Housing", ExplodeDirection::NegZ, 50.0));
                ev.add(ExplodeStep::new("Cover Plate", ExplodeDirection::PosY, 40.0));
                ev
            },
            render_settings: RenderSettings::new(),
            assembly_browser: {
                let mut ab = AssemblyBrowser::new();
                let root = ComponentNode::new(0, "Main Assembly", ComponentKind::Assembly);
                ab.add(root);
                ab.add(ComponentNode::new(1, "Bracket Sub-Assy", ComponentKind::Assembly).with_parent(0));
                ab.add(ComponentNode::new(2, "Main Bracket", ComponentKind::Part).with_parent(1));
                ab.add(ComponentNode::new(3, "Drive Shaft", ComponentKind::Part).with_parent(0));
                ab.add(ComponentNode::new(4, "Bearing Housing", ComponentKind::Part).with_parent(0));
                ab.add(ComponentNode::new(5, "M6x20 SHCS", ComponentKind::StandardPart).with_parent(0).with_instances(12));
                ab
            },
            reference_geometry: ReferenceGeometry::new(),
            gdt_panel: {
                let mut gp = GdtPanel::new();
                gp.add(FeatureControlFrame::new(GdtCharacteristic::Position)
                    .with_tolerance(0.05)
                    .with_datum("A", None)
                    .with_datum("B", None));
                gp.add(FeatureControlFrame::new(GdtCharacteristic::Flatness)
                    .with_tolerance(0.02));
                gp
            },

            // Phase 7
            sketch_tools: SketchTools::new(),
            hole_wizard: HoleWizard::new(),
            sheet_metal: {
                let mut sm = SheetMetal::new();
                sm.add(BendEntry::new(0, 90.0, 2.0, 1.5));
                sm.add(BendEntry::new(1, 45.0, 3.0, 2.0));
                sm
            },
            pattern_dialog: PatternDialog::new(),
            export_dialog: ExportDialog::new(),

            // Phase 8
            collaboration: {
                let mut collab = Collaboration::new();
                collab.users.push(Collaborator::new("You", [0.3, 0.7, 0.9, 1.0]));
                collab.users.push(Collaborator::new("Alice", [0.9, 0.4, 0.3, 1.0]));
                collab.users[1].cursor = Some([400.0, 300.0]);
                collab.users[1].status = "Editing Sketch2".to_string();
                collab
            },
            version_history: {
                let mut vh = VersionHistory::new();
                vh.add(VersionEntry::new("v3", "Initial Design", VersionKind::Version)
                    .with_author("You").with_description("Created base geometry"));
                vh.add(VersionEntry::new("v2", "Added fillets", VersionKind::ManualSave)
                    .with_author("You").with_description("Fillet edges R=2mm"));
                let mut current = VersionEntry::new("v1", "Hole pattern", VersionKind::Version);
                current.current = true;
                vh.add(current.with_author("You").with_description("M6 bolt pattern"));
                vh
            },
            preferences: Preferences::new(),
            shortcut_editor: {
                let mut se = ShortcutEditor::new();
                se.load_defaults();
                se
            },
            data_management: {
                let mut dm = DataManagement::new();
                dm.add(ManagedDocument::new("Main Assembly", "ASM-001"));
                dm.add(ManagedDocument::new("Main Bracket", "PN-001-A"));
                dm.documents[1].state = LifecycleState::InReview;
                dm.add(ManagedDocument::new("Drive Shaft", "PN-002-A"));
                dm
            },

            outline_renderer,
            clip_plane: ClipPlane::new(),
            shading_mode: ShadingMode::Solid,
            theme_mode,
            theme,
            dpr,
            width,
            height,
        })
    }

    // ── Input events from JS ──────────────────────────────────────────

    pub fn key_down(&mut self, code: String) {
        let kc = KeyCode::from(code.as_str());
        self.input.key_down(kc);

        // Command palette intercepts keyboard when open
        if self.command_palette.visible {
            match code.as_str() {
                "Escape" => self.command_palette.close(),
                "ArrowUp" => self.command_palette.select_prev(),
                "ArrowDown" => self.command_palette.select_next(),
                "Enter" => {
                    if let Some(cmd_idx) = self.command_palette.selected_command() {
                        self.execute_command(cmd_idx);
                    }
                    self.command_palette.close();
                }
                "Backspace" => {
                    self.command_palette.query.backspace();
                    self.command_palette.filter();
                }
                _ => {
                    // Single printable chars
                    if code.starts_with("Key") && code.len() == 4 {
                        let c = code.chars().last().unwrap().to_ascii_lowercase();
                        self.command_palette.query.insert_char(c);
                        self.command_palette.filter();
                    } else if code.starts_with("Digit") && code.len() == 6 {
                        let c = code.chars().last().unwrap();
                        self.command_palette.query.insert_char(c);
                        self.command_palette.filter();
                    } else if code == "Space" {
                        self.command_palette.query.insert_char(' ');
                        self.command_palette.filter();
                    } else if code == "Minus" {
                        self.command_palette.query.insert_char('-');
                        self.command_palette.filter();
                    }
                }
            }
            return;
        }

        // Transform numeric input intercept — when user is typing a value
        if self.transform_input.active {
            match code.as_str() {
                "Escape" => {
                    self.transform_input.cancel();
                    self.toasts.info("Transform cancelled");
                }
                "Enter" | "NumpadEnter" => {
                    if let Some(val) = self.transform_input.confirm() {
                        self.apply_transform_value(val);
                    }
                }
                "Backspace" => self.transform_input.backspace(),
                "Minus" | "NumpadSubtract" => self.transform_input.toggle_negative(),
                "KeyX" => self.transform_input.set_axis(AxisConstraint::X),
                "KeyY" => self.transform_input.set_axis(AxisConstraint::Y),
                "KeyZ" => self.transform_input.set_axis(AxisConstraint::Z),
                "Tab" => {
                    let next = self.transform_input.axis.cycle();
                    self.transform_input.set_axis(next);
                }
                _ => {
                    // Digits
                    if code.starts_with("Digit") && code.len() == 6 {
                        let c = code.chars().last().unwrap();
                        self.transform_input.push_char(c);
                    } else if code.starts_with("Numpad") {
                        if let Some(c) = code.strip_prefix("Numpad").and_then(|s| s.chars().next()) {
                            if c.is_ascii_digit() {
                                self.transform_input.push_char(c);
                            }
                        }
                    } else if code == "Period" || code == "NumpadDecimal" {
                        self.transform_input.push_char('.');
                    }
                }
            }
            return;
        }

        // Global shortcuts
        let ctrl = self.input.held("ctrl_mod");
        let shift = self.input.held("shift_mod");

        match code.as_str() {
            // Undo: Ctrl+Z (without Shift)
            "KeyZ" if ctrl && !shift => {
                if let Some(desc) = self.undo_stack.undo(&mut self.demo.scene) {
                    self.toasts.info(&format!("Undo: {}", desc));
                    // Re-validate selection
                    if let Some(idx) = self.selected_node {
                        if idx >= self.demo.scene.len() {
                            self.selected_node = None;
                            self.selection.clear();
                            self.gizmo.visible = false;
                        }
                    }
                }
            }
            // Redo: Ctrl+Shift+Z or Ctrl+Y
            "KeyZ" if ctrl && shift => {
                if let Some(desc) = self.undo_stack.redo(&mut self.demo.scene) {
                    self.toasts.info(&format!("Redo: {}", desc));
                }
            }
            "KeyY" if ctrl => {
                if let Some(desc) = self.undo_stack.redo(&mut self.demo.scene) {
                    self.toasts.info(&format!("Redo: {}", desc));
                }
            }
            // Duplicate: Ctrl+D / Shift+D
            "KeyD" if ctrl || shift => {
                self.duplicate_selected();
            }
            // Select all: Ctrl+A
            "KeyA" if ctrl => {
                self.selection.clear();
                for i in 0..self.demo.scene.len() {
                    self.selection.push(i);
                }
                if let Some(&last) = self.selection.last() {
                    self.selected_node = Some(last);
                    let pos = self.demo.scene.transform(last).col(3).truncate();
                    self.gizmo.position = pos;
                    self.gizmo.visible = true;
                }
            }
            // Snap toggle: Ctrl+Shift+S
            "KeyS" if ctrl && shift => {
                self.snap_to_grid = !self.snap_to_grid;
            }
            // Mate dialog: Ctrl+M
            "KeyM" if ctrl => {
                if self.mate_dialog.visible {
                    self.mate_dialog.close();
                } else {
                    self.mate_dialog.open();
                    // If we already have a selection, set it as object A
                    if let Some(idx) = self.selected_node {
                        let name = self.demo.scene.node(idx).name.clone();
                        self.mate_dialog.set_object_a(idx, &name);
                    }
                }
            }
            // Command palette: Ctrl+K or F3
            "KeyK" if ctrl => self.command_palette.open(),
            "F3" => self.command_palette.open(),
            // View presets — standard numpad layout
            "Numpad1" | "Digit1" => self.apply_view_preset(ViewPreset::Front),
            "Numpad2" | "Digit2" => self.apply_view_preset(ViewPreset::Back),
            "Numpad3" | "Digit3" => self.apply_view_preset(ViewPreset::Right),
            "Numpad4" | "Digit4" => self.apply_view_preset(ViewPreset::Left),
            "Numpad7" | "Digit7" => self.apply_view_preset(ViewPreset::Top),
            "Numpad8" | "Digit8" => self.apply_view_preset(ViewPreset::Bottom),
            "Numpad9" | "Digit9" => self.apply_view_preset(ViewPreset::Iso),
            // Focus on selected (numpad .)
            "NumpadDecimal" | "Period" if !ctrl => {
                if let Some(idx) = self.selected_node {
                    let pos = self.demo.scene.transform(idx).col(3).truncate();
                    self.camera.focus_on(pos, 6.0);
                }
            }
            // Help overlay
            "F1" => {
                self.shortcut_help.toggle();
            }
            // Gizmo mode shortcuts — also start numeric input if object selected
            "KeyG" if !ctrl => {
                self.set_gizmo_mode(GizmoMode::Translate);
                if self.selected_node.is_some() {
                    self.transform_input.begin(TransformMode::Translate);
                }
            }
            "KeyR" if !ctrl => {
                self.set_gizmo_mode(GizmoMode::Rotate);
                if self.selected_node.is_some() {
                    self.transform_input.begin(TransformMode::Rotate);
                }
            }
            "KeyS" if !ctrl && !shift => {
                self.set_gizmo_mode(GizmoMode::Scale);
                if self.selected_node.is_some() {
                    self.transform_input.begin(TransformMode::Scale);
                }
            }
            "KeyQ" => self.set_tool_active(0), // Select
            // Wireframe toggle
            "KeyZ" if !ctrl => {
                self.shading_mode = self.shading_mode.cycle();
            }
            // Ortho/perspective toggle
            "Numpad5" | "Digit5" => {
                self.camera.toggle_ortho();
            }
            // Toggle measurements
            "KeyM" if !ctrl => {
                self.measurements.visible = !self.measurements.visible;
            }
            // Performance HUD: Ctrl+Shift+P
            "KeyP" if ctrl && shift => {
                self.perf_hud.toggle();
            }
            // Cross-section clip plane: Ctrl+Shift+C
            "KeyC" if ctrl && shift => {
                self.clip_plane.toggle();
                let state = if self.clip_plane.enabled { "ON" } else { "OFF" };
                self.toasts.info(&format!("Section plane: {}", state));
            }
            // Cycle clip axis: Ctrl+Shift+X
            "KeyX" if ctrl && shift => {
                if self.clip_plane.enabled {
                    self.clip_plane.axis = self.clip_plane.axis.cycle();
                    self.toasts.info(&format!("Clip axis: {}", self.clip_plane.axis.name()));
                }
            }
            // Flip clip plane: Ctrl+Shift+F
            "KeyF" if ctrl && shift => {
                if self.clip_plane.enabled {
                    self.clip_plane.flip();
                    self.toasts.info("Clip plane flipped");
                }
            }
            // Viewport labels toggle: N
            "KeyN" => {
                self.viewport_labels.toggle();
            }
            // Delete selected (with undo)
            "Delete" | "Backspace" => {
                self.delete_selected();
            }
            // Property panel toggle: P
            "KeyP" if !ctrl && !shift => {
                self.property_panel.toggle();
                if self.property_panel.visible {
                    self.toasts.info("Properties panel");
                }
            }
            // Selection filter toggle: F5
            "F5" => {
                self.enhanced_status.filter_visible = !self.enhanced_status.filter_visible;
            }
            // Color picker toggle: C (without modifiers)
            "KeyC" if !ctrl && !shift => {
                self.color_picker.visible = !self.color_picker.visible;
            }
            // Notification center toggle: Ctrl+Shift+N
            "KeyN" if ctrl && shift => {
                self.notification_center.toggle();
            }
            // Dimension overlay toggle: Ctrl+Shift+D
            "KeyD" if ctrl && shift => {
                self.dimension_overlay.toggle();
            }
            // Selection info panel: I (without modifiers)
            "KeyI" if !ctrl && !shift => {
                if self.selected_node.is_some() {
                    self.selection_info.toggle();
                }
            }
            // Appearance browser: Ctrl+Shift+A
            "KeyA" if ctrl && shift => {
                self.appearance_browser.toggle();
            }
            // Viewport splitter cycle: Ctrl+Shift+V
            "KeyV" if ctrl && shift => {
                self.viewport_splitter.cycle_layout();
                self.toasts.info(&format!("Layout: {}", self.viewport_splitter.layout.label()));
            }
            // Feature tree toggle: F
            "KeyF" if !ctrl && !shift => {
                self.feature_tree.visible = !self.feature_tree.visible;
            }
            // Section view toggle: Ctrl+Shift+X
            "KeyX" if ctrl && shift => {
                self.section_view.toggle();
                let state = if self.section_view.active { "ON" } else { "OFF" };
                self.toasts.info(&format!("Section view: {}", state));
            }
            // Measure tool toggle: M
            "KeyM" if !ctrl && !shift => {
                self.measure_tool.toggle();
                let state = if self.measure_tool.active { "ON" } else { "OFF" };
                self.toasts.info(&format!("Measure tool: {}", state));
            }
            // BOM table toggle: Ctrl+B
            "KeyB" if ctrl && !shift => {
                self.bom_table.toggle();
            }
            // Drawing view toggle: Ctrl+Shift+W
            "KeyW" if ctrl && shift => {
                self.drawing_view.toggle();
                if self.drawing_view.active && self.drawing_view.views.is_empty() {
                    self.drawing_view.add_standard_views();
                    self.drawing_view.title_block.title = "Assembly Drawing".to_string();
                    self.drawing_view.title_block.part_number = "ASM-001".to_string();
                    self.drawing_view.title_block.company = "OpenIE".to_string();
                    self.drawing_view.title_block.material = "Various".to_string();
                }
            }
            // Exploded view toggle: Ctrl+Shift+E
            "KeyE" if ctrl && shift => {
                self.exploded_view.toggle();
            }
            // Render settings toggle: Ctrl+Shift+R
            "KeyR" if ctrl && shift => {
                self.render_settings.toggle();
            }
            // Assembly browser toggle: A (no modifiers)
            "KeyA" if !ctrl && !shift => {
                self.assembly_browser.toggle();
            }
            // Reference geometry: Ctrl+Shift+G
            "KeyG" if ctrl && shift => {
                self.reference_geometry.toggle();
            }
            // GDT panel: Ctrl+Shift+D
            "KeyD" if ctrl && shift => {
                self.gdt_panel.toggle();
            }
            // Sketch tools toggle: S (no modifiers)
            "KeyS" if !ctrl && !shift => {
                self.sketch_tools.toggle();
            }
            // Hole wizard: H (no modifiers)
            "KeyH" if !ctrl && !shift => {
                self.hole_wizard.toggle();
            }
            // Sheet metal: Ctrl+Shift+M
            "KeyM" if ctrl && shift => {
                self.sheet_metal.toggle();
            }
            // Pattern dialog: Ctrl+Shift+P
            "KeyP" if ctrl && shift => {
                self.pattern_dialog.toggle();
            }
            // Export dialog: Ctrl+E
            "KeyE" if ctrl && !shift => {
                self.export_dialog.toggle();
            }
            // Collaboration panel: Ctrl+Shift+L
            "KeyL" if ctrl && shift => {
                self.collaboration.toggle();
            }
            // Version history: Ctrl+Shift+H
            "KeyH" if ctrl && shift => {
                self.version_history.toggle();
            }
            // Preferences: Ctrl+Comma (typical) — use Ctrl+Shift+,
            "Comma" if ctrl => {
                self.preferences.toggle();
            }
            // Shortcut editor: Ctrl+K Ctrl+S pattern — simplified to Ctrl+Shift+K
            "KeyK" if ctrl && shift => {
                self.shortcut_editor.toggle();
            }
            // Data management: Ctrl+Shift+F
            "KeyF" if ctrl && shift => {
                self.data_management.toggle();
            }
            // Section plane cycle: Ctrl+Shift+C (when section view active)
            "KeyC" if ctrl && shift && self.section_view.active => {
                self.section_view.cycle_plane();
                self.toasts.info(&format!("Section: {}", self.section_view.plane.label()));
            }
            // Confirm active operation: Enter
            "Enter" | "NumpadEnter" if self.confirmation_corner.active => {
                self.confirmation_corner.end();
                self.toasts.success("Operation confirmed");
            }
            // Deselect / close overlays
            "Escape" => {
                if self.shortcut_editor.visible {
                    self.shortcut_editor.toggle();
                } else if self.preferences.visible {
                    self.preferences.toggle();
                } else if self.export_dialog.visible {
                    self.export_dialog.toggle();
                } else if self.hole_wizard.visible {
                    self.hole_wizard.toggle();
                } else if self.pattern_dialog.visible {
                    self.pattern_dialog.toggle();
                } else if self.render_settings.visible {
                    self.render_settings.toggle();
                } else if self.drawing_view.active {
                    self.drawing_view.active = false;
                } else if self.bom_table.visible {
                    self.bom_table.visible = false;
                } else if self.measure_tool.active {
                    self.measure_tool.toggle();
                } else if self.section_view.active {
                    self.section_view.toggle();
                } else if self.marking_menu.visible {
                    self.marking_menu.close();
                } else if self.color_picker.visible {
                    self.color_picker.visible = false;
                } else if self.confirmation_corner.active {
                    self.confirmation_corner.end();
                    self.toasts.info("Operation cancelled");
                } else if self.shortcut_help.visible {
                    self.shortcut_help.visible = false;
                } else if self.property_panel.visible {
                    self.property_panel.close();
                } else if self.context_menu.visible {
                    self.context_menu.close();
                } else {
                    self.selected_node = None;
                    self.selection.clear();
                    self.gizmo.visible = false;
                    self.context_toolbar.hide();
                }
            }
            _ => {}
        }
    }

    pub fn key_up(&mut self, code: String) {
        self.input.key_up(KeyCode::from(code.as_str()));
    }

    pub fn mouse_move(&mut self, x: f32, y: f32, dx: f32, dy: f32) {
        self.raw_input.mouse_x = x * self.dpr;
        self.raw_input.mouse_y = y * self.dpr;
        self.input.mouse_move(dx, dy);

        // Menu bar hover
        self.menu_bar.handle_hover(self.raw_input.mouse_x, self.raw_input.mouse_y);

        // Mate dialog hover
        if self.mate_dialog.visible && (self.mate_dialog.step == physical_ui::mate_dialog::MateStep::ChooseMate
            || self.mate_dialog.step == physical_ui::mate_dialog::MateStep::EditParams)
        {
            let px = (self.width as f32 - 340.0) * 0.5;
            let py = (self.height as f32 - 520.0) * 0.5;
            self.mate_dialog.hovered_op = self.mate_dialog.hit_test_ops(
                self.raw_input.mouse_x, self.raw_input.mouse_y, px, py,
            );
        }

        // Marking menu hover update
        if self.marking_menu.visible {
            self.marking_menu.update_hover(self.raw_input.mouse_x, self.raw_input.mouse_y);
        }

        // Quick access bar hover
        self.quick_access_bar.hovered = self.quick_access_bar.hit_test(
            self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, 0.0,
        );

        // Breadcrumb bar hover
        {
            let bb_y = self.quick_access_bar.height + self.menu_bar.height + 24.0
                + self.workspace_switcher.height;
            self.breadcrumb_bar.hovered = self.breadcrumb_bar.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, bb_y,
            );
        }

        // Progress overlay cancel hover
        if self.progress_overlay.visible {
            self.progress_overlay.cancel_hovered = self.progress_overlay.hit_test_cancel(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
                self.width as f32, self.height as f32,
            );
        }

        // Color picker drag update
        if self.color_picker.visible && self.color_picker.dragging.is_some() {
            self.color_picker.handle_mouse(
                self.raw_input.mouse_x, self.raw_input.mouse_y, true,
            );
        }

        // Workspace switcher hover
        let viewport_header_h = 24.0_f32; // ViewportHeader bar_h constant
        self.workspace_switcher.hovered = self.workspace_switcher.hit_test(
            self.raw_input.mouse_x, self.raw_input.mouse_y,
            0.0, self.menu_bar.height + viewport_header_h,
        );

        // Flyout toolbar hover
        {
            let ft_x = 4.0;
            let ft_y = self.menu_bar.height + 30.0 + self.workspace_switcher.height;
            self.flyout_toolbar.hovered_button = self.flyout_toolbar.hit_test_button(
                self.raw_input.mouse_x, self.raw_input.mouse_y, ft_x, ft_y,
            );
            if self.flyout_toolbar.open_flyout.is_some() {
                self.flyout_toolbar.hovered_item = self.flyout_toolbar.hit_test_flyout(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, ft_x, ft_y,
                );
            }
        }

        // Feature tree hover
        if self.feature_tree.visible {
            let ft_panel_x = 0.0;
            let ft_panel_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            self.feature_tree.hovered = self.feature_tree.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y, ft_panel_x, ft_panel_y,
            );
        }

        // Section view slider drag
        if self.section_view.dragging {
            self.section_view.handle_slider_drag(
                self.raw_input.mouse_x,
                self.width as f32 - self.section_view.panel_width - 8.0,
            );
        }

        // BOM table hover
        if self.bom_table.visible {
            let bom_x = (self.width as f32 - self.bom_table.width) * 0.5;
            let bom_y = (self.height as f32 - 400.0) * 0.5;
            self.bom_table.hovered = self.bom_table.hit_test_row(
                self.raw_input.mouse_x, self.raw_input.mouse_y, bom_x, bom_y,
            );
        }

        // Drawing view hover
        if self.drawing_view.active {
            self.drawing_view.hovered_view = self.drawing_view.hit_test_view(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
            );
        }

        // Context toolbar hover
        if self.context_toolbar.visible {
            self.context_toolbar.hovered = self.context_toolbar.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
            );
        }

        // Viewport header hover
        self.viewport_header.hovered = self.viewport_header.hit_test(
            self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, self.menu_bar.height,
        );

        // Toolbar hover detection
        let tb_x = 4.0;
        let tb_y = 30.0;
        self.toolbar.hovered = self.toolbar.hit_test(
            self.raw_input.mouse_x, self.raw_input.mouse_y, tb_x, tb_y
        );

        // Context menu hover
        if self.context_menu.visible {
            self.context_menu.hovered_item = self.context_menu.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y
            );
        }

        // Right-click drag: orbit, Middle-click drag: pan
        if self.input.held("orbit") && !self.raw_input.right_just_pressed {
            self.camera.rotate(dx * 0.005, dy * 0.005);
        }
        if self.input.held("pan") {
            self.camera.pan(-dx, dy);
        }

        // Left-click drag: box select when no object selected, move when selected
        if self.input.held("select") && self.selected_node.is_none() {
            let dist = ((self.raw_input.mouse_x - self.raw_input.drag_start_x).powi(2)
                + (self.raw_input.mouse_y - self.raw_input.drag_start_y).powi(2))
                .sqrt();
            if dist > 4.0 && self.raw_input.box_select_start.is_none() {
                self.raw_input.box_select_start = Some((
                    self.raw_input.drag_start_x,
                    self.raw_input.drag_start_y,
                ));
            }
        }

        // Left-click drag: move selected object on XZ ground plane
        if self.input.held("select") && self.selected_node.is_some() {
            let dist = ((self.raw_input.mouse_x - self.raw_input.drag_start_x).powi(2)
                + (self.raw_input.mouse_y - self.raw_input.drag_start_y).powi(2))
                .sqrt();
            if dist > 4.0 && !self.raw_input.left_dragging {
                self.raw_input.left_dragging = true;
                // Capture start transform for undo
                if let Some(idx) = self.selected_node {
                    self.raw_input.drag_start_transform = Some(self.demo.scene.transform(idx));
                }
            }
            if self.raw_input.left_dragging {
                if let Some(idx) = self.selected_node {
                    let obj_y = self.demo.scene.transform(idx).col(3).y;
                    let (origin, dir) = self.camera.screen_to_ray(
                        self.raw_input.mouse_x,
                        self.raw_input.mouse_y,
                        self.width as f32,
                        self.height as f32,
                    );
                    if let Some(hit) = OrbitCamera::ray_ground_intersect(origin, dir, obj_y) {
                        let old = self.demo.scene.transform(idx);
                        let scale = Vec3::new(
                            old.col(0).truncate().length(),
                            old.col(1).truncate().length(),
                            old.col(2).truncate().length(),
                        );
                        let snapped = self.snap_position(Vec3::new(hit.x, obj_y, hit.z));
                        let new_transform = Mat4::from_translation(snapped)
                            * Mat4::from_scale(scale);
                        self.demo.scene.set_transform(idx, new_transform);
                    }
                }
            }
        }

        // Update status bar coordinates
        let (origin, dir) = self.camera.screen_to_ray(
            self.raw_input.mouse_x, self.raw_input.mouse_y,
            self.width as f32, self.height as f32,
        );
        if let Some(hit) = OrbitCamera::ray_ground_intersect(origin, dir, 0.0) {
            self.status_bar.coordinates = Some(format!(
                "X:{:.1}  Y:{:.1}  Z:{:.1}", hit.x, 0.0, hit.z
            ));
        }
    }

    pub fn mouse_down(&mut self, button: u8) {
        self.raw_input.mouse_down = true;
        self.input.mouse_button_down(MouseButton::from(button));

        if button == 0 {
            self.raw_input.left_just_pressed = true;
            self.raw_input.drag_start_x = self.raw_input.mouse_x;
            self.raw_input.drag_start_y = self.raw_input.mouse_y;

            // Quick access bar click
            if let Some(btn_idx) = self.quick_access_bar.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
                0.0, 0.0, // drawn at top
            ) {
                if let Some(action_id) = self.quick_access_bar.handle_click(btn_idx) {
                    self.execute_menu_action(action_id);
                }
                self.raw_input.left_just_pressed = false;
            }

            // Breadcrumb bar click
            {
                let bb_y = self.quick_access_bar.height + self.menu_bar.height + 24.0
                    + self.workspace_switcher.height;
                if let Some(seg_idx) = self.breadcrumb_bar.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, bb_y,
                ) {
                    // Navigate up to that level
                    while self.breadcrumb_bar.segments.len() > seg_idx + 1 {
                        self.breadcrumb_bar.pop();
                    }
                    if let Some(last) = self.breadcrumb_bar.segments.last_mut() {
                        last.active = true;
                    }
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Notification center badge click
            {
                let badge_x = self.width as f32 - 40.0;
                let badge_y = self.quick_access_bar.height;
                if self.notification_center.hit_test_badge(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, badge_x, badge_y,
                ) {
                    self.notification_center.toggle();
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Progress overlay cancel click
            if self.progress_overlay.visible {
                if self.progress_overlay.hit_test_cancel(
                    self.raw_input.mouse_x, self.raw_input.mouse_y,
                    self.width as f32, self.height as f32,
                ) {
                    self.progress_overlay.cancel_requested = true;
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Workspace switcher click
            {
                let ws_y = self.menu_bar.height + 24.0; // viewport header height
                if let Some(tab_idx) = self.workspace_switcher.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, ws_y,
                ) {
                    let mode = WorkspaceMode::all()[tab_idx];
                    self.workspace_switcher.set_workspace(mode);
                    self.toasts.info(&format!("Workspace: {}", mode.label()));
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Flyout toolbar click
            {
                let ft_x = 4.0;
                let ft_y = self.menu_bar.height + 30.0 + self.workspace_switcher.height;
                // Flyout item click first
                if self.flyout_toolbar.open_flyout.is_some() {
                    if let Some(item_idx) = self.flyout_toolbar.hit_test_flyout(
                        self.raw_input.mouse_x, self.raw_input.mouse_y, ft_x, ft_y,
                    ) {
                        if let Some(action_id) = self.flyout_toolbar.handle_flyout_click(item_idx) {
                            self.execute_menu_action(action_id);
                        }
                        self.raw_input.left_just_pressed = false;
                    }
                }
                // Button click
                if let Some(btn_idx) = self.flyout_toolbar.hit_test_button(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, ft_x, ft_y,
                ) {
                    if let Some(action_id) = self.flyout_toolbar.handle_button_click(btn_idx) {
                        self.execute_menu_action(action_id);
                    }
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Confirmation corner click
            if self.confirmation_corner.active {
                if let Some(btn) = self.confirmation_corner.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, self.width as f32,
                ) {
                    match btn {
                        0 => {
                            self.confirmation_corner.end();
                            self.toasts.success("Operation confirmed");
                        }
                        1 => {
                            self.confirmation_corner.end();
                            self.toasts.info("Operation cancelled");
                        }
                        _ => {}
                    }
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Color picker click
            if self.color_picker.visible {
                if self.color_picker.handle_mouse(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, true,
                ) {
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Feature tree click
            if self.feature_tree.visible {
                let ft_panel_x = 0.0;
                let ft_panel_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                    + self.breadcrumb_bar.height;
                if let Some(idx) = self.feature_tree.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, ft_panel_x, ft_panel_y,
                ) {
                    self.feature_tree.selected = Some(idx);
                    let name = self.feature_tree.features[idx].name.clone();
                    self.toasts.info(&format!("Feature: {}", name));
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Section view slider click
            if self.section_view.active {
                let sv_panel_x = self.width as f32 - self.section_view.panel_width - 8.0;
                let sv_panel_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
                if self.section_view.hit_test_slider(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, sv_panel_x, sv_panel_y,
                ) {
                    self.section_view.dragging = true;
                    self.section_view.handle_slider_drag(self.raw_input.mouse_x, sv_panel_x);
                    self.raw_input.left_just_pressed = false;
                }
            }

            // BOM table row click
            if self.bom_table.visible {
                let bom_x = (self.width as f32 - self.bom_table.width) * 0.5;
                let bom_y = (self.height as f32 - 400.0) * 0.5;
                if let Some(idx) = self.bom_table.hit_test_row(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, bom_x, bom_y,
                ) {
                    self.bom_table.selected = Some(idx);
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Drawing view click
            if self.drawing_view.active {
                if let Some(idx) = self.drawing_view.hit_test_view(
                    self.raw_input.mouse_x, self.raw_input.mouse_y,
                ) {
                    self.drawing_view.selected_view = Some(idx);
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Context toolbar click
            if self.context_toolbar.visible {
                if let Some(action_id) = self.context_toolbar.handle_click(
                    self.raw_input.mouse_x, self.raw_input.mouse_y,
                ) {
                    self.execute_menu_action(action_id);
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Enhanced status bar click
            if let Some(hit) = self.enhanced_status.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
                self.width as f32, self.height as f32,
            ) {
                match hit {
                    "unit" => {
                        self.enhanced_status.unit_system = self.enhanced_status.unit_system.cycle();
                        self.toasts.info(&format!("Units: {}", self.enhanced_status.unit_system.label()));
                    }
                    "filter_V" => self.enhanced_status.selection_filter.vertices = !self.enhanced_status.selection_filter.vertices,
                    "filter_E" => self.enhanced_status.selection_filter.edges = !self.enhanced_status.selection_filter.edges,
                    "filter_F" => self.enhanced_status.selection_filter.faces = !self.enhanced_status.selection_filter.faces,
                    "filter_B" => self.enhanced_status.selection_filter.bodies = !self.enhanced_status.selection_filter.bodies,
                    _ => {}
                }
                self.raw_input.left_just_pressed = false;
            }

            // Property panel section collapse
            if self.property_panel.visible {
                let top_y = self.menu_bar.height + 30.0;
                if let Some(sec_idx) = self.property_panel.hit_test_header(
                    self.raw_input.mouse_x, self.raw_input.mouse_y,
                    self.width as f32, top_y,
                ) {
                    self.property_panel.toggle_section(sec_idx);
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Timeline click
            if self.timeline.visible {
                let tl_y = self.height as f32 - self.enhanced_status.height - self.timeline.height;
                if let Some(entry_idx) = self.timeline.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y,
                    0.0, tl_y,
                ) {
                    // Click on timeline entry: select that object
                    if entry_idx < self.demo.scene.len() {
                        self.selected_node = Some(entry_idx);
                        let pos = self.demo.scene.transform(entry_idx).col(3).truncate();
                        self.gizmo.position = pos;
                        self.gizmo.visible = true;
                    }
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Menu bar click
            if let Some(action_id) = self.menu_bar.handle_click(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
            ) {
                self.execute_menu_action(action_id);
                self.raw_input.left_just_pressed = false;
            }
            // If menu is open, consume the click
            if self.menu_bar.open_menu.is_some() {
                self.raw_input.left_just_pressed = false;
            }

            // Mate dialog object picking
            if self.mate_dialog.visible {
                use physical_ui::mate_dialog::MateStep;
                let mx = self.raw_input.mouse_x;
                let my = self.raw_input.mouse_y;
                let panel_w = 340.0;
                let panel_h = 520.0;
                let px = (self.width as f32 - panel_w) * 0.5;
                let py = (self.height as f32 - panel_h) * 0.5;

                // Check if click is inside dialog
                let in_dialog = mx >= px && mx <= px + panel_w && my >= py && my <= py + panel_h;

                if in_dialog {
                    // Check mate op click
                    if let Some(op_idx) = self.mate_dialog.hit_test_ops(mx, my, px, py) {
                        self.mate_dialog.selected_op = op_idx;
                        if self.mate_dialog.operations[op_idx].needs_param {
                            self.mate_dialog.step = MateStep::EditParams;
                        }
                    }

                    // Check Apply button
                    let apply_x = px + panel_w - 160.0;
                    let apply_y = py + panel_h - 32.0;
                    if mx >= apply_x && mx < apply_x + 70.0 && my >= apply_y && my < apply_y + 24.0 {
                        self.apply_mate_from_dialog();
                    }

                    // Check Cancel button
                    let cancel_x = px + panel_w - 82.0;
                    if mx >= cancel_x && mx < cancel_x + 70.0 && my >= apply_y && my < apply_y + 24.0 {
                        self.mate_dialog.close();
                    }

                    self.raw_input.left_just_pressed = false;
                } else if self.mate_dialog.step == MateStep::PickObjectA
                    || self.mate_dialog.step == MateStep::PickObjectB
                {
                    // Pick object by clicking in viewport
                    let (origin, dir) = self.camera.screen_to_ray(
                        mx, my, self.width as f32, self.height as f32,
                    );
                    if let Some(idx) = self.demo.scene.pick(origin, dir) {
                        let name = self.demo.scene.node(idx).name.clone();
                        if self.mate_dialog.step == MateStep::PickObjectA {
                            self.mate_dialog.set_object_a(idx, &name);
                        } else {
                            self.mate_dialog.set_object_b(idx, &name);
                        }
                    }
                    self.raw_input.left_just_pressed = false;
                }
            }

            // Close context menu on left click
            if self.context_menu.visible {
                if let Some(item_idx) = self.context_menu.hit_test(
                    self.raw_input.mouse_x, self.raw_input.mouse_y
                ) {
                    self.execute_context_menu_item(item_idx);
                }
                self.context_menu.close();
            }

            // Viewport header click (offset by menu bar height)
            if let Some(btn_idx) = self.viewport_header.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, self.menu_bar.height,
            ) {
                match btn_idx {
                    0 => self.shading_mode = ShadingMode::Solid,
                    1 => self.shading_mode = ShadingMode::Wireframe,
                    2 => self.shading_mode = ShadingMode::SolidWireframe,
                    4 => self.camera.toggle_ortho(),
                    6 => self.snap_to_grid = !self.snap_to_grid,
                    _ => {} // separator
                }
                self.raw_input.left_just_pressed = false;
            }

            // Close command palette on click outside
            if self.command_palette.visible {
                // Simple: just close
                self.command_palette.close();
            }

            // Toolbar click
            let tb_x = 4.0;
            let tb_y = 30.0;
            if let Some(btn_idx) = self.toolbar.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y, tb_x, tb_y
            ) {
                self.set_tool_active(btn_idx);
                self.raw_input.left_just_pressed = false; // consume click
            }

            // Nav cube click
            if let Some(preset) = self.nav_cube.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
                self.width as f32, self.height as f32,
            ) {
                self.apply_view_preset(preset);
                self.raw_input.left_just_pressed = false;
            }

            // Outliner click
            let outliner_x = self.width as f32 - 204.0;
            let outliner_y = 120.0;
            if let Some(action) = self.outliner.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y,
                outliner_x, outliner_y,
            ) {
                match action {
                    OutlinerAction::Select(idx) => {
                        self.selected_node = Some(idx);
                        let pos = self.demo.scene.transform(idx).col(3).truncate();
                        self.gizmo.position = pos;
                        self.gizmo.visible = true;
                    }
                    OutlinerAction::ToggleVisibility(idx) => {
                        let node = self.demo.scene.node_mut(idx);
                        node.visible = !node.visible;
                    }
                }
                self.raw_input.left_just_pressed = false;
            }
        }

        if button == 2 {
            self.raw_input.right_just_pressed = true;
        }
    }

    pub fn mouse_up(&mut self, button: u8) {
        self.raw_input.mouse_down = false;
        self.input.mouse_button_up(MouseButton::from(button));

        if button == 0 {
            // Section view slider release
            if self.section_view.dragging {
                self.section_view.dragging = false;
            }

            // Color picker drag release
            if self.color_picker.visible {
                self.color_picker.handle_mouse(
                    self.raw_input.mouse_x, self.raw_input.mouse_y, false,
                );
            }

            // Record undo for drag transform
            if self.raw_input.left_dragging {
                if let (Some(idx), Some(old_t)) = (self.selected_node, self.raw_input.drag_start_transform.take()) {
                    let new_t = self.demo.scene.transform(idx);
                    if old_t != new_t {
                        self.undo_stack.push(Action::SetTransform {
                            index: idx,
                            old_transform: old_t,
                            new_transform: new_t,
                        });
                    }
                }
            }
            // Box select: resolve selection from marquee
            if let Some((sx, sy)) = self.raw_input.box_select_start.take() {
                let ex = self.raw_input.mouse_x;
                let ey = self.raw_input.mouse_y;
                let min_x = sx.min(ex);
                let max_x = sx.max(ex);
                let min_y = sy.min(ey);
                let max_y = sy.max(ey);

                let aspect = self.width as f32 / self.height.max(1) as f32;
                let vp_mat = self.camera.view_proj(aspect);
                let sw = self.width as f32;
                let sh = self.height as f32;

                self.selection.clear();
                for (i, node) in self.demo.scene.iter() {
                    if !node.visible { continue; }
                    let pos = node.transform.col(3).truncate();
                    let clip = vp_mat * pos.extend(1.0);
                    if clip.w <= 0.0 { continue; }
                    let ndc = clip.truncate() / clip.w;
                    let screen_x = (ndc.x * 0.5 + 0.5) * sw;
                    let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * sh;

                    if screen_x >= min_x && screen_x <= max_x
                        && screen_y >= min_y && screen_y <= max_y
                    {
                        self.selection.push(i);
                    }
                }

                if let Some(&last) = self.selection.last() {
                    self.selected_node = Some(last);
                    let pos = self.demo.scene.transform(last).col(3).truncate();
                    self.gizmo.position = pos;
                    self.gizmo.visible = true;
                    if self.selection.len() > 1 {
                        self.toasts.info(&format!("Selected {} objects", self.selection.len()));
                    }
                }
            }
            self.raw_input.left_dragging = false;
        }

        // Right-click release: if marking menu is open, resolve selection
        if button == 2 {
            if self.marking_menu.visible {
                if let Some(action_id) = self.marking_menu.release() {
                    self.execute_menu_action(action_id);
                }
            } else if self.raw_input.right_just_pressed {
                self.raw_input.right_just_pressed = false;
                self.open_marking_menu();
            }
        }
    }

    pub fn mouse_wheel(&mut self, delta: f32) {
        self.camera.zoom(delta * -0.01);
        self.input.mouse_wheel(delta);
    }

    pub fn touch_start(&mut self, id: i32, x: f32, y: f32) {
        self.touch_state.on_touch_start(id as u32, x, y);
    }

    pub fn touch_move(&mut self, id: i32, x: f32, y: f32) {
        self.touch_state.on_touch_move(id as u32, x, y);
    }

    pub fn touch_end(&mut self, id: i32) {
        self.touch_state.on_touch_end(id as u32);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            self.forward_renderer.resize(&self.device, width, height);
            self.workspace.resolve(width as f32, height as f32);
        }
    }

    // ── Command execution ─────────────────────────────────────────────

    fn execute_command(&mut self, cmd_idx: usize) {
        let cmd = &self.command_palette.commands[cmd_idx];
        match cmd.name.as_str() {
            "Undo" => { self.undo_stack.undo(&mut self.demo.scene); }
            "Redo" => { self.undo_stack.redo(&mut self.demo.scene); }
            "Delete" => self.delete_selected(),
            "Duplicate" => self.duplicate_selected(),
            "Toggle Snap" => { self.snap_to_grid = !self.snap_to_grid; }
            "Deselect" => {
                self.selected_node = None;
                self.selection.clear();
                self.gizmo.visible = false;
            }
            "Front View" => self.apply_view_preset(ViewPreset::Front),
            "Right View" => self.apply_view_preset(ViewPreset::Right),
            "Top View" => self.apply_view_preset(ViewPreset::Top),
            "Isometric View" => self.apply_view_preset(ViewPreset::Iso),
            "Zoom to Fit" => {
                self.camera.target = Vec3::ZERO;
                self.camera.distance = 12.0;
            }
            "Move Tool" => self.set_gizmo_mode(GizmoMode::Translate),
            "Rotate Tool" => self.set_gizmo_mode(GizmoMode::Rotate),
            "Scale Tool" => self.set_gizmo_mode(GizmoMode::Scale),
            "Next Material" => {
                self.demo.selected_material_idx =
                    (self.demo.selected_material_idx + 1) % MATERIAL_IDS.len();
            }
            "Cycle Theme" => {
                let all = ThemeMode::all();
                let idx = all.iter().position(|m| *m == self.theme_mode).unwrap_or(0);
                self.theme_mode = all[(idx + 1) % all.len()];
                let hour = js_sys::Date::new_0().get_hours();
                self.theme = self.theme_mode.resolve(hour);
                self.ui_ctx.apply_theme(&self.theme);
            }
            "Toggle Wireframe" => {
                self.shading_mode = self.shading_mode.cycle();
            }
            "Toggle Ortho" => {
                self.camera.toggle_ortho();
            }
            "Toggle Measurements" => {
                self.measurements.visible = !self.measurements.visible;
            }
            "Toggle Labels" => {
                self.viewport_labels.toggle();
            }
            "Toggle Section Plane" => {
                self.clip_plane.toggle();
                let state = if self.clip_plane.enabled { "ON" } else { "OFF" };
                self.toasts.info(&format!("Section plane: {}", state));
            }
            "Cycle Clip Axis" => {
                if self.clip_plane.enabled {
                    self.clip_plane.axis = self.clip_plane.axis.cycle();
                    self.toasts.info(&format!("Clip axis: {}", self.clip_plane.axis.name()));
                }
            }
            "Toggle Perf HUD" => {
                self.perf_hud.toggle();
            }
            _ => {}
        }
    }

    fn execute_context_menu_item(&mut self, item_idx: usize) {
        let label = self.context_menu.items[item_idx].label.clone();
        match label.as_str() {
            "Delete" => self.delete_selected(),
            "Deselect" => {
                self.selected_node = None;
                self.selection.clear();
                self.gizmo.visible = false;
            }
            "Zoom to Fit" => {
                self.camera.target = Vec3::ZERO;
                self.camera.distance = 12.0;
            }
            "Front View" => self.apply_view_preset(ViewPreset::Front),
            "Top View" => self.apply_view_preset(ViewPreset::Top),
            "Right View" => self.apply_view_preset(ViewPreset::Right),
            _ => {}
        }
    }

    /// Open the radial marking menu (right-click).
    fn open_marking_menu(&mut self) {
        let mut slots: [Option<MarkingEntry>; 8] = Default::default();

        if self.selected_node.is_some() {
            // Object-specific actions
            slots[MarkingSlot::N as usize] = Some(MarkingEntry::new("Move", "modify.move", "G"));
            slots[MarkingSlot::NE as usize] = Some(MarkingEntry::new("Rotate", "modify.rotate", "R"));
            slots[MarkingSlot::E as usize] = Some(MarkingEntry::new("Scale", "modify.scale", "S"));
            slots[MarkingSlot::S as usize] = Some(MarkingEntry::new("Delete", "edit.delete", "X"));
            slots[MarkingSlot::SW as usize] = Some(MarkingEntry::new("Dup", "edit.duplicate", "D"));
            slots[MarkingSlot::W as usize] = Some(MarkingEntry::new("Deselect", "edit.deselect", "-"));
            slots[MarkingSlot::NW as usize] = Some(MarkingEntry::new("Mate", "mate.open", "M"));
            slots[MarkingSlot::SE as usize] = Some(MarkingEntry::new("Props", "view.properties", "P"));
        } else {
            // Scene-level actions
            slots[MarkingSlot::N as usize] = Some(MarkingEntry::new("Undo", "edit.undo", "U"));
            slots[MarkingSlot::S as usize] = Some(MarkingEntry::new("Redo", "edit.redo", "R"));
            slots[MarkingSlot::E as usize] = Some(MarkingEntry::new("Fit", "view.fit", "F"));
            slots[MarkingSlot::W as usize] = Some(MarkingEntry::new("Front", "view.front", "1"));
            slots[MarkingSlot::NE as usize] = Some(MarkingEntry::new("Iso", "view.iso", "0"));
            slots[MarkingSlot::NW as usize] = Some(MarkingEntry::new("Top", "view.top", "7"));
            slots[MarkingSlot::SE as usize] = Some(MarkingEntry::new("Right", "view.right", "3"));
            slots[MarkingSlot::SW as usize] = Some(MarkingEntry::new("Cube", "insert.cube", "+"));
        }

        self.marking_menu.open(self.raw_input.mouse_x, self.raw_input.mouse_y, slots);
    }

    fn open_context_menu(&mut self) {
        let mut items = Vec::new();
        if self.selected_node.is_some() {
            items.push(MenuItem::action("Delete").with_shortcut("Del"));
            items.push(MenuItem::action("Deselect").with_shortcut("Esc"));
            items.push(MenuItem::separator());
        }
        items.push(MenuItem::action("Zoom to Fit").with_shortcut("Home"));
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Front View").with_shortcut("1"));
        items.push(MenuItem::action("Top View").with_shortcut("7"));
        items.push(MenuItem::action("Right View").with_shortcut("3"));
        self.context_menu.open(self.raw_input.mouse_x, self.raw_input.mouse_y, items);
    }

    fn apply_view_preset(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.yaw_pitch();
        self.camera.animate_to(yaw, pitch);
    }

    fn set_gizmo_mode(&mut self, mode: GizmoMode) {
        self.gizmo.mode = mode;
        // Update toolbar active state
        let idx = match mode {
            GizmoMode::Translate => 1,
            GizmoMode::Rotate => 2,
            GizmoMode::Scale => 3,
        };
        self.set_tool_active(idx);
    }

    fn set_tool_active(&mut self, idx: usize) {
        for (i, btn) in self.toolbar.buttons.iter_mut().enumerate() {
            btn.active = i == idx;
        }
        // Update gizmo mode based on tool
        match idx {
            1 => self.gizmo.mode = GizmoMode::Translate,
            2 => self.gizmo.mode = GizmoMode::Rotate,
            3 => self.gizmo.mode = GizmoMode::Scale,
            _ => {} // Select mode — no gizmo mode change
        }
        // Update status bar
        self.status_bar.tool = match idx {
            0 => "Select".into(),
            1 => "Move".into(),
            2 => "Rotate".into(),
            3 => "Scale".into(),
            _ => "Select".into(),
        };
    }

    fn delete_selected(&mut self) {
        if let Some(idx) = self.selected_node.take() {
            let node = self.demo.scene.node(idx).clone();
            let name = node.name.clone();
            self.undo_stack.push(Action::RemoveObject { node, index: idx });
            self.demo.scene.remove(idx);
            self.selection.retain(|&i| i != idx);
            self.gizmo.visible = false;
            self.toasts.push(&format!("Deleted {}", name), ToastLevel::Warning);
        }
    }

    fn duplicate_selected(&mut self) {
        if let Some(idx) = self.selected_node {
            let src = self.demo.scene.node(idx).clone();
            // Offset the duplicate slightly
            let mut dup = src.clone();
            dup.name = format!("{}.001", dup.name);
            let offset = glam::Mat4::from_translation(Vec3::new(1.5, 0.0, 0.0));
            dup.transform = offset * dup.transform;

            let dup_name = dup.name.clone();
            let new_idx = self.demo.scene.add(dup.clone());
            self.undo_stack.push(Action::Duplicate {
                source_index: idx,
                new_node: dup,
                new_index: new_idx,
            });
            self.toasts.success(&format!("Duplicated → {}", dup_name));

            // Select the duplicate
            self.selected_node = Some(new_idx);
            self.selection = vec![new_idx];
            let pos = self.demo.scene.transform(new_idx).col(3).truncate();
            self.gizmo.position = pos;
            self.gizmo.visible = true;
        }
    }

    /// Execute a menu bar action by ID.
    fn execute_menu_action(&mut self, id: &str) {
        match id {
            // File
            "file.new" => {
                self.demo.scene = Scene::new();
                self.selected_node = None;
                self.selection.clear();
                self.gizmo.visible = false;
                self.toasts.info("New scene");
            }
            // Edit
            "edit.undo" => { if let Some(d) = self.undo_stack.undo(&mut self.demo.scene) { self.toasts.info(&format!("Undo: {}", d)); } }
            "edit.redo" => { if let Some(d) = self.undo_stack.redo(&mut self.demo.scene) { self.toasts.info(&format!("Redo: {}", d)); } }
            "edit.select_all" => {
                self.selection.clear();
                for i in 0..self.demo.scene.len() { self.selection.push(i); }
                if let Some(&last) = self.selection.last() {
                    self.selected_node = Some(last);
                    self.gizmo.visible = true;
                }
            }
            "edit.deselect" => {
                self.selected_node = None;
                self.selection.clear();
                self.gizmo.visible = false;
            }
            "edit.duplicate" => self.duplicate_selected(),
            "edit.delete" => self.delete_selected(),
            // View
            "view.front" => self.apply_view_preset(ViewPreset::Front),
            "view.back" => self.apply_view_preset(ViewPreset::Back),
            "view.right" => self.apply_view_preset(ViewPreset::Right),
            "view.left" => self.apply_view_preset(ViewPreset::Left),
            "view.top" => self.apply_view_preset(ViewPreset::Top),
            "view.bottom" => self.apply_view_preset(ViewPreset::Bottom),
            "view.iso" => self.apply_view_preset(ViewPreset::Iso),
            "view.ortho" => self.camera.toggle_ortho(),
            "view.shading" => { self.shading_mode = self.shading_mode.cycle(); }
            "view.measurements" => { self.measurements.visible = !self.measurements.visible; }
            "view.labels" => self.viewport_labels.toggle(),
            "view.section" => {
                self.clip_plane.toggle();
                let s = if self.clip_plane.enabled { "ON" } else { "OFF" };
                self.toasts.info(&format!("Section plane: {}", s));
            }
            "view.perf" => self.perf_hud.toggle(),
            "view.properties" => {
                self.property_panel.toggle();
                if self.property_panel.visible {
                    self.toasts.info("Properties panel");
                }
            }
            "view.fit" => { self.camera.target = Vec3::ZERO; self.camera.distance = 12.0; }
            "view.grid" => {} // TODO: toggle grid
            // Insert
            "insert.cube" => self.insert_primitive("Cube", builtin::CUBE),
            "insert.sphere" => self.insert_primitive("Sphere", builtin::SPHERE),
            "insert.cylinder" => self.insert_primitive("Cylinder", builtin::CYLINDER),
            "insert.torus" => self.insert_primitive("Torus", builtin::TORUS),
            "insert.icosphere" => self.insert_primitive("Icosphere", builtin::ICOSPHERE),
            // Modify
            "modify.move" => {
                self.set_gizmo_mode(GizmoMode::Translate);
                if self.selected_node.is_some() { self.transform_input.begin(TransformMode::Translate); }
            }
            "modify.rotate" => {
                self.set_gizmo_mode(GizmoMode::Rotate);
                if self.selected_node.is_some() { self.transform_input.begin(TransformMode::Rotate); }
            }
            "modify.scale" => {
                self.set_gizmo_mode(GizmoMode::Scale);
                if self.selected_node.is_some() { self.transform_input.begin(TransformMode::Scale); }
            }
            "modify.reset" => {
                if let Some(idx) = self.selected_node {
                    let old_t = self.demo.scene.transform(idx);
                    let new_t = Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0));
                    self.undo_stack.push(Action::SetTransform { index: idx, old_transform: old_t, new_transform: new_t });
                    self.demo.scene.set_transform(idx, new_t);
                    self.toasts.info("Transform reset");
                }
            }
            "modify.snap" => { self.snap_to_grid = !self.snap_to_grid; }
            // Mate
            "mate.open" => {
                self.mate_dialog.open();
                if let Some(idx) = self.selected_node {
                    let name = self.demo.scene.node(idx).name.clone();
                    self.mate_dialog.set_object_a(idx, &name);
                }
            }
            "mate.stack_top" => self.quick_mate(MateOp::StackOnTop),
            "mate.concentric" => self.quick_mate(MateOp::Concentric),
            "mate.align_x" => self.quick_mate(MateOp::AlignX),
            "mate.align_z" => self.quick_mate(MateOp::AlignZ),
            "mate.flush_px" => self.quick_mate(MateOp::FlushPosX),
            "mate.solve" => self.solve_constraints(),
            // Help
            "help.shortcuts" => self.shortcut_help.toggle(),
            "help.palette" => self.command_palette.open(),
            "help.about" => self.toasts.info("OpenIE — Physical AI Platform v0.1"),
            _ => {}
        }
    }

    /// Insert a new primitive at the origin.
    fn insert_primitive(&mut self, name: &str, mesh_id: u32) {
        let count = self.demo.scene.len();
        let full_name = if count > 0 {
            format!("{}.{:03}", name, count)
        } else {
            name.to_string()
        };
        let node = SceneNode::new(
            &full_name, mesh_id, 1,
            Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0)),
        );
        let idx = self.demo.scene.add(node.clone());
        self.undo_stack.push(Action::Duplicate {
            source_index: idx,
            new_node: node,
            new_index: idx,
        });
        self.selected_node = Some(idx);
        self.selection = vec![idx];
        let pos = self.demo.scene.transform(idx).col(3).truncate();
        self.gizmo.position = pos;
        self.gizmo.visible = true;
        self.toasts.success(&format!("Inserted {}", full_name));
    }

    /// Quick mate between the first two selected objects (or selected + last clicked).
    fn quick_mate(&mut self, op: MateOp) {
        if self.selection.len() >= 2 {
            let a_idx = self.selection[0];
            let b_idx = self.selection[1];
            let a_t = self.demo.scene.transform(a_idx);
            let b_t = self.demo.scene.transform(b_idx);
            let new_t = compute_mate(op, a_t, b_t);
            self.undo_stack.push(Action::SetTransform {
                index: a_idx,
                old_transform: a_t,
                new_transform: new_t,
            });
            self.demo.scene.set_transform(a_idx, new_t);
            self.toasts.success(&format!("Mated: {}", op.label()));
        } else if let Some(idx) = self.selected_node {
            // Need a second object — use the next one in scene
            let other = if idx + 1 < self.demo.scene.len() { idx + 1 } else if idx > 0 { idx - 1 } else { return; };
            let a_t = self.demo.scene.transform(idx);
            let b_t = self.demo.scene.transform(other);
            let new_t = compute_mate(op, a_t, b_t);
            self.undo_stack.push(Action::SetTransform {
                index: idx,
                old_transform: a_t,
                new_transform: new_t,
            });
            self.demo.scene.set_transform(idx, new_t);
            self.toasts.success(&format!("Mated: {}", op.label()));
        } else {
            self.toasts.push("Select 2 objects to mate", ToastLevel::Warning);
        }
    }

    /// Apply the mate from the dialog.
    fn apply_mate_from_dialog(&mut self) {
        let Some(a_idx) = self.mate_dialog.object_a else { return };
        let Some(b_idx) = self.mate_dialog.object_b else { return };

        let op_id = self.mate_dialog.selected_op_id();
        let op = match op_id {
            "stack_top" => MateOp::StackOnTop,
            "stack_below" => MateOp::StackBelow,
            "align_x" => MateOp::AlignX,
            "align_y" => MateOp::AlignY,
            "align_z" => MateOp::AlignZ,
            "flush_px" => MateOp::FlushPosX,
            "flush_nx" => MateOp::FlushNegX,
            "flush_pz" => MateOp::FlushPosZ,
            "flush_nz" => MateOp::FlushNegZ,
            "concentric" => MateOp::Concentric,
            "offset" => {
                let dist = self.mate_dialog.param_value().unwrap_or(1.0);
                MateOp::Offset(dist)
            }
            _ => return,
        };

        let a_t = self.demo.scene.transform(a_idx);
        let b_t = self.demo.scene.transform(b_idx);
        let new_t = compute_mate(op, a_t, b_t);
        self.undo_stack.push(Action::SetTransform {
            index: a_idx,
            old_transform: a_t,
            new_transform: new_t,
        });
        self.demo.scene.set_transform(a_idx, new_t);

        // Add persistent constraint
        let name = self.mate_dialog.constraint_name.text.clone();
        self.mate_system.add(MateConstraint::new(&name, a_idx, b_idx, op));

        self.toasts.success(&format!("Applied: {}", op.label()));
        self.mate_dialog.close();
    }

    /// Solve all active mate constraints.
    fn solve_constraints(&mut self) {
        let updates = self.mate_system.solve(|i| self.demo.scene.transform(i));
        let count = updates.len();
        for (idx, new_t) in updates {
            self.demo.scene.set_transform(idx, new_t);
        }
        if count > 0 {
            self.toasts.info(&format!("Solved {} constraints", count));
        }
    }

    /// Apply a confirmed numeric transform value to the selected object.
    fn apply_transform_value(&mut self, val: f32) {
        let Some(idx) = self.selected_node else { return };
        let old_t = self.demo.scene.transform(idx);
        let axis = self.transform_input.axis;

        let new_t = match self.transform_input.mode {
            TransformMode::Translate => {
                let offset = match axis {
                    AxisConstraint::X => Vec3::new(val, 0.0, 0.0),
                    AxisConstraint::Y => Vec3::new(0.0, val, 0.0),
                    AxisConstraint::Z => Vec3::new(0.0, 0.0, val),
                    AxisConstraint::None => Vec3::new(val, 0.0, 0.0), // default X
                };
                Mat4::from_translation(offset) * old_t
            }
            TransformMode::Rotate => {
                let radians = val.to_radians();
                let rot = match axis {
                    AxisConstraint::X => Mat4::from_rotation_x(radians),
                    AxisConstraint::Y | AxisConstraint::None => Mat4::from_rotation_y(radians),
                    AxisConstraint::Z => Mat4::from_rotation_z(radians),
                };
                let pos = old_t.col(3);
                Mat4::from_translation(pos.truncate()) * rot
                    * Mat4::from_translation(-pos.truncate()) * old_t
            }
            TransformMode::Scale => {
                let s = match axis {
                    AxisConstraint::X => Vec3::new(val, 1.0, 1.0),
                    AxisConstraint::Y => Vec3::new(1.0, val, 1.0),
                    AxisConstraint::Z => Vec3::new(1.0, 1.0, val),
                    AxisConstraint::None => Vec3::splat(val),
                };
                old_t * Mat4::from_scale(s)
            }
        };

        self.undo_stack.push(Action::SetTransform {
            index: idx,
            old_transform: old_t,
            new_transform: new_t,
        });
        self.demo.scene.set_transform(idx, new_t);

        let desc = self.transform_input.display();
        self.toasts.success(&desc);
    }

    /// Snap a position to the grid if snapping is enabled.
    fn snap_position(&self, pos: Vec3) -> Vec3 {
        if self.snap_to_grid {
            let s = self.grid_snap_size;
            Vec3::new(
                (pos.x / s).round() * s,
                pos.y, // Don't snap Y (vertical)
                (pos.z / s).round() * s,
            )
        } else {
            pos
        }
    }

    // ── Main frame ────────────────────────────────────────────────────

    pub fn frame(&mut self, _seconds: f32) {
        let dt = 1.0 / 60.0;
        self.input.update(dt);

        // Workspace panel resize dragging
        let screen_w = self.width as f32;
        let screen_h = self.height as f32;
        self.workspace.handle_input(
            self.raw_input.mouse_x, self.raw_input.mouse_y,
            self.raw_input.mouse_down,
            screen_w, screen_h,
        );

        // Smooth camera animation
        self.camera.update(dt);

        // Performance recording
        self.perf_hud.record_frame(dt);

        // Toast timers
        self.toasts.update(dt);

        // Tooltip timer
        self.tooltip.hover_time += dt;

        // Property panel slide animation
        self.property_panel.update(dt);

        // Context toolbar fade animation
        self.context_toolbar.update(dt);

        // Marking menu timer
        self.marking_menu.update(dt);

        // Confirmation corner fade
        self.confirmation_corner.update(dt);

        // Workspace switcher transition
        self.workspace_switcher.update(dt);

        // Progress overlay animation
        self.progress_overlay.update(dt);

        // Dimension overlay — clear transient each frame
        self.dimension_overlay.clear_transient();

        // Left-click pick: select object (only on click, not drag)
        if self.raw_input.left_just_pressed {
            self.raw_input.left_just_pressed = false;
            let (origin, dir) = self.camera.screen_to_ray(
                self.raw_input.mouse_x,
                self.raw_input.mouse_y,
                self.width as f32,
                self.height as f32,
            );
            self.selected_node = self.demo.scene.pick(origin, dir);

            // Update gizmo position + show context toolbar
            if let Some(idx) = self.selected_node {
                let pos = self.demo.scene.transform(idx).col(3).truncate();
                self.gizmo.position = pos;
                self.gizmo.visible = true;

                // Show context toolbar near the click
                let tb_y = (self.raw_input.mouse_y - 44.0).max(self.menu_bar.height + 30.0);
                self.context_toolbar.show(
                    self.raw_input.mouse_x + 12.0,
                    tb_y,
                    vec![
                        ContextButton::new("Move", "modify.move", "G"),
                        ContextButton::new("Rot", "modify.rotate", "R"),
                        ContextButton::new("Scl", "modify.scale", "S"),
                        ContextButton::separator(),
                        ContextButton::new("Dup", "edit.duplicate", "D"),
                        ContextButton::new("Del", "edit.delete", "X"),
                    ],
                );

                // Auto-open property panel on selection
                if !self.property_panel.visible {
                    self.property_panel.open();
                }
            } else {
                self.gizmo.visible = false;
                self.context_toolbar.hide();
            }
        }

        // Keep gizmo position synced with selected object
        if let Some(idx) = self.selected_node {
            let pos = self.demo.scene.transform(idx).col(3).truncate();
            self.gizmo.position = pos;
        }

        // Show confirmation corner during active transforms
        if self.transform_input.active && !self.confirmation_corner.active {
            let desc = match self.transform_input.mode {
                TransformMode::Translate => "Move",
                TransformMode::Rotate => "Rotate",
                TransformMode::Scale => "Scale",
            };
            self.confirmation_corner.begin(OperationType::Transform, desc);
        } else if !self.transform_input.active && self.confirmation_corner.active {
            if self.confirmation_corner.operation == OperationType::Transform {
                self.confirmation_corner.end();
            }
        }

        // Update snap indicator state
        self.snap_indicator.grid_snap = self.snap_to_grid;
        self.snap_indicator.grid_size = self.grid_snap_size;
        if self.raw_input.left_dragging {
            if !self.snap_indicator.dragging {
                self.snap_indicator.begin_drag(self.raw_input.drag_start_x, self.raw_input.drag_start_y);
            }
            self.snap_indicator.update_drag(self.raw_input.mouse_x, self.raw_input.mouse_y);
            // Determine axis from drag direction
            let dx = (self.raw_input.mouse_x - self.raw_input.drag_start_x).abs();
            let dy = (self.raw_input.mouse_y - self.raw_input.drag_start_y).abs();
            self.snap_indicator.axis = if dx > dy * 2.0 {
                SnapAxis::X
            } else if dy > dx * 2.0 {
                SnapAxis::Y
            } else {
                SnapAxis::XZ
            };
            // Distance readout
            let dist = (dx * dx + dy * dy).sqrt();
            self.snap_indicator.distance = Some(dist / 100.0); // approx world units
        } else if self.snap_indicator.dragging {
            self.snap_indicator.end_drag();
        }

        // Auto-populate selection info panel
        if self.selection_info.visible {
            if let Some(idx) = self.selected_node {
                let node = self.demo.scene.node(idx);
                let t = node.transform;
                let scale = glam::Vec3::new(
                    t.col(0).truncate().length(),
                    t.col(1).truncate().length(),
                    t.col(2).truncate().length(),
                );
                let pos = t.col(3).truncate();
                let mut props = SelectionProperties::new(&node.name);
                props.bbox_min = [pos.x - scale.x * 0.5, pos.y - scale.y * 0.5, pos.z - scale.z * 0.5];
                props.bbox_max = [pos.x + scale.x * 0.5, pos.y + scale.y * 0.5, pos.z + scale.z * 0.5];
                props.volume = scale.x * scale.y * scale.z;
                props.surface_area = 2.0 * (scale.x * scale.y + scale.y * scale.z + scale.x * scale.z);
                props.center_of_mass = [pos.x, pos.y, pos.z];
                let mat_id = MATERIAL_IDS[self.demo.selected_material_idx % MATERIAL_IDS.len()];
                props.material = mat_id.to_string();
                if let Some(r) = cascade::density(mat_id) {
                    if let Value::Density(d) = r.value {
                        let density_val = d.value();
                        props.density = Some(density_val as f32);
                        props.mass = Some(props.volume * density_val as f32);
                    }
                }
                props.faces = 6; // cube faces as demo
                props.edges = 12;
                props.vertices = 8;
                props.triangles = 12;
                self.selection_info.properties = Some(props);
            } else {
                self.selection_info.hide();
            }
        }

        // Viewport splitter resolve
        {
            let vp_x = 0.0;
            let vp_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            let vp_h = screen_h - vp_y - self.enhanced_status.height - self.timeline.height;
            self.viewport_splitter.resolve(vp_x, vp_y, screen_w, vp_h);
        }

        // Sync breadcrumb with workspace + selection
        {
            let ws_label = self.workspace_switcher.active.label();
            if self.breadcrumb_bar.segments.is_empty()
                || self.breadcrumb_bar.segments[0].label != ws_label
            {
                self.breadcrumb_bar.clear();
                self.breadcrumb_bar.push(BreadcrumbSegment::new(ws_label, "#", 0));
            }
            // If object selected, add it as a segment
            if let Some(idx) = self.selected_node {
                let name = self.demo.scene.node(idx).name.clone();
                let seg_count = self.breadcrumb_bar.segments.len();
                if seg_count == 1 {
                    self.breadcrumb_bar.push(
                        BreadcrumbSegment::new(&name, "@", idx as u32 + 1).active()
                    );
                } else if seg_count >= 2 && self.breadcrumb_bar.segments[1].label != name {
                    // Replace the object segment
                    self.breadcrumb_bar.segments.truncate(1);
                    self.breadcrumb_bar.push(
                        BreadcrumbSegment::new(&name, "@", idx as u32 + 1).active()
                    );
                }
            } else if self.breadcrumb_bar.segments.len() > 1 {
                self.breadcrumb_bar.segments.truncate(1);
                if let Some(last) = self.breadcrumb_bar.segments.last_mut() {
                    last.active = true;
                }
            }
        }

        // Update enhanced status bar info
        self.enhanced_status.object_count = self.demo.scene.len();
        self.enhanced_status.tool = self.status_bar.tool.clone();

        let ortho_label = if self.camera.orthographic { "Ortho" } else { "Persp" };
        let snap_label = if self.snap_to_grid { format!("Snap:{}", self.grid_snap_size) } else { "Snap:Off".into() };
        let clip_label = if self.clip_plane.enabled {
            format!("Clip:{}", self.clip_plane.axis.name())
        } else {
            String::new()
        };
        self.enhanced_status.mode = if clip_label.is_empty() {
            format!(
                "{} | {} | {} | {}",
                if self.selected_node.is_some() { "Object" } else { "Scene" },
                self.shading_mode.name(),
                ortho_label,
                snap_label,
            )
        } else {
            format!(
                "{} | {} | {} | {} | {}",
                if self.selected_node.is_some() { "Object" } else { "Scene" },
                self.shading_mode.name(),
                ortho_label,
                snap_label,
                clip_label,
            )
        };
        self.enhanced_status.hints = if self.command_palette.visible {
            "Esc: close palette".into()
        } else if self.selected_node.is_some() {
            "G:Move R:Rotate S:Scale Del:Delete P:Props RMB:Radial".into()
        } else {
            "Ctrl+K:Cmds Ctrl+Z:Undo RMB:Radial Menu Scroll:Zoom".into()
        };

        // Update cursor world coords for status bar
        let (origin, dir) = self.camera.screen_to_ray(
            self.raw_input.mouse_x, self.raw_input.mouse_y,
            self.width as f32, self.height as f32,
        );
        if let Some(hit) = OrbitCamera::ray_ground_intersect(origin, dir, 0.0) {
            self.enhanced_status.cursor_coords = Some([hit.x, 0.0, hit.z]);
        } else {
            self.enhanced_status.cursor_coords = None;
        }

        // Build property panel sections
        self.property_panel.clear();
        if let Some(idx) = self.selected_node {
            let node = self.demo.scene.node(idx);
            let t = node.transform;
            let pos = t.col(3).truncate();
            let scale = Vec3::new(
                t.col(0).truncate().length(),
                t.col(1).truncate().length(),
                t.col(2).truncate().length(),
            );
            self.property_panel.title = format!("{} (#{idx})", node.name);

            let mut transform_sec = PropertySection::new("Transform");
            transform_sec.add(PropertyEntry::new("Position X", &format!("{:.3}", pos.x)).editable());
            transform_sec.add(PropertyEntry::new("Position Y", &format!("{:.3}", pos.y)).editable());
            transform_sec.add(PropertyEntry::new("Position Z", &format!("{:.3}", pos.z)).editable());
            transform_sec.add(PropertyEntry::new("Scale X", &format!("{:.2}", scale.x)).editable());
            transform_sec.add(PropertyEntry::new("Scale Y", &format!("{:.2}", scale.y)).editable());
            transform_sec.add(PropertyEntry::new("Scale Z", &format!("{:.2}", scale.z)).editable());
            self.property_panel.add_section(transform_sec);

            let mut info_sec = PropertySection::new("Object Info");
            info_sec.add(PropertyEntry::new("Mesh ID", &format!("{}", node.mesh_id)));
            info_sec.add(PropertyEntry::new("Material", &format!("{}", node.material_id)));
            let vol = scale.x * scale.y * scale.z;
            info_sec.add(PropertyEntry::new("Volume (est)", &format!("{:.3} m\u{00B3}", vol)));
            let sa = 2.0 * (scale.x * scale.y + scale.y * scale.z + scale.x * scale.z);
            info_sec.add(PropertyEntry::new("Surface (est)", &format!("{:.3} m\u{00B2}", sa)));
            info_sec.add(PropertyEntry::new("Visible", if node.visible { "Yes" } else { "No" }));
            self.property_panel.add_section(info_sec);

            // Material section from cascade
            let mat_id = MATERIAL_IDS[self.demo.selected_material_idx % MATERIAL_IDS.len()];
            let mut mat_sec = PropertySection::new(&format!("Material: {}", mat_id));
            if let Some(r) = cascade::yield_strength(mat_id) {
                if let Value::Pressure(p) = r.value {
                    mat_sec.add(PropertyEntry::new("Yield", &format!("{:.0} MPa", p.to_mpa())));
                }
            }
            if let Some(r) = cascade::ultimate_tensile(mat_id) {
                if let Value::Pressure(p) = r.value {
                    mat_sec.add(PropertyEntry::new("UTS", &format!("{:.0} MPa", p.to_mpa())));
                }
            }
            if let Some(r) = cascade::elastic_modulus(mat_id) {
                if let Value::Pressure(p) = r.value {
                    mat_sec.add(PropertyEntry::new("Elastic Mod", &format!("{:.1} GPa", p.to_mpa() / 1000.0)));
                }
            }
            if let Some(r) = cascade::density(mat_id) {
                if let Value::Density(d) = r.value {
                    mat_sec.add(PropertyEntry::new("Density", &format!("{:.0} kg/m\u{00B3}", d.value())));
                }
            }
            self.property_panel.add_section(mat_sec);
        } else {
            self.property_panel.title = "Scene".to_string();
            let mut scene_sec = PropertySection::new("Scene Info");
            scene_sec.add(PropertyEntry::new("Objects", &format!("{}", self.demo.scene.len())));
            scene_sec.add(PropertyEntry::new("Undo depth", &format!("{}", self.undo_stack.depth())));
            scene_sec.add(PropertyEntry::new("Theme", self.theme_mode.name()));
            scene_sec.add(PropertyEntry::new("Shading", self.shading_mode.name()));
            self.property_panel.add_section(scene_sec);
        }

        // Update timeline hover
        {
            let tl_y = screen_h - self.enhanced_status.height - self.timeline.height;
            self.timeline.hovered = self.timeline.hit_test(
                self.raw_input.mouse_x, self.raw_input.mouse_y, 0.0, tl_y,
            );
        }

        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(tex)
            | wgpu::CurrentSurfaceTexture::Suboptimal(tex) => tex,
            other => {
                log::warn!("Failed to acquire surface texture: {:?}", other);
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame-encoder"),
        });

        // 1. Clear to themed background
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.theme.clear_r,
                            g: self.theme.clear_g,
                            b: self.theme.clear_b,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        // 2. Render grid
        let aspect = self.width as f32 / self.height.max(1) as f32;
        let vp = self.camera.view_proj(aspect);
        let inv_vp = vp.inverse();
        let eye = self.camera.eye();
        let grid_uniforms = GridUniforms {
            view_proj: vp.to_cols_array(),
            inv_view_proj: inv_vp.to_cols_array(),
            camera_pos: [eye.x, eye.y, eye.z],
            _pad: 0.0,
        };
        self.grid_renderer.render(&self.device, &self.queue, &mut encoder, &view, &grid_uniforms);

        // 3. Render 3D scene objects (solid / wireframe / both)
        let render_objects = self.demo.scene.to_render_objects();
        if !render_objects.is_empty() {
            let view_mat = self.camera.view_matrix();
            let proj_mat = self.camera.proj_matrix(aspect);

            let clip_eq = if self.clip_plane.enabled {
                self.clip_plane.equation()
            } else {
                [0.0; 4]
            };

            let forward_input = ForwardFrameInput {
                view: view_mat,
                proj: proj_mat,
                camera_pos: eye,
                sun_dir: [-0.3, -1.0, -0.5],
                sun_intensity: 1.2,
                sun_color: [1.0, 0.98, 0.95],
                ambient: 0.15,
                clip_plane: clip_eq,
            };

            let sel_id = self.selected_node.map(|i| i as u32);

            // Solid pass (Solid or SolidWireframe modes)
            if self.shading_mode != ShadingMode::Wireframe {
                self.forward_renderer.render_with_selection(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    &view,
                    &forward_input,
                    &render_objects,
                    &self.mesh_registry,
                    &self.demo.materials,
                    sel_id,
                );
            }

            // Wireframe pass (Wireframe or SolidWireframe modes)
            if self.shading_mode != ShadingMode::Solid {
                self.wireframe_renderer.render(
                    &self.queue,
                    &mut encoder,
                    &view,
                    self.forward_renderer.depth_view(),
                    &forward_input,
                    &render_objects,
                    &self.mesh_registry,
                    &self.demo.materials,
                    sel_id,
                );
            }

            // Selection outline pass
            if let Some(sel) = sel_id {
                self.outline_renderer.render(
                    &self.queue,
                    &mut encoder,
                    &view,
                    self.forward_renderer.depth_view(),
                    vp,
                    &render_objects,
                    &self.mesh_registry,
                    &[sel],
                );
            }

            // Track stats for perf HUD
            let mut total_verts = 0u32;
            let mut total_tris = 0u32;
            for obj in &render_objects {
                if let Some((_, _, ic)) = self.mesh_registry.get(obj.mesh_id) {
                    total_verts += ic; // approximate
                    total_tris += ic / 3;
                }
            }
            self.perf_hud.objects = render_objects.len() as u32;
            self.perf_hud.vertices = total_verts;
            self.perf_hud.triangles = total_tris;
            self.perf_hud.draw_calls = render_objects.len() as u32 + 2; // grid + gizmo
        }

        // 3b. Render gizmo on top of scene
        self.gizmo_renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            vp,
            &self.gizmo,
        );

        // 3c. Background gradient overlay (subtle top-to-bottom gradient)
        {
            let mut grad_draw = DrawList::new();
            let half = screen_h * 0.5;
            // Top half — slightly lighter
            let top_col = [
                (self.theme.clear_r as f32 + 0.04).min(1.0),
                (self.theme.clear_g as f32 + 0.04).min(1.0),
                (self.theme.clear_b as f32 + 0.06).min(1.0),
                0.4,
            ];
            grad_draw.push_quad(0.0, 0.0, screen_w, half, top_col);
            // Bottom half — slightly darker
            let bot_col = [
                (self.theme.clear_r as f32 - 0.02).max(0.0),
                (self.theme.clear_g as f32 - 0.02).max(0.0),
                (self.theme.clear_b as f32 - 0.01).max(0.0),
                0.3,
            ];
            grad_draw.push_quad(0.0, half, screen_w, half, bot_col);
            self.ui_renderer.render(
                &self.device, &self.queue, &mut encoder, &view, [screen_w, screen_h],
                &[grad_draw],
            );
        }

        // 4. Render UI overlays
        let screen_size = [screen_w, screen_h];

        // Begin UI frame
        self.ui_ctx.begin_frame(
            screen_w, screen_h,
            self.raw_input.mouse_x, self.raw_input.mouse_y,
            self.raw_input.mouse_down,
        );

        // Workspace panel borders
        let mut workspace_draw = DrawList::new();
        self.workspace.draw(&mut workspace_draw, &self.theme);

        // Viewport overlays draw list
        let mut overlay_draw = DrawList::new();

        // Axes indicator (bottom-left)
        self.axes_indicator.draw(
            &mut overlay_draw,
            self.camera.yaw, self.camera.pitch,
            screen_w, screen_h,
        );

        // Navigation cube (upper-right)
        self.nav_cube.draw(
            &mut overlay_draw,
            self.camera.yaw, self.camera.pitch,
            screen_w, screen_h,
        );

        // Measurement overlay
        self.measurements.draw(&mut overlay_draw, vp, screen_w, screen_h);

        // Dimension overlay (on-canvas dimension annotations)
        self.dimension_overlay.draw(&mut overlay_draw, vp, screen_w, screen_h);

        // Constraint icons (on-canvas sketch constraints)
        self.constraint_icons.draw(&mut overlay_draw, vp, screen_w, screen_h);

        // Annotation tools (callouts, arrows, clouds)
        self.annotation_tools.draw(&mut overlay_draw, vp, screen_w, screen_h);

        // Viewport header bar (top of viewport)
        {
            let ortho_label = if self.camera.orthographic { "Ortho" } else { "Persp" };
            let snap_label = if self.snap_to_grid { "Snap" } else { "No Snap" };
            self.viewport_header.set_buttons(vec![
                if self.shading_mode == ShadingMode::Solid { HeaderButton::new("Solid").active() }
                else { HeaderButton::new("Solid") },
                if self.shading_mode == ShadingMode::Wireframe { HeaderButton::new("Wire").active() }
                else { HeaderButton::new("Wire") },
                if self.shading_mode == ShadingMode::SolidWireframe { HeaderButton::new("S+W").active() }
                else { HeaderButton::new("S+W") },
                HeaderButton::new("|"),
                if self.camera.orthographic { HeaderButton::new(ortho_label).active() }
                else { HeaderButton::new(ortho_label) },
                HeaderButton::new("|"),
                if self.snap_to_grid { HeaderButton::new(snap_label).active() }
                else { HeaderButton::new(snap_label) },
            ]);
            let header_y = self.menu_bar.height;
            self.viewport_header.draw(
                &mut overlay_draw, 0.0, header_y, screen_w,
                self.theme.header_bg, self.theme.text,
            );
        }

        // Quick access bar (very top — above menu bar)
        self.quick_access_bar.draw(
            &mut overlay_draw,
            0.0, 0.0, screen_w,
            self.theme.header_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Workspace switcher (below viewport header)
        {
            let ws_y = self.menu_bar.height + 24.0; // viewport header height
            self.workspace_switcher.draw(
                &mut overlay_draw,
                0.0, ws_y, screen_w,
                self.theme.header_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Breadcrumb bar (below workspace switcher)
        {
            let bb_y = self.quick_access_bar.height + self.menu_bar.height + 24.0
                + self.workspace_switcher.height;
            self.breadcrumb_bar.draw(
                &mut overlay_draw,
                0.0, bb_y, screen_w,
                self.theme.header_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Menu bar (top — above viewport header)
        self.menu_bar.draw(&mut overlay_draw, screen_w, self.theme.header_bg, self.theme.text);

        // Toolbar (left side, below menu + header)
        let tb_x = 4.0;
        let tb_y = self.menu_bar.height + 30.0;
        self.toolbar.draw(&mut overlay_draw, tb_x, tb_y, self.theme.panel_bg, self.theme.text);

        // Flyout toolbar (below regular toolbar)
        {
            let ft_x = 4.0;
            let ft_y = self.menu_bar.height + 30.0 + self.workspace_switcher.height;
            self.flyout_toolbar.draw(
                &mut overlay_draw,
                ft_x, ft_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Enhanced status bar (bottom)
        self.enhanced_status.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.header_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Timeline bar (above status bar)
        {
            let tl_y = screen_h - self.enhanced_status.height - self.timeline.height;
            self.timeline.draw(
                &mut overlay_draw,
                tl_y, screen_w,
                self.theme.header_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Outliner panel (right side, below nav cube)
        self.outliner.clear();
        for (i, node) in self.demo.scene.iter() {
            self.outliner.push(&node.name, i, node.visible, self.selected_node == Some(i));
        }
        let outliner_x = screen_w - 204.0;
        let outliner_y = 120.0; // below nav cube
        let outliner_w = 200.0;
        let outliner_h = (self.outliner.entries.len() as f32 * 22.0 + 21.0).min(screen_h - 200.0);
        self.outliner.draw(
            &mut overlay_draw,
            outliner_x, outliner_y,
            outliner_w, outliner_h,
            self.theme.panel_bg,
            self.theme.text,
        );

        // Properties panel — material data from cascade
        self.ui_ctx.panel_begin("OpenIE — Physical AI Platform");

        if let Some(idx) = self.selected_node {
            let node = self.demo.scene.node(idx);
            let t = node.transform;
            let pos = t.col(3).truncate();
            let scale = Vec3::new(
                t.col(0).truncate().length(),
                t.col(1).truncate().length(),
                t.col(2).truncate().length(),
            );

            self.ui_ctx.label(&format!("Selected: {} (#{idx})", node.name));
            self.ui_ctx.separator();

            // Transform section
            self.ui_ctx.panel_begin("Transform");
            self.ui_ctx.label(&format!("X: {:.3}  Y: {:.3}  Z: {:.3}", pos.x, pos.y, pos.z));
            self.ui_ctx.label(&format!("Scale: {:.2} x {:.2} x {:.2}", scale.x, scale.y, scale.z));
            self.ui_ctx.label(&format!("Mesh: {}  Mat: {}", node.mesh_id, node.material_id));

            if self.ui_ctx.button("Reset Position") {
                let old_t = self.demo.scene.transform(idx);
                let new_t = Mat4::from_scale(scale);
                self.undo_stack.push(Action::SetTransform {
                    index: idx,
                    old_transform: old_t,
                    new_transform: new_t,
                });
                self.demo.scene.set_transform(idx, new_t);
            }
            self.ui_ctx.panel_end();

            // Object info section — bounding box dimensions
            self.ui_ctx.panel_begin("Object Info");
            let dim = scale; // for unit primitives, scale ≈ bounding box extents
            self.ui_ctx.label(&format!("Bounds: {:.2} × {:.2} × {:.2}", dim.x, dim.y, dim.z));
            let volume_approx = dim.x * dim.y * dim.z;
            self.ui_ctx.label(&format!("Vol (est): {:.3} m³", volume_approx));
            let surface_approx = 2.0 * (dim.x * dim.y + dim.y * dim.z + dim.x * dim.z);
            self.ui_ctx.label(&format!("SA (est): {:.3} m²", surface_approx));
            self.ui_ctx.panel_end();
        } else {
            self.ui_ctx.label("No selection");
            self.ui_ctx.label(&format!("Objects: {}", self.demo.scene.len()));
            if self.undo_stack.can_undo() {
                self.ui_ctx.label(&format!("Undo depth: {}", self.undo_stack.depth()));
            }
        }
        self.ui_ctx.separator();

        // Material properties — cascade queries (Tier 1: LUT)
        let mat_id = MATERIAL_IDS[self.demo.selected_material_idx % MATERIAL_IDS.len()];
        self.ui_ctx.panel_begin(&format!("Material: {mat_id}"));

        if let Some(r) = cascade::yield_strength(mat_id) {
            if let Value::Pressure(p) = r.value {
                self.ui_ctx.label(&format!("Yield: {:.0} MPa [{}]", p.to_mpa(), r.source));
            }
        }
        if let Some(r) = cascade::ultimate_tensile(mat_id) {
            if let Value::Pressure(p) = r.value {
                self.ui_ctx.label(&format!("UTS: {:.0} MPa", p.to_mpa()));
            }
        }
        if let Some(r) = cascade::elastic_modulus(mat_id) {
            if let Value::Pressure(p) = r.value {
                self.ui_ctx.label(&format!("E: {:.1} GPa", p.to_mpa() / 1000.0));
            }
        }
        if let Some(r) = cascade::density(mat_id) {
            if let Value::Density(d) = r.value {
                self.ui_ctx.label(&format!("Density: {:.0} kg/m\u{00B3}", d.value()));
            }
        }
        if let Some(r) = cascade::thermal_conductivity(mat_id) {
            if let Value::Scalar(k) = r.value {
                self.ui_ctx.label(&format!("k: {:.1} W/mK", k));
            }
        }
        self.ui_ctx.panel_end();

        // DFM check panel (Tier 1: LUT)
        self.ui_ctx.panel_begin("DFM Check");
        let wall_check = cascade::check_wall(
            Length::mm(1.5),
            physical_lut::manufacturing::Process::CncMill3Ax,
            physical_lut::manufacturing::MaterialClass::Aluminum,
        );
        if let Some(r) = wall_check {
            let status = if matches!(r.value, Value::Bool(true)) { "PASS" } else { "FAIL" };
            self.ui_ctx.label(&format!("Wall 1.5mm: {status}"));
        }
        let corner_check = cascade::check_corner(
            Length::mm(0.5),
            physical_lut::manufacturing::Process::CncMill3Ax,
            physical_lut::manufacturing::MaterialClass::Aluminum,
        );
        if let Some(r) = corner_check {
            let status = if matches!(r.value, Value::Bool(true)) { "PASS" } else { "FAIL" };
            self.ui_ctx.label(&format!("Corner R0.5: {status}"));
        }
        self.ui_ctx.panel_end();

        // Beam analysis (Tier 2: Formula)
        self.ui_ctx.panel_begin("Beam Analysis");
        if let Some(r) = cascade::beam_deflection_simply_supported_center(
            Force::kn(10.0), Length::m(1.0), mat_id, Length::mm(50.0), Length::mm(50.0),
        ) {
            if let Value::Length(d) = r.value {
                self.ui_ctx.label(&format!("SS 10kN/1m: {:.2}mm [{:?}]", d.to_mm(), r.tier));
            }
        }
        if let Some(r) = cascade::stress_concentration_hole(Length::mm(5.0), Length::mm(100.0)) {
            if let Value::Dimensionless(kt) = r.value {
                self.ui_ctx.label(&format!("Kt 5mm/100mm: {:.3}", kt.value()));
            }
        }
        {
            let r = cascade::hoop_stress(Pressure::mpa(10.0), Length::mm(50.0), Length::mm(2.0));
            if let Value::Pressure(s) = r.value {
                self.ui_ctx.label(&format!("Hoop: {:.0} MPa", s.to_mpa()));
            }
        }
        self.ui_ctx.panel_end();

        if self.ui_ctx.button("Next Material") {
            self.demo.selected_material_idx =
                (self.demo.selected_material_idx + 1) % MATERIAL_IDS.len();
        }

        self.ui_ctx.separator();
        self.ui_ctx.label(&format!("Theme: {}", self.theme_mode.name()));
        if self.ui_ctx.button("Cycle Theme") {
            let all = ThemeMode::all();
            let idx = all.iter().position(|m| *m == self.theme_mode).unwrap_or(0);
            self.theme_mode = all[(idx + 1) % all.len()];
            let hour = js_sys::Date::new_0().get_hours();
            self.theme = self.theme_mode.resolve(hour);
            self.ui_ctx.apply_theme(&self.theme);
        }

        self.ui_ctx.panel_end();

        let draw_lists = self.ui_ctx.end_frame();

        // Context menu overlay
        self.context_menu.draw(&mut overlay_draw);

        // Command palette overlay
        self.command_palette.draw(&mut overlay_draw, screen_w, screen_h);

        // Box select marquee
        if let Some((sx, sy)) = self.raw_input.box_select_start {
            let ex = self.raw_input.mouse_x;
            let ey = self.raw_input.mouse_y;
            let rx = sx.min(ex);
            let ry = sy.min(ey);
            let rw = (ex - sx).abs();
            let rh = (ey - sy).abs();
            // Fill
            overlay_draw.push_quad(rx, ry, rw, rh, [0.3, 0.5, 1.0, 0.15]);
            // Border (4 edges)
            overlay_draw.push_quad(rx, ry, rw, 1.0, [0.4, 0.6, 1.0, 0.8]);
            overlay_draw.push_quad(rx, ry + rh - 1.0, rw, 1.0, [0.4, 0.6, 1.0, 0.8]);
            overlay_draw.push_quad(rx, ry, 1.0, rh, [0.4, 0.6, 1.0, 0.8]);
            overlay_draw.push_quad(rx + rw - 1.0, ry, 1.0, rh, [0.4, 0.6, 1.0, 0.8]);
        }

        // Viewport labels (3D-projected object names)
        {
            self.viewport_labels.clear();
            if self.viewport_labels.visible {
                for (i, node) in self.demo.scene.iter() {
                    if !node.visible { continue; }
                    let pos = node.transform.col(3).truncate();
                    self.viewport_labels.add_3d(
                        &node.name, pos, vp, screen_w, screen_h,
                        self.selected_node == Some(i),
                    );
                }
            }
            self.viewport_labels.draw(&mut overlay_draw, screen_w, screen_h);
        }

        // Clip plane indicator
        if self.clip_plane.enabled {
            let label = format!(
                "Section: {} = {:.2}{}",
                self.clip_plane.axis.name(),
                self.clip_plane.position,
                if self.clip_plane.flipped { " (flip)" } else { "" },
            );
            // Draw indicator at top-center
            let text_w = physical_ui::font::measure_text(&label, 11.0, None);
            let lx = (screen_w - text_w - 16.0) * 0.5;
            let ly = 32.0;
            overlay_draw.push_quad(lx, ly, text_w + 16.0, 20.0, [0.15, 0.1, 0.0, 0.85]);
            overlay_draw.push_quad(lx, ly, text_w + 16.0, 2.0, [1.0, 0.6, 0.1, 0.9]);
            let mut cx = lx + 8.0;
            for c in label.chars() {
                let params = physical_ui::font::CharQuadParams {
                    c, x: cx, y: ly + 4.0, size: 11.0,
                    color: [1.0, 0.8, 0.3, 1.0], atlas: None,
                };
                cx += physical_ui::font::emit_char_quads(
                    &params, &mut overlay_draw.vertices, &mut overlay_draw.indices,
                );
            }
        }

        // Performance HUD
        self.perf_hud.draw(&mut overlay_draw, screen_w, screen_h);

        // Transform numeric input banner
        self.transform_input.draw(&mut overlay_draw, screen_w, screen_h);

        // Property panel (right side)
        {
            let top_y = self.menu_bar.height + 30.0;
            self.property_panel.draw(
                &mut overlay_draw,
                screen_w, screen_h, top_y,
                &self.theme,
            );
        }

        // Context toolbar (floating near selection)
        self.context_toolbar.draw(
            &mut overlay_draw,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Marking menu (radial right-click)
        self.marking_menu.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Selection info panel (left side, above timeline)
        {
            let si_x = self.selection_info.anchor_x;
            let si_y = screen_h - self.enhanced_status.height - self.timeline.height - 220.0;
            self.selection_info.draw(
                &mut overlay_draw,
                si_x, si_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Appearance browser (right side panel)
        if self.appearance_browser.visible {
            let ab_x = screen_w - self.appearance_browser.width - 8.0;
            let ab_y = 140.0;
            self.appearance_browser.draw(
                &mut overlay_draw,
                ab_x, ab_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Viewport splitter borders
        {
            let vp_x = 0.0;
            let vp_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            let vp_h = screen_h - vp_y - self.enhanced_status.height - self.timeline.height;
            self.viewport_splitter.draw(
                &mut overlay_draw,
                vp_x, vp_y, screen_w, vp_h,
                [self.theme.header_bg[0] + 0.1, self.theme.header_bg[1] + 0.1,
                 self.theme.header_bg[2] + 0.1, 1.0],
                self.theme.text,
                self.theme.accent,
            );
        }

        // Feature tree (left panel)
        {
            let ft_panel_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            let ft_panel_h = screen_h - ft_panel_y - self.enhanced_status.height - self.timeline.height;
            self.feature_tree.draw(
                &mut overlay_draw,
                0.0, ft_panel_y, ft_panel_h,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Section view control panel (right side)
        if self.section_view.active {
            let sv_panel_x = screen_w - self.section_view.panel_width - 8.0;
            let sv_panel_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.section_view.draw(
                &mut overlay_draw,
                sv_panel_x, sv_panel_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Measure tool readouts (on-canvas)
        self.measure_tool.draw_readouts(
            &mut overlay_draw,
            self.theme.text,
            self.theme.accent,
        );

        // Measure tool panel (right side, below section view)
        if self.measure_tool.active {
            let mt_panel_x = screen_w - self.measure_tool.panel_width - 8.0;
            let mt_panel_y = if self.section_view.active { 200.0 } else { 80.0 }
                + self.menu_bar.height + self.workspace_switcher.height;
            self.measure_tool.draw_panel(
                &mut overlay_draw,
                mt_panel_x, mt_panel_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // BOM table (centered modal)
        if self.bom_table.visible {
            let bom_x = (screen_w - self.bom_table.width) * 0.5;
            let bom_y = (screen_h - 400.0) * 0.5;
            self.bom_table.draw(
                &mut overlay_draw,
                bom_x, bom_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Drawing view (full-screen overlay when active)
        if self.drawing_view.active {
            self.drawing_view.draw(
                &mut overlay_draw,
                screen_w, screen_h,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: Exploded view controls (right panel)
        if self.exploded_view.visible {
            let ev_x = screen_w - self.exploded_view.panel_width - 8.0;
            let ev_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.exploded_view.draw(
                &mut overlay_draw,
                ev_x, ev_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: Render settings (right panel)
        {
            let rs_x = screen_w - self.render_settings.panel_width - 8.0;
            let rs_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.render_settings.draw(
                &mut overlay_draw,
                rs_x, rs_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: Assembly browser (left panel, next to feature tree)
        if self.assembly_browser.visible {
            let ab_x = if self.feature_tree.visible { self.feature_tree.width } else { 0.0 };
            let ab_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            let ab_h = screen_h - ab_y - self.enhanced_status.height - self.timeline.height;
            self.assembly_browser.draw(
                &mut overlay_draw,
                ab_x, ab_y, ab_h,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: Reference geometry (right panel)
        if self.reference_geometry.visible {
            let rg_x = screen_w - self.reference_geometry.width - 8.0;
            let rg_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 200.0;
            self.reference_geometry.draw(
                &mut overlay_draw,
                rg_x, rg_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: GDT panel (right panel)
        if self.gdt_panel.visible {
            let gdt_x = screen_w - self.gdt_panel.width - 8.0;
            let gdt_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.gdt_panel.draw(
                &mut overlay_draw,
                gdt_x, gdt_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 6: GDT annotations on canvas
        self.gdt_panel.draw_annotations(
            &mut overlay_draw,
            self.theme.text,
            self.theme.accent,
        );

        // Phase 7: Sketch tools palette (left side)
        if self.sketch_tools.visible {
            let st_x = if self.feature_tree.visible { self.feature_tree.width + 4.0 } else { 4.0 };
            let st_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            self.sketch_tools.draw(
                &mut overlay_draw,
                st_x, st_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 7: Hole wizard (centered modal)
        self.hole_wizard.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Phase 7: Sheet metal (right panel)
        if self.sheet_metal.visible {
            let sm_x = screen_w - self.sheet_metal.width - 8.0;
            let sm_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.sheet_metal.draw(
                &mut overlay_draw,
                sm_x, sm_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 7: Pattern dialog (centered modal)
        self.pattern_dialog.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Phase 7: Export dialog (centered modal)
        self.export_dialog.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Phase 8: Collaboration cursors (on canvas)
        self.collaboration.draw_cursors(&mut overlay_draw);
        self.collaboration.draw_comment_pins(&mut overlay_draw, self.theme.accent);

        // Phase 8: Collaboration panel (right side)
        if self.collaboration.visible {
            let co_x = screen_w - self.collaboration.width - 8.0;
            let co_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            self.collaboration.draw(
                &mut overlay_draw,
                co_x, co_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 8: Version history (right panel)
        if self.version_history.visible {
            let vh_x = screen_w - self.version_history.width - 8.0;
            let vh_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            let vh_h = screen_h - vh_y - self.enhanced_status.height - self.timeline.height;
            self.version_history.draw(
                &mut overlay_draw,
                vh_x, vh_y, vh_h,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 8: Data management panel (left side)
        if self.data_management.visible {
            let dm_x = if self.feature_tree.visible { self.feature_tree.width } else { 0.0 };
            let dm_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height
                + self.breadcrumb_bar.height;
            self.data_management.draw(
                &mut overlay_draw,
                dm_x, dm_y,
                self.theme.panel_bg,
                self.theme.text,
                self.theme.accent,
            );
        }

        // Phase 8: Preferences dialog (centered modal)
        self.preferences.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Phase 8: Shortcut editor (centered modal)
        self.shortcut_editor.draw(
            &mut overlay_draw,
            screen_w, screen_h,
            self.theme.panel_bg,
            self.theme.text,
            self.theme.accent,
        );

        // Confirmation corner (top-right during active operations)
        self.confirmation_corner.draw(
            &mut overlay_draw,
            screen_w,
            self.menu_bar.height + 30.0,
        );

        // Snap indicator (during drag operations)
        self.snap_indicator.draw(&mut overlay_draw, screen_w, screen_h);

        // Color picker (modal, centered)
        if self.color_picker.visible {
            // Position the picker centered on screen
            self.color_picker.x = (screen_w - 220.0) * 0.5;
            self.color_picker.y = (screen_h - 260.0) * 0.5;
            self.color_picker.draw(&mut overlay_draw);
        }

        // Tooltip overlay
        self.tooltip.draw(&mut overlay_draw);

        // Toast notifications (bottom-right, above status bar)
        self.toasts.draw(&mut overlay_draw, screen_w, screen_h);

        // Notification center (right side, below menu bar)
        {
            let nc_x = screen_w - self.notification_center.width - 8.0;
            let nc_y = self.menu_bar.height + 24.0 + self.workspace_switcher.height + 4.0;
            // Draw badge on menu bar
            let badge_x = screen_w - 40.0;
            let badge_y = self.quick_access_bar.height;
            self.notification_center.draw(
                &mut overlay_draw,
                if self.notification_center.expanded { nc_x } else { badge_x },
                if self.notification_center.expanded { nc_y } else { badge_y },
                self.theme.panel_bg,
                self.theme.text,
            );
        }

        // Progress overlay (modal, on top of almost everything)
        self.progress_overlay.draw(&mut overlay_draw, screen_w, screen_h);

        // Shortcut help overlay (modal, on top of everything)
        self.shortcut_help.draw(&mut overlay_draw, screen_w, screen_h);

        // Mate dialog (modal, on top of everything)
        self.mate_dialog.draw(&mut overlay_draw, screen_w, screen_h);

        // Render all UI layers
        // Layer 1: Workspace borders
        if !workspace_draw.vertices.is_empty() {
            self.ui_renderer.render(
                &self.device, &self.queue, &mut encoder, &view, screen_size,
                &[workspace_draw],
            );
        }

        // Layer 2: UI panels and widgets
        self.ui_renderer.render(
            &self.device, &self.queue, &mut encoder, &view, screen_size,
            &draw_lists,
        );

        // Layer 3: Viewport overlays (axes, nav cube, toolbar, status bar, context menu, palette)
        if !overlay_draw.vertices.is_empty() {
            self.ui_renderer.render(
                &self.device, &self.queue, &mut encoder, &view, screen_size,
                &[overlay_draw],
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.input.end_frame();
        self.touch_state.clear_frame(dt);
    }
}
