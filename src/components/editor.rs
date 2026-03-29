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
    prelude::{GtkWindowExt, RangeExt, WidgetExt},
    subclass::prelude::*,
};
use gtk::{
    gdk, gio, glib::{
        self, VariantTy, clone, object::{Cast, ObjectExt}, property::PropertySet, types::StaticType, variant::ToVariant
    }, prelude::BoxExt
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::Sub,
    rc::Rc,
    str::FromStr,
};
use uuid::Uuid;

use crate::{
    components::{
        canvas::BrushCanvas,
        color_chip::BrushColorChip,
        layer_item::BrushLayerItem,
        layer_tree::BrushLayerTree,
        utils::{color::oklab_to_rgba, editor_state::BrushEditorState},
    },
    data::project::BrushProject,
};

mod imp {

    use gtk::glib::WeakRef;

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

        // Editor widgets
        #[template_child]
        pub shortcut_controller: TemplateChild<gtk::ShortcutController>,
        #[template_child]
        primary_chip: TemplateChild<BrushColorChip>,
        #[template_child]
        secondary_chip: TemplateChild<BrushColorChip>,
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

        // State, stored in the editor content but needs to be referenced by UI
        pub editor_state: Rc<RefCell<BrushEditorState>>,
        pub current_project: Rc<RefCell<BrushProject>>,
        pub layer_widgets: RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>,
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

            klass.install_action("editor.new-tab", None, |editor, _, _| {
                editor.new_tab();
                let _ = adw::prelude::WidgetExt::activate_action(
                    editor,
                    "win.should-open-editor",
                    None,
                );
            });

            klass.install_action("editor.close-tab", None, |editor, _, _| {
                let tab_view = &editor.imp().tab_view;
                let page = tab_view.selected_page();
                if let Some(page) = page {
                    tab_view.close_page(&page);
                }
            });

            klass.install_action("editor.swap-colors", None, |editor, _, _| {
                editor.imp().editor_state.borrow().swap_colors();
                let state = editor.imp().editor_state.borrow();
                let primary_color = oklab_to_rgba(&state.primary_color.borrow());
                let secondary_color = oklab_to_rgba(&state.secondary_color.borrow());

                editor.emit_by_name::<()>("primary-changed", &[&primary_color]);
                editor.emit_by_name::<()>("secondary-changed", &[&secondary_color]);
            });

            klass.install_action("editor.toggle-editor", None, |editor, _, _| {
                editor.set_property("show_editor", !editor.show_editor());
            });

            klass.install_action("editor.toggle-toolbox", None, |editor, _, _| {
                editor.set_property("show_toolbox", !editor.show_toolbox());
            });

            klass.install_action("editor.new-pixel", None, |editor, _, _| {
                if let Some(tab) = editor.current_page() {
                    let _ = tab.activate_action("canvas.new-pixel", None);
                    editor.sync_project(&tab);
                }
            });

            klass.install_action("editor.new-group", None, |editor, _, _| {
                if let Some(tab) = editor.current_page() {
                    let _ = tab.activate_action("canvas.new-group", None);
                    editor.sync_project(&tab);
                }
            });

            klass.install_action(
                "editor.activate-layer",
                Some(VariantTy::STRING),
                |editor, _, arg| {
                    if let Some(var) = arg {
                        let value = var.to_string(); // 'UUID'
                        let id = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                        if let Ok(id) = Uuid::from_str(id) {
                            editor.activate_layer(id);
                        }
                    }
                },
            );

            klass.install_action(
                "editor.set-layer-opacity",
                Some(VariantTy::DOUBLE),
                |editor, _, arg| {
                    if let Some(tab) = editor.current_page() {
                        let _ = tab.activate_action("canvas.set-layer-opacity", arg);
                        editor.sync_project(&tab);
                    }
                },
            );

            klass.install_action("editor.delete-layer", None, |editor, _, _| {
                if let Some(tab) = editor.current_page() {
                    let _ = tab.activate_action("canvas.remove-layer", None);
                    editor.clear_layer_tree();
                    editor.sync_project(&tab);
                }
            });

            klass.install_action("editor.move-layer-up", None, |editor, _, _| {
                if let Some(tab) = editor.current_page() {
                    let _ = tab.activate_action("canvas.move-layer-up", None);
                    editor.clear_layer_tree();
                    editor.sync_project(&tab);
                }
            });

            klass.install_action("editor.move-layer-down", None, |editor, _, _| {
                if let Some(tab) = editor.current_page() {
                    let _ = tab.activate_action("canvas.move-layer-down", None);
                    editor.clear_layer_tree();
                    editor.sync_project(&tab);
                }
            });

            klass.install_action("editor.cancel", None, |editor, _, _| {
                editor.release_focus();
            });

