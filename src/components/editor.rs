/* editor.rs
 *
 * Copyright 2026 FatDawlf
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::{
    TabPage, gdk, gio,
    glib::{
        self, VariantTy, WeakRef, clone,
        object::{Cast, ObjectExt},
        property::PropertySet,
        types::StaticType,
    },
    prelude::{BoxExt, GtkWindowExt, RangeExt, ToggleButtonExt, WidgetExt},
    subclass::prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::{Deref, Sub},
    path::Path,
    rc::Rc,
    str::FromStr,
};
use strum::IntoEnumIterator;
use uuid::Uuid;

use crate::{
    components::{
        canvas::{BrushCanvas, CanvasAction},
        color_chip::BrushColorChip,
        color_selector::BrushColorSelector,
        layer_item::BrushLayerItem,
        layer_tree::BrushLayerTree,
        utils::{color::to_rgba, editor_state::BrushEditorState, tools::BrushTool},
        window::WindowActions,
    },
    data::{blend_modes::BrushBlendMode, file::open_project, project::BrushProject},
};

mod imp {

    use gtk::{gio::prelude::ListModelExt, glib::object::CastNone};

    use crate::components::color_wheel::BrushColorWheel;

    use super::*;

    #[derive(Debug, Default, glib::Properties, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/editor.ui")]
    #[properties(wrapper_type = super::BrushEditor)]
    pub struct BrushEditor {
        // Tab components
        #[template_child]
        pub toolbar_view: TemplateChild<adw::ToolbarView>,
        #[template_child]
        title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub tab_overview: TemplateChild<adw::TabOverview>,
        #[template_child]
        pub tab_view: TemplateChild<adw::TabView>,
        #[template_child]
        pub left_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub right_split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub toolbox_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub osd_wheel_revealer: TemplateChild<gtk::Revealer>,

        // Editor widgets
        #[template_child]
        pub primary_chip: TemplateChild<BrushColorChip>,
        #[template_child]
        pub secondary_chip: TemplateChild<BrushColorChip>,
        #[template_child]
        pub color_selector: TemplateChild<BrushColorSelector>,
        #[template_child]
        pub osd_wheel: TemplateChild<BrushColorWheel>,
        #[template_child]
        pub layer_tree: TemplateChild<BrushLayerTree>,
        #[template_child]
        pub brush_opacity: TemplateChild<gtk::Scale>,
        #[template_child]
        pub brush_size: TemplateChild<gtk::Scale>,
        #[template_child]
        pub brush_opacity_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub brush_size_label: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub eraser_toggle: TemplateChild<gtk::ToggleButton>,

        // State, stored in the canvas but needs to be referenced by editor
        pub editor_state: Rc<RefCell<BrushEditorState>>,
        pub layer_widget_cache: RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>,
        pub current_layer: Rc<RefCell<Option<Uuid>>>,
        pub current_zoom: Rc<Cell<f32>>,
        pub current_rotation: Rc<Cell<f32>>,

        // Properties
        #[property(get, set)]
        active_tool: RefCell<String>,
        #[property(get, set)]
        show_editor: RefCell<bool>,
        #[property(get, set)]
        show_toolbox: RefCell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushEditor {
        const NAME: &'static str = "BrushEditor";
        type Type = super::BrushEditor;
        type ParentType = gtk::Box;

        fn new() -> Self {
            Self {
                active_tool: RefCell::new("brush".to_owned()),
                show_editor: RefCell::new(true),
                show_toolbox: RefCell::new(true),
                ..Default::default()
            }
        }

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            EditorAction::init_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushEditor {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);

            if pspec.name() == "active-tool" {
                let tool_name = value.get::<String>().unwrap();
                self.editor_state.borrow_mut().set_tool(&tool_name);
            }
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: std::sync::OnceLock<Vec<glib::subclass::Signal>> =
                std::sync::OnceLock::new();

            SIGNALS.get_or_init(|| {
                vec![
                    glib::subclass::Signal::builder("primary-changed")
                        .param_types([gdk::RGBA::static_type()])
                        .build(),
                    glib::subclass::Signal::builder("secondary-changed")
                        .param_types([gdk::RGBA::static_type()])
                        .build(),
                ]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Make shortcut controller have a managed scope so accels work without focused
            {
                let list = obj.observe_controllers();

                for i in 0..list.n_items() {
                    if let Some(controller) = list.item(i).and_downcast::<gtk::ShortcutController>()
                    {
                        controller.set_scope(gtk::ShortcutScope::Managed);
                    }
                }
            }

            let primary = self.primary_chip.get();
            let secondary = self.secondary_chip.get();

            // Tab management
            {
                let title = self.title.get();

                self.tab_view.connect_selected_page_notify(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |view| {
                        if let Some(page) = view.selected_page() {
                            page.bind_property("title", &title, "title")
                                .sync_create()
                                .build();

                            let child = page.child();
                            if let Ok(canvas_tab) = child.downcast::<BrushCanvas>() {
                                canvas_tab.imp().layer_widget_cache.borrow_mut().clear();
                                obj.obj().sync_project(&canvas_tab);
                            }
                        }
                    }
                ));

                self.tab_view.connect_page_detached(move |tab_view, _, _| {
                    let _ = tab_view.activate_action("win.should-close-editor", None);
                });

                obj.bind_property("show_editor", &self.toolbar_view.get(), "reveal-top-bars")
                    .sync_create()
                    .build();
                obj.bind_property("show_editor", &self.left_split.get(), "show-sidebar")
                    .sync_create()
                    .build();
                obj.bind_property("show_editor", &self.right_split.get(), "show-sidebar")
                    .sync_create()
                    .build();
                obj.bind_property("show_toolbox", &self.toolbox_revealer.get(), "reveal-child")
                    .sync_create()
                    .build();

                obj.connect_show_toolbox_notify(|s| {
                    if s.show_toolbox() && !s.show_editor() {
                        s.imp().osd_wheel_revealer.set_reveal_child(true);
                    } else {
                        s.imp().osd_wheel_revealer.set_reveal_child(false);
                    }
                });
                obj.connect_show_editor_notify(|s| {
                    if s.show_toolbox() && !s.show_editor() {
                        s.imp().osd_wheel_revealer.set_reveal_child(true);
                    } else {
                        s.imp().osd_wheel_revealer.set_reveal_child(false);
                    }
                });

                let selector = obj.imp().color_selector.get();

                obj.imp()
                    .osd_wheel
                    .bind_property("h", &selector, "h")
                    .sync_create()
                    .bidirectional()
                    .build();
                obj.imp()
                    .osd_wheel
                    .bind_property("s", &selector, "s")
                    .sync_create()
                    .bidirectional()
                    .build();
                obj.imp()
                    .osd_wheel
                    .bind_property("v", &selector, "v")
                    .sync_create()
                    .bidirectional()
                    .build();
            }

            // Initial UI sync
            {
                let state = self.editor_state.borrow();
                let primary_color = to_rgba(&state.primary_color.borrow());
                let secondary_color = to_rgba(&state.secondary_color.borrow());

                primary.set_color(primary_color);
                secondary.set_color(secondary_color);
            }

            // Setup signal listeners
            {
                obj.connect_local("primary-changed", false, move |args| {
                    let rgba = args[1].get::<gdk::RGBA>().unwrap();
                    primary.set_color(rgba);
                    None
                });

                obj.connect_local("secondary-changed", false, move |args| {
                    let rgba = args[1].get::<gdk::RGBA>().unwrap();
                    secondary.set_color(rgba);
                    None
                });

                obj.imp().brush_opacity.connect_value_changed(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |s| {
                        let val = s.value();
                        let obj = obj.obj();
                        let imp = obj.imp();
                        let label = &imp.brush_opacity_label;

                        label.set_value(val * 100f64);
                    }
                ));

                obj.imp().brush_opacity_label.connect_value_changed(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |l| {
                        let obj = obj.obj();
                        let imp = obj.imp();
                        let slider = &imp.brush_opacity;
                        let state = imp.editor_state.borrow();

                        let val = (l.value() / 100f64).clamp(0f64, 1f64);

                        slider.set_value(val);
                        state.set_brush_opacity(val as f32);
                    }
                ));

                obj.imp().brush_size.connect_value_changed(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |s| {
                        let obj = obj.obj();
                        let imp = obj.imp();
                        let label = &imp.brush_size_label;

                        let val = s.value().clamp(0.00001f64, 1f64);

                        let min_val = 1.0f64;
                        let max_val = 1000.0f64;
                        let mapped_val = min_val * (max_val / min_val).powf(val);

                        label.set_value(mapped_val);
                    }
                ));

                obj.imp().brush_size_label.connect_value_changed(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    move |l| {
                        let obj = obj.obj();
                        let imp = obj.imp();
                        let slider = &imp.brush_size;
                        let state = imp.editor_state.borrow();

                        let val = l.value();
                        let clamped = val.clamp(1f64, 1000f64);
                        let min_val = 1.0f64;
                        let max_val = 1000.0f64;

                        let slider_pos = (clamped / min_val).ln() / (max_val / min_val).ln();
                        slider.set_value(slider_pos);
                        state.set_brush_size(val as u32);
                    }
                ));
            }
        }
    }
    impl WidgetImpl for BrushEditor {}
    impl BoxImpl for BrushEditor {}
}

glib::wrapper! {
    pub struct BrushEditor(ObjectSubclass<imp::BrushEditor>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gio::ActionGroup, gio::ActionMap;
}

impl BrushEditor {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn current_project(&self) -> RefCell<BrushProject> {
        let canvas = self.current_page().unwrap();
        canvas.project_context()
    }

    // TODO: Linear time activation?
    fn activate_layer(&self, id: Uuid) {
        let imp = self.imp();
        let project_rc = self.current_project();
        let project = project_rc.borrow();
        let layer_widget_cache = imp.layer_widget_cache.borrow();

        if let Some(old_id) = *imp.current_layer.borrow()
            && let Some(old_layer) = project.find_layer(old_id)
            && let Some(entry) = layer_widget_cache.get(&old_id)
            && let Some(widget) = entry.upgrade()
        {
            widget.update(Some(id), old_layer)
        }

        imp.current_layer.set(Some(id));
        self.update_controls(&project);

        if let Some(canvas) = self.current_page() {
            canvas.imp().active_layer.set(Some(id));
        }

        if let Some(new_layer) = project.find_layer(id)
            && let Some(entry) = layer_widget_cache.get(&id)
            && let Some(widget) = entry.upgrade()
        {
            widget.update(Some(id), new_layer)
        }
    }

    fn update_controls(&self, project: &BrushProject) {
        let imp = self.imp();
        let layer_tree = imp.layer_tree.imp();
        // Widgets
        let opacity_slider = &layer_tree.layer_opacity;
        let blend_mode_dropdown = &layer_tree.blend_mode;
        // Values
        if let Some(active_id) = *imp.current_layer.borrow()
            && let Some(layer) = project.find_layer(active_id)
        {
            let opacity = layer.opacity();
            let blend_mode = layer.blend_mode();
            let blend_idx = BrushBlendMode::iter()
                .position(|b| b == blend_mode)
                .unwrap();

            // Update
            layer_tree.should_update.set(false);
            opacity_slider.set_value(opacity as f64);
            blend_mode_dropdown.set_selected(blend_idx as u32);
            layer_tree.should_update.set(true);
        }
    }

    fn sync_project(&self, canvas: &BrushCanvas) {
        let canvas_project = canvas.project_context();
        let project = canvas_project.borrow();

        let zoom = canvas.zoom();
        let rotation = canvas.rotation();

        if let Some(selected_layer) = canvas.selected_layer() {
            self.imp().current_layer.set(Some(selected_layer));
        }

        self.imp().current_zoom.set(zoom);
        self.imp().current_rotation.set(rotation);

        self.sync_layers_panel(&project, canvas);
    }

    // TODO: Async/multithreaded widget creation
    fn sync_layers_panel(&self, project: &BrushProject, canvas: &BrushCanvas) {
        let selected_layer = self.imp().current_layer.borrow();
        let layers_box = self.imp().layer_tree.get().imp().tree.get();
        let mut cache = canvas.imp().layer_widget_cache.borrow_mut();

        let mut layers: Vec<BrushLayerItem> = Vec::new();

        for layer in &project.layers {
            let item = BrushLayerItem::new(layer, *selected_layer, &mut cache);
            layers.push(item);
        }

        while let Some(child) = layers_box.first_child() {
            layers_box.remove(&child);
        }

        for item in layers {
            layers_box.append(&item);
        }

        self.imp().layer_widget_cache.replace(cache.clone());
        self.update_controls(project);
    }

    pub fn current_page(&self) -> Option<BrushCanvas> {
        if let Some(tab) = self.imp().tab_view.selected_page() {
            let child = tab.child();
            if let Ok(canvas_tab) = child.downcast::<BrushCanvas>() {
                return Some(canvas_tab);
            }
        }

        None
    }

    fn rename_tab(&self, new_name: &str) {
        if let Some(tab) = self.imp().tab_view.selected_page() {
            tab.set_title(new_name);
        }
    }

    /**
        This function should be responsible for prompting to the user
        to make a new project, choosing the size and template if so.

        Then, it should generate the appropriate tab and call new_tab on it.
    */
    fn new_document(&self) {
        let tab_view = &self.imp().tab_view;
        let canvas = BrushCanvas::new(self.imp().editor_state.clone());

        let tab_page = tab_view.append(&canvas);
        tab_page.set_live_thumbnail(true);

        if tab_view.n_pages() > 1 {
            let title = "New Document ".to_owned() + tab_view.n_pages().to_string().as_str();
            tab_page.set_title(title.as_str());
        } else {
            let title = "New Document".to_owned();
            tab_page.set_title(title.as_str());
        }

        self.new_tab(&tab_page);
    }

    fn open_document(&self, loc: &str) {
        let loc = loc.to_string();

        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                let tab_view = &obj.imp().tab_view;
                let file_path = loc.clone();

                let result =
                    gtk::gio::spawn_blocking(move || open_project(Path::new(loc.as_str())))
                        .await
                        .expect("Failed to finish save");
                match result {
                    Ok(p) => {
                        let canvas = BrushCanvas::from_project(
                            obj.imp().editor_state.clone(),
                            p,
                            &file_path,
                        );

                        let path = Path::new(file_path.as_str());
                        let title = path.file_name().unwrap().to_str().unwrap();

                        let tab_page = tab_view.append(&canvas);
                        tab_page.set_live_thumbnail(true);
                        tab_page.set_title(title);
                        obj.new_tab(&tab_page);
                    }
                    Err(e) => {
                        eprintln!("{e}");
                    }
                }
            }
        ));
    }

    fn new_tab(&self, tab: &TabPage) {
        let tab_view = &self.imp().tab_view;
        tab_view.set_selected_page(tab);
        let _ = tab_view.activate_action(&WindowActions::OpenEditor, None);
    }

    pub fn release_focus(&self) {
        if let Some(root) = self.root()
            && let Some(window) = root.dynamic_cast_ref::<gtk::Window>()
        {
            window.set_focus(None::<&gtk::Widget>);
        };
    }
}

