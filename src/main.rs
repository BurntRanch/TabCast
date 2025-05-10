use std::thread::sleep;
use std::time::Duration;

use adw::gio;
use adw::glib;
use adw::glib::clone;
use adw::glib::property::PropertyGet;
use adw::prelude::*;
use adw::Application;
use gtk4::Builder;
use tokio::task;

fn scan_devices() -> Vec<String> {
    let mut devices: Vec<String> = Vec::new();
    
    /* pretend there's a very interesting loop here that automagically finds every device */
    sleep(Duration::from_secs(5));
    devices.push("Test!".to_owned());

    return devices;
}

#[tokio::main]
async fn main() {
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

        let scan_spinner = builder
            .object::<gtk4::Spinner>("scan-spinner")
            .expect("Couldn't build scan-spinner.");

        /* Why tf does gtk-rs not just have "get_child(idx)"?? Why do I need to get dora the explorer to find a single child??? */
        device_list.connect_row_activated(|_, list_box_row| { println!("Row #{} activated!", list_box_row.index()) });

        // Create channel that can hold at most 1 message at a time
        let (sender, receiver) = async_channel::bounded(1);
        
        // The long running operation runs now in a separate thread
        gio::spawn_blocking(move || {
            let devices = scan_devices();
            // Activate the button again
            sender
                .send_blocking(devices)
                .expect("The channel needs to be open.");
        });

        // The main loop executes the asynchronous block
        glib::spawn_future_local(clone!(
            async move {
                while let Ok(devices) = receiver.recv().await {
                    for device in devices {
                        let device_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                        
                        let computer_icon = gtk4::Image::new();
                        computer_icon.set_icon_name(Some("computer"));
                        device_box.append(&computer_icon);

                        let device_name = gtk4::Label::new(Some(&device));
                        device_name.set_margin_start(5);
                        device_box.append(&device_name);
                        
                        let fixed = gtk4::Fixed::new();
                        fixed.set_hexpand(true);
                        device_box.append(&fixed);

                        let connect_label = gtk4::Label::new(Some("<b>Connect</b>"));
                        connect_label.set_use_markup(true);
                        connect_label.set_halign(gtk4::Align::End);
                        device_box.append(&connect_label);

                        device_list.append(&device_box);
                    }
                }
            }
        ));

        window.present();
    });

    app.run();
}
