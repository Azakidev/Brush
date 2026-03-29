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

use crate::components::editor::BrushEditor;
use crate::components::welcome::BrushWelcome;
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

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

            klass.install_action("win.new-document", None, |win, _, _| {
                let tab_view = &win.imp().editor.imp().tab_view;
                let _ = tab_view.activate_action("editor.new-tab", None);
            });

            klass.install_action("win.should-open-editor", None, |win, _, _| {
                let tab_view = &win.imp().editor.imp().tab_view;
                win.should_open_editor(tab_view.n_pages());
            });

            klass.install_action("win.should-close-editor", None, |win, _, _| {
                let tab_view = &win.imp().editor.imp().tab_view;
                win.should_close_editor(tab_view.n_pages());
            });
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
        let overview = &editor.tab_overview;

        if page_count > 0 && stack.visible_child_name().unwrap().as_str() != "editor" {
            overview.set_open(false);
            editor.obj().set_property("show_editor", true.to_value());
            editor.obj().set_property("show_toolbox", true.to_value());
            stack.set_visible_child_name("editor");
            editor.obj().release_focus();
        }
    }
}
