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

        let testbutton = builder
            .object::<gtk4::Button>("test-button")
            .expect("Couldn't build test-button.");
        testbutton.connect_clicked(|_| { println!("Clicked!") });

        window.present();
    });

    app.run();
}
