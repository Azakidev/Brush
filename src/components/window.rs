/* window.rs
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
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::{gdk, gio, glib, prelude::*, subclass::prelude::*};
use std::ops::Deref;
use strum::IntoEnumIterator;

use crate::components::editor::{BrushEditor, EditorAction};
use crate::components::welcome::BrushWelcome;
use crate::data::file::request_open;

mod imp {
    use crate::config;

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/art/FatDawlf/Brush/window.ui")]
    pub struct BrushWindow {
        // Overlays
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub view_stack: TemplateChild<adw::ViewStack>,

        // Stack pages
        #[template_child]
        pub welcome: TemplateChild<BrushWelcome>,
        #[template_child]
        pub editor: TemplateChild<BrushEditor>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BrushWindow {
        const NAME: &'static str = "BrushWindow";
        type Type = super::BrushWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();

            WindowActions::init_actions(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BrushWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            if config::APP_ID.ends_with(".Devel") {
                obj.add_css_class("devel");
            }

            obj.setup_key_controller();
        }
    }
    impl WidgetImpl for BrushWindow {}
    impl WindowImpl for BrushWindow {}
    impl ApplicationWindowImpl for BrushWindow {}
    impl AdwApplicationWindowImpl for BrushWindow {}
}

glib::wrapper! {
    pub struct BrushWindow(ObjectSubclass<imp::BrushWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gtk::Native, gtk::Root, gtk::ShortcutManager, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gio::ActionGroup, gio::ActionMap;
}

impl BrushWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        glib::Object::builder()
            .property("application", application)
            .build()
    }

    fn setup_key_controller(&self) {
        let controller = gtk::EventControllerKey::new();

        let ws = self.downgrade();
        controller.connect_key_pressed(move |_, key, _, _| {
            if let Some(obj) = ws.upgrade()
                && let editor = &obj.imp().editor
                && let Some(canvas) = editor.current_page()
            {
                match key {
                    gdk::Key::space => canvas.imp().should_pan.set(true),
                    gdk::Key::apostrophe => canvas.imp().should_pan.set(true),
                    _ => (),
                }
            }
            glib::Propagation::Proceed
        });

        let ws = self.downgrade();
        controller.connect_key_released(move |_, key, _, _| {
            if let Some(obj) = ws.upgrade()
                && let editor = &obj.imp().editor
                && let Some(canvas) = editor.current_page()
            {
                match key {
                    gdk::Key::space => canvas.imp().should_pan.set(false),
                    gdk::Key::apostrophe => canvas.imp().should_pan.set(false),
                    _ => (),
                }
            }
        });

        self.add_controller(controller);
    }

    fn should_close_editor(&self, page_count: i32) {
        let stack = &self.imp().view_stack;
        let overview = &self.imp().editor.imp().tab_overview;

        if page_count == 0 && stack.visible_child_name().unwrap().as_str() != "welcome" {
            stack.set_visible_child_name("welcome");
            let _ = overview.activate_action("overview.close", None);
        }
    }

    fn should_open_editor(&self, page_count: i32) {
        let stack = &self.imp().view_stack;
        let editor = self.imp().editor.imp();

        if page_count > 0 && stack.visible_child_name().unwrap().as_str() != "editor" {
            editor.tab_overview.set_open(false);
            editor.obj().set_property("show_editor", true.to_value());
            editor.obj().set_property("show_toolbox", true.to_value());
            stack.set_visible_child_name("editor");
            editor.obj().release_focus();
        }
    }

    pub fn open_file(&self, path: &str) {
        let tab_view = &self.imp().editor.imp().tab_view;

        let _ = tab_view.activate_action(&EditorAction::OpenProject, Some(&path.to_variant()));
    }

    fn request_open(&self) {
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = obj)]
            self,
            async move {
                if let Ok(files) = request_open().await {
                    let file = files.first().unwrap();
                    let path = file.to_str().unwrap();
                    obj.open_file(path);
                }
            }
        ));
    }
}

#[derive(strum::Display, strum::AsRefStr, strum::EnumIter)]
pub enum WindowActions {
    #[strum(to_string = "win.new-document")]
    NewDocument,
    #[strum(to_string = "win.open-document")]
    OpenDocument,
    #[strum(to_string = "win.should-open-editor")]
    OpenEditor,
    #[strum(to_string = "win.should-close-editor")]
    CloseEditor,
}

impl Deref for WindowActions {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl WindowActions {
    fn init_actions(klass: &mut <imp::BrushWindow as ObjectSubclass>::Class) {
        for action in Self::iter() {
            match action {
                Self::NewDocument => {
                    klass.install_action(&action, None, |win, _, _| {
                        let tab_view = &win.imp().editor.imp().tab_view;
                        let _ = tab_view.activate_action(&EditorAction::NewTab, None);
                    });

                    klass.add_binding_action(gdk::Key::N, gdk::ModifierType::CONTROL_MASK, &action);
                }
                Self::OpenDocument => {
                    klass.install_action(&action, None, |win, _, _| {
                        win.request_open();
                    });

                    klass.add_binding_action(gdk::Key::O, gdk::ModifierType::CONTROL_MASK, &action);
                }
                Self::OpenEditor => {
                    klass.install_action(&action, None, |win, _, _| {
                        let tab_view = &win.imp().editor.imp().tab_view;
                        win.should_open_editor(tab_view.n_pages());
                    });
                }
                Self::CloseEditor => {
                    klass.install_action(&action, None, |win, _, _| {
                        let tab_view = &win.imp().editor.imp().tab_view;
                        win.should_close_editor(tab_view.n_pages());
                    });
                }
            }
        }
    }
}
