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

use adw::{prelude::WidgetExt, subclass::prelude::*, TabPage};
use gtk::glib::property::PropertySet;
use gtk::glib::VariantTy;
use gtk::{
    gdk, gio,
    glib::{
        self, clone,
        object::{Cast, ObjectExt},
        types::StaticType,
        variant::ToVariant,
        WeakRef,
    },
    prelude::BoxExt,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use uuid::Uuid;

use crate::{
    components::{
        color_chip::BrushColorChip,
        editor_content::BrushEditorContent,
        layer_item::BrushLayerItem,
        layer_tree::BrushLayerTree,
        utils::{color::oklab_to_rgba, editor_state::BrushEditorState},
    },
    data::project::BrushProject,
};
use std::ops::Sub;

mod imp {

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

        // State, stored in the editor content but needs to be referenced by UI
        pub editor_state: Rc<RefCell<BrushEditorState>>,
        pub current_project: Rc<RefCell<BrushProject>>,
        pub layer_widgets: Rc<RefCell<HashMap<Uuid, WeakRef<BrushLayerItem>>>>,
        pub current_layer: Rc<RefCell<Uuid>>,
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
                let primary_color = oklab_to_rgba(state.primary_color.clone().into_inner());
                let secondary_color = oklab_to_rgba(state.secondary_color.clone().into_inner());

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
                if let Some(tab) = editor.imp().tab_view.selected_page() {
                    let _ = tab.child().activate_action("canvas.new-pixel", None);
                    editor.sync_tab(tab);
                }
            });

            klass.install_action("editor.new-group", None, |editor, _, _| {
                if let Some(tab) = editor.imp().tab_view.selected_page() {
                    let _ = tab.child().activate_action("canvas.new-group", None);
                    editor.sync_tab(tab);
                }
            });

            klass.install_action(
                "editor.activate-layer",
                Some(VariantTy::STRING),
                |editor, _, arg| {
                    if let Some(var) = arg {
                        let value = var.to_string(); // 'UUID'
                        let id = value.get(1..value.len().sub(1)).unwrap(); // Remove quotes
                        if let Ok(id) = Uuid::from_str(&id) {
                            editor.activate_layer(id);
                        }
                    }
                },
            );

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
                            if let Ok(canvas_tab) = child.downcast::<BrushEditorContent>() {
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
                let primary_color = oklab_to_rgba(state.primary_color.clone().into_inner());
                let secondary_color = oklab_to_rgba(state.secondary_color.clone().into_inner());

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

        let old_id = imp.current_layer.borrow().clone();

        let page = imp
            .tab_view
            .selected_page()
            .expect("Can't select a layer without being in a tab")
            .child()
            .downcast::<BrushEditorContent>()
            .expect("There's no other option for a child of the tab view");

        imp.current_layer.set(id);
        page.imp().active_layer.set(Some(id));

        if let Some(old_layer) = project.find_layer(old_id) {
            if let Some(old_widget) = layer_widgets.get(&old_id) {
                if let Some(widget) = old_widget.upgrade() {
                    widget.update(&id, old_layer)
                }
            }
        }

        if let Some(new_layer) = project.find_layer(id) {
            if let Some(new_widget) = layer_widgets.get(&id) {
                if let Some(widget) = new_widget.upgrade() {
                    widget.update(&id, new_layer)
                }
            }
        }
    }

    fn sync_tab(&self, page: TabPage) {
        let child = page.child();
        if let Ok(canvas_tab) = child.downcast::<BrushEditorContent>() {
            self.sync_project(&canvas_tab);
        }
    }

    fn sync_project(&self, tab: &BrushEditorContent) {
        let project = tab.project_context();
        let layer_widgets = tab.widget_cache();

        let project_borrow = project.borrow();
        let layer_borrow = layer_widgets.borrow();

        let zoom = tab.zoom();
        let rotation = tab.rotation();

        if let Some(selected_layer) = tab.selected_layer() {
            self.imp().current_layer.set(selected_layer);
        }

        self.imp().current_project.set(project_borrow.clone());
        self.imp().layer_widgets.set(layer_borrow.clone());

        self.imp().current_zoom.set(zoom);
        self.imp().current_rotation.set(rotation);
        self.sync_layers_panel();
    }

    fn sync_layers_panel(&self) {
        let project = self.imp().current_project.borrow();
        let selected_layer = self.imp().current_layer.borrow();
        let layers_box = self.imp().layer_tree.get().imp().tree.get();
        let mut widget_cache = self.imp().layer_widgets.borrow_mut();

        while let Some(child) = layers_box.first_child() {
            layers_box.remove(&child);
        }

        for layer in &project.layers {
            let item = BrushLayerItem::new(layer, &selected_layer, &mut widget_cache);
            layers_box.append(&item);
        }
    }

    fn setup_accels(&self) {
        let imp = self.imp();
        imp.shortcut_controller.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(
                gdk::Key::T,
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
    }

    /**
        This function should be responsible for prompting to the user
        to make a new project, choosing the size and template if so.

        Then, it should generate the appropriate tab and return it.
    */
    fn new_document(&self) -> adw::TabPage {
        let tab_view = &self.imp().tab_view;
        let editor_content = BrushEditorContent::new(self.imp().editor_state.clone());

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
}