#[derive(strum::Display, strum::EnumIter, strum::AsRefStr)]
pub enum EditorAction {
    // Document management
    #[strum(to_string = "editor.new-tab")]
    NewTab,
    #[strum(to_string = "editor.rename-tab")]
    RenameTab,
    #[strum(to_string = "editor.close-tab")]
    CloseTab,
    // Editor state changes
    #[strum(to_string = "editor.cancel")]
    Cancel,
    #[strum(to_string = "editor.swap-colors")]
    SwapColors,
    #[strum(to_string = "editor.set-color")]
    SetColor,
    #[strum(to_string = "editor.toggle-editor")]
    ToggleEditor,
    #[strum(to_string = "editor.toggle-toolbox")]
    ToggleToolbox,
    #[strum(to_string = "editor.change-tool")]
    SetTool,
    #[strum(to_string = "editor.toggle-erase")]
    ToggleErase,
    // Project management
    #[strum(to_string = "editor.open")]
    OpenProject,
    #[strum(to_string = "editor.save")]
    SaveProject,
    #[strum(to_string = "editor.save-as")]
    SaveProjectAs,
    #[strum(to_string = "editor.export-as")]
    ExportProjectAs,
    // Layer handling
    #[strum(to_string = "editor.new-pixel")]
    NewPixel,
    #[strum(to_string = "editor.new-group")]
    NewGroup,
    #[strum(to_string = "editor.activate-layer")]
    ActivateLayer,
    #[strum(to_string = "editor.delete-layer")]
    DeleteLayer,
    #[strum(to_string = "editor.move-layer-up")]
    MoveLayerUp,
    #[strum(to_string = "editor.move-layer-down")]
    MoveLayerDown,
    // Layer modifications
    #[strum(to_string = "editor.set-layer-opacity")]
    SetLayerOpacity,
    #[strum(to_string = "editor.set-layer-blend")]
    SetLayerBlendMode,
    #[strum(to_string = "editor.toggle-lock")]
    ToggleLock,
    #[strum(to_string = "editor.toggle-visible")]
    ToggleVisible,
    #[strum(to_string = "editor.toggle-alpha-clip")]
    ToggleAlphaClip,
    #[strum(to_string = "editor.toggle-alpha-lock")]
    ToggleAlphaLock,
    #[strum(to_string = "editor.toggle-passthrough")]
    TogglePassthrough,
}