            klass.install_property_action("editor.change-tool", "active_tool");
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
                                obj.obj().clear_layer_tree();
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
            }

            // Shortcuts
            obj.setup_accels();

            // Initial UI sync
            {
                let state = self.editor_state.borrow();
                let primary_color = oklab_to_rgba(&state.primary_color.borrow());
                let secondary_color = oklab_to_rgba(&state.secondary_color.borrow());

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

    fn activate_layer(&self, id: Uuid) {
        let imp = self.imp();
        let project = imp.current_project.borrow();
        let layer_widgets = imp.layer_widgets.borrow();

        if let Some(old_id) = *imp.current_layer.borrow() {
            if let Some(old_layer) = project.find_layer(old_id) {
                if let Some(entry) = layer_widgets.get(&old_id) {
                    if let Some(widget) = entry.upgrade() {
                        widget.update(Some(id), old_layer)
                    }
                }
            }
        }

        imp.current_layer.set(Some(id));

        if let Some(canvas) = self.current_page() {
            canvas.imp().active_layer.set(Some(id));
        }

        if let Some(new_layer) = project.find_layer(id) {
            if let Some(entry) = layer_widgets.get(&id) {
                if let Some(widget) = entry.upgrade() {
                    widget.update(Some(id), new_layer)
                }
            }
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

        self.imp().current_project.set(project.clone());

        self.imp().current_zoom.set(zoom);
        self.imp().current_rotation.set(rotation);
        self.sync_layers_panel(&project, canvas);
    }

    fn sync_layers_panel(&self, project: &BrushProject, canvas: &BrushCanvas) {
        let selected_layer = self.imp().current_layer.borrow();
        let layers_box = self.imp().layer_tree.get().imp().tree.get();
        let mut cache = canvas.imp().layer_widgets.borrow_mut();

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

        self.imp().layer_widgets.replace(cache.clone());
    }

    fn clear_layer_tree(&self) {
        let layers_box = self.imp().layer_tree.get().imp().tree.get();

        while let Some(child) = layers_box.first_child() {
            layers_box.remove(&child);
        }
    }

    fn current_page(&self) -> Option<BrushCanvas> {
        if let Some(tab) = self.imp().tab_view.selected_page() {
            let child = tab.child();
            if let Ok(canvas_tab) = child.downcast::<BrushCanvas>() {
                return Some(canvas_tab);
            }
        }

        None
    }

    fn setup_accels(&self) {
        let imp = self.imp();
        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::N,
                gdk::ModifierType::CONTROL_MASK,
            )),
            Some(gtk::NamedAction::new("editor.new-tab")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::W,
                gdk::ModifierType::CONTROL_MASK,
            )),
            Some(gtk::NamedAction::new("editor.close-tab")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::X,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("editor.swap-colors")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::comma,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("editor.toggle-editor")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::period,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("editor.toggle-toolbox")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::B,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::CallbackAction::new(|widget, _args| {
                let _ = widget.activate_action("editor.change-tool", Some(&"brush".to_variant()));
                glib::Propagation::Stop
            })),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::A,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::CallbackAction::new(|widget, _args| {
                let _ =
                    widget.activate_action("editor.change-tool", Some(&"box_select".to_variant()));
                glib::Propagation::Stop
            })),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Insert,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("editor.new-pixel")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Insert,
                gdk::ModifierType::SHIFT_MASK,
            )),
            Some(gtk::NamedAction::new("editor.new-group")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Up,
                gdk::ModifierType::ALT_MASK,
            )),
            Some(gtk::NamedAction::new("editor.move-layer-up")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Down,
                gdk::ModifierType::ALT_MASK,
            )),
            Some(gtk::NamedAction::new("editor.move-layer-down")),
        ));

        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::Escape,
                gdk::ModifierType::NO_MODIFIER_MASK,
            )),
            Some(gtk::NamedAction::new("editor.cancel")),
        ));
    }

    /**
        This function should be responsible for prompting to the user
        to make a new project, choosing the size and template if so.

        Then, it should generate the appropriate tab and return it.
    */
    fn new_document(&self) -> adw::TabPage {
        let tab_view = &self.imp().tab_view;
        let editor_content = BrushCanvas::new(self.imp().editor_state.clone());

        let tab_page = tab_view.append(&editor_content);
        tab_page.set_live_thumbnail(true);

        if tab_view.n_pages() > 1 {
            let title = "New Document ".to_owned() + tab_view.n_pages().to_string().as_str();
            tab_page.set_title(title.as_str());
        } else {
            let title = "New Document".to_owned();
            tab_page.set_title(title.as_str());
        }

        tab_page
    }

    /**
    This function should be responsible for creating and adding a new tab to the view.

    If provided a file it should properly load it and create the corresponding tab.
    Otherwise, it should prompt for a new document dialog
    and properly generate a new project in memory to be saved by the user.
    */
    fn new_tab(
        &self,
        // file: Option<File> or something
    ) {
        let tab_view = &self.imp().tab_view;
        let tab_page = self.new_document();

        tab_view.set_selected_page(&tab_page);
    }

    pub fn release_focus(&self) {
        if let Some(root) = self.root() {
            if let Some(window) = root.dynamic_cast_ref::<gtk::Window>() {
                window.set_focus(None::<&gtk::Widget>);
            }
        };
    }
}
