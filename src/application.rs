/* application.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::BrushWindow;
use crate::config::VERSION;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct BrushApplication {}

    #[glib::object_subclass]
    impl ObjectSubclass for BrushApplication {
        const NAME: &'static str = "BrushApplication";
        type Type = super::BrushApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for BrushApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_gactions();
        }
    }

    impl ApplicationImpl for BrushApplication {
        // We connect to the activate callback to create a window when the application
        // has been launched. Additionally, this callback notifies us when the user
        // tries to launch a "second instance" of the application. When they try
        // to do that, we'll just present any existing window.
        fn activate(&self) {
            let application = self.obj();
            // Get the current window or create one if necessary
            let window = application.active_window().unwrap_or_else(|| {
                let window = BrushWindow::new(&*application);
                window.upcast()
            });

            application.setup_icons();

            // Ask the window manager/compositor to present the window
            window.present();
        }
    }

    impl GtkApplicationImpl for BrushApplication {}
    impl AdwApplicationImpl for BrushApplication {}
}

glib::wrapper! {
    pub struct BrushApplication(ObjectSubclass<imp::BrushApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl BrushApplication {
    pub fn new(application_id: &str, flags: &gio::ApplicationFlags) -> Self {
        glib::Object::builder()
            .property("application-id", application_id)
            .property("flags", flags)
            .property("resource-base-path", "/art/FatDawlf/Brush")
            .build()
    }

    fn setup_gactions(&self) {
        let actions = [
            gio::ActionEntry::builder("quit")
                .activate(|app: &Self, _, _| app.quit())
                .build(),
            gio::ActionEntry::builder("about")
                .activate(|app: &Self, _, _| app.show_about())
                .build(),
        ];

        self.add_action_entries(actions);

        self.set_accels_for_action("app.quit", &["<Ctrl>q"]);
    }

    fn setup_icons(&self) {
        let display = gtk::gdk::Display::default().expect("Could not connect to a display.");
        let icon_theme = gtk::IconTheme::for_display(&display);

        icon_theme.add_resource_path("/art/FatDawlf/Brush/icons");
    }

    fn show_about(&self) {
        let window = self.active_window().unwrap();
        let about = adw::AboutDialog::builder()
            .application_name("Brush")
            .application_icon("art.FatDawlf.Brush")
            .developer_name("FatDawlf")
            .version(VERSION)
            .developers(vec!["FatDawlf https://fatdawlf.art"])
            // Translators: Replace "translator-credits" with your name/username, and optionally an email or URL.
            .translator_credits(gettext("translator-credits"))
            .license_type(gtk::License::Gpl30)
            .copyright("© 2026 FatDawlf")
            .build();

        about.present(Some(&window));
    }
}
