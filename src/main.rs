use adw::glib::property::PropertyGet;
use adw::prelude::*;
use adw::Application;
use gtk4::Builder;

fn main() {
    let app = Application::builder()
        .application_id("io.github.burntranch.tabcast")
        .build();

    app.connect_activate(|app| {
        let builder = Builder::from_string(include_str!("../ui/tabcast.ui"));

        let window = builder
            .object::<adw::ApplicationWindow>("window")
            .expect("Couldn't build window.");
        window.set_application(Some(app));

        let device_list = builder
            .object::<gtk4::ListBox>("device-list")
            .expect("Couldn't build device-list.");

        /* Why tf does gtk-rs not just have "get_child(idx)"?? Why do I need to get dora the explorer to find a single child??? */
        device_list.connect_row_activated(|_, list_box_row| { println!("Row #{} activated!", list_box_row.index()) });

        window.present();
    });

    app.run();
}