impl Deref for EditorAction {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl EditorAction {
    fn init_actions(klass: &mut <imp::BrushEditor as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                // Document management
                Self::NewTab => {
                    klass.install_action(&action, None, |e, _, _| {
                        e.new_document();
                    });
                }
                Self::RenameTab => {
                    klass.install_action(&action, Some(VariantTy::STRING), |e, _, arg| {
                        if let Some(var) = arg {
                            let value = var.to_string(); // 'Name'
                            let name = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                            e.rename_tab(name);
                        }
                    });
                }
                Self::CloseTab => {
                    klass.install_action(&action, None, |e, _, _| {
                        let tab_view = &e.imp().tab_view;
                        let page = tab_view.selected_page();
                        if let Some(page) = page {
                            tab_view.close_page(&page);
                        }
                    });

                    klass.add_binding_action(gdk::Key::W, gdk::ModifierType::CONTROL_MASK, &action);
                }
                // Editor state changes
                Self::ToggleEditor => {
                    klass.install_action(&action, None, |e, _, _| {
                        e.set_property("show_editor", !e.show_editor());
                    });

                    klass.add_binding_action(
                        gdk::Key::comma,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::ToggleToolbox => {
                    klass.install_action(&action, None, |e, _, _| {
                        e.set_property("show_toolbox", !e.show_toolbox());
                    });

                    klass.add_binding_action(
                        gdk::Key::period,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::Cancel => {
                    klass.install_action(&action, None, |e, _, _| {
                        e.release_focus();
                    });

                    klass.add_binding_action(
                        gdk::Key::Escape,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::SwapColors => {
                    klass.install_action(&action, None, |e, _, _| {
                        e.imp().editor_state.borrow().swap_colors();
                        let state = e.imp().editor_state.borrow();
                        let selector = &e.imp().color_selector;

                        let primary_color = to_rgba(&state.primary_color.borrow());
                        let secondary_color = to_rgba(&state.secondary_color.borrow());

                        e.emit_by_name::<()>("primary-changed", &[&primary_color]);
                        e.emit_by_name::<()>("secondary-changed", &[&secondary_color]);
                        selector.set_color(&state.primary_color.borrow());
                    });

                    klass.add_binding_action(
                        gdk::Key::X,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::SetColor => {
                    klass.install_action(&action, None, |e, _, _| {
                        let state = e.imp().editor_state.borrow();
                        let selector = &e.imp().color_selector;
                        let color = selector.color();

                        state.primary_color.replace(color);

                        let rgb = to_rgba(&color);
                        e.emit_by_name::<()>("primary-changed", &[&rgb]);
                    });
                }
                Self::SetTool => {
                    klass.install_property_action(&action, "active_tool");

                    // TODO: Binds for the rest of the tools
                    klass.add_binding(gdk::Key::B, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::Brush.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::F, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::Fill.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::R, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::Box.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::J, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::Ellipse.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::S, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::SelectBox.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::A, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::SelectLasso.as_ref());
                        glib::Propagation::Stop
                    });

                    klass.add_binding(gdk::Key::W, gdk::ModifierType::NO_MODIFIER_MASK, |e| {
                        e.set_active_tool(BrushTool::SelectWand.as_ref());
                        glib::Propagation::Stop
                    });
                }
                Self::ToggleErase => {
                    klass.install_action(&action, None, |e, _, _| {
                        let state = e.imp().editor_state.borrow();
                        let mode = !*state.erase_mode.borrow();
                        state.set_erase_mode(mode);
                        e.imp().eraser_toggle.set_active(mode);
                    });

                    klass.add_binding_action(
                        gdk::Key::E,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                // Project handling
                Self::OpenProject => {
                    klass.install_action(&action, Some(VariantTy::STRING), |e, _, arg| {
                        if let Some(var) = arg {
                            let value = var.to_string(); // 'path'
                            let path = value.get(1..value.len().sub(1)); // Remove quotes
                            e.open_document(path.unwrap());
                        }
                    });
                }
                Self::SaveProject => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::SaveProject, None);
                        }
                    });
                }
                Self::SaveProjectAs => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::SaveProjectAs, None);
                        }
                    });
                }
                Self::ExportProjectAs => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::ExportProjectAs, None);
                        }
                    });
                }
                // Layer handling
                Self::NewPixel => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::NewPixel, None);
                            e.sync_project(&tab);
                        }
                    });

                    klass.add_binding_action(
                        gdk::Key::Insert,
                        gdk::ModifierType::NO_MODIFIER_MASK,
                        &action,
                    );
                }
                Self::NewGroup => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::NewGroup, None);
                            e.sync_project(&tab);
                        }
                    });

                    klass.add_binding_action(
                        gdk::Key::Insert,
                        gdk::ModifierType::SHIFT_MASK,
                        &action,
                    );
                }
                Self::ActivateLayer => {
                    klass.install_action(&action, Some(VariantTy::STRING), |e, _, arg| {
                        if let Some(var) = arg {
                            let value = var.to_string(); // 'UUID'
                            let id = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                            if let Ok(id) = Uuid::from_str(id) {
                                e.activate_layer(id);
                            }
                        }
                    });
                }
                Self::DeleteLayer => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::DeleteLayer, None);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::MoveLayerUp => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::MoveLayerUp, None);
                            e.sync_project(&tab);
                        }
                    });

                    klass.add_binding_action(gdk::Key::Up, gdk::ModifierType::ALT_MASK, &action);
                }
                Self::MoveLayerDown => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::MoveLayerDown, None);
                            e.sync_project(&tab);
                        }
                    });

                    klass.add_binding_action(gdk::Key::Down, gdk::ModifierType::ALT_MASK, &action);
                }
                // Layer modifications
                Self::SetLayerOpacity => {
                    klass.install_action(&action, Some(VariantTy::DOUBLE), |e, _, arg| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::SetLayerOpacity, arg);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::SetLayerBlendMode => {
                    klass.install_action(&action, Some(VariantTy::UINT32), |e, _, arg| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::SetLayerBlendMode, arg);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::ToggleVisible => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::ToggleVisible, None);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::ToggleAlphaClip => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::ToggleAlphaClip, None);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::ToggleAlphaLock => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::ToggleAlphaLock, None);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::TogglePassthrough => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::TogglePassthrough, None);
                            e.sync_project(&tab);
                        }
                    });
                }
                Self::ToggleLock => {
                    klass.install_action(&action, None, |e, _, _| {
                        if let Some(tab) = e.current_page() {
                            let _ = tab.activate_action(&CanvasAction::ToggleLock, None);
                            e.sync_project(&tab);
                        }
                    });
                }
            }
        }
    }
}
