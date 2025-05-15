use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::raw::c_char;
use std::thread::sleep;
use std::time::Duration;

use adw::gio;
use adw::glib;
use adw::glib::clone;
use adw::glib::property::PropertyGet;
use adw::prelude::*;
use adw::Application;
use glib::ffi::GError;
use gstreamer_app::gst::prelude::ElementExt;
use gstreamer_app::gst::Caps;
use gstreamer_app::gst::Fraction;
use gstreamer_app::AppSrc;
use gstreamer_rtsp::gst::ffi::gst_parse_launch;
use gstreamer_rtsp::gst::prelude::GstBinExt;
use gstreamer_rtsp::gst::Pipeline;
use gstreamer_rtsp::RTSPUrl;
use gstreamer_rtsp_server::prelude::RTSPMediaFactoryExt;
use gstreamer_rtsp_server::prelude::RTSPMountPointsExt;
use gstreamer_rtsp_server::prelude::RTSPServerExt;
use gstreamer_rtsp_server::prelude::RTSPServerExtManual;
use gstreamer_rtsp_server::RTSPMedia;
use gstreamer_rtsp_server::RTSPMediaFactory;
use gtk4::Builder;
use gstreamer_rtsp_server::RTSPServer;
use anyhow::Error;
use derive_more::derive::{Display, Error};

use pipewire::properties::properties;
use pipewire::spa;
use pipewire::spa::pod::Pod;
use pipewire::{self as pw, context, core, main_loop, stream};

mod tabcast_devices;
use tabcast_devices::scanner;

mod tabcast_streamer;
use tabcast_streamer::caster;

#[derive(Debug, Display, Error)]
#[display("Could not get mount points")]
struct NoMountPoints;

struct UserData {
    format: spa::param::video::VideoInfoRaw,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

        device_list.connect_row_activated(|_, list_box_row| { println!("Row #{} activated!", list_box_row.index()) });

        // Create channel that can hold at most 1 message at a time
        let (sender, receiver) = async_channel::bounded(1);
        
        // The long running operation runs now in a separate thread
        gio::spawn_blocking(move || {
            loop {
                sender
                    .send_blocking(Vec::<String>::new())
                    .expect("The channel needs to be open.");
                let devices = scanner::scan_devices();
                // Activate the button again
                sender
                    .send_blocking(devices)
                    .expect("The channel needs to be open.");

                sleep(Duration::from_secs(3));
            }
        });

        // The main loop executes the asynchronous block
        glib::spawn_future_local(clone!(
            async move {
                let mut got_something: bool = false;

                /* Every odd number of messages just indicates a search that just started. It's always an empty array. */
                while let Ok(devices) = receiver.recv().await {
                    if !got_something {
                        got_something = true;
                        scan_spinner.set_spinning(true);
                        continue;
                    }

                    /* erase all children */
                    let mut child = scan_spinner.next_sibling();
                    while !child.is_none() {
                        let child_widget = child.unwrap();
                        child = child_widget.next_sibling();
                        device_list.remove(&child_widget);
                    }

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
                    got_something = false;
                    scan_spinner.set_spinning(false);
                }
            }
        ));

        window.present();
    });

    let screencast = caster::start_casting().expect("screen cast request must be accepted");

    pw::init();
    
    let pw_mainloop = unsafe { pw::thread_loop::ThreadLoop::new(None, None) }?;
    let pw_context = pw::context::Context::new(&pw_mainloop)?;
    let pw_fd = unsafe { OwnedFd::from_raw_fd(screencast.pipewire_fd()) };

    let core = pw::context::Context::connect_fd(&pw_context, pw_fd, None)?;

    let stream = pw::stream::Stream::new(
        &core, "TabCast Stream", 
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        }
    )?;

    let data = UserData {
        format: Default::default(),
    };

    let _listener = stream
        .add_local_listener_with_user_data(data)
        .state_changed(|_, _, old, new| {
            println!("Pipewire stream changed state! {:?} => {:?}", old, new);
        })
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            
            /* We don't care about this */
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }

            let (media_type, media_subtype) = 
                match spa::param::format_utils::parse_format(param) {
                    Ok(v) => v,
                    Err(_) => return,
                };
            
            if media_type != spa::param::format::MediaType::Video || media_subtype != spa::param::format::MediaSubtype::Raw {
                return;
            }

            user_data
                .format
                .parse(param)
                .expect("Parsing param failed");

            println!("format: {} ({:?})", user_data.format.format().as_raw(), user_data.format.format());
            println!("resolution: {}x{}", user_data.format.size().width, user_data.format.size().height);
            println!("fps: {}(/{})", user_data.format.framerate().num, user_data.format.framerate().denom);
        })
        .process(|stream, user_data| {
            match stream.dequeue_buffer() {
                None => println!("Out of pipewire buffers!"),

                Some(mut buffer) => {
                    buffer.datas_mut(); /* todo: feed to gstreamer directly because pipewiresrc is being stupid and apparently only ever documented in the dark web. */
                },
            }
        })
        .register()?;

    let obj = pw::spa::pod::object!(
        pw::spa::utils::SpaTypes::ObjectParamFormat,
        pw::spa::param::ParamType::EnumFormat,
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::MediaType,
            Id,
            pw::spa::param::format::MediaType::Video
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::MediaSubtype,
            Id,
            pw::spa::param::format::MediaSubtype::Raw
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            pw::spa::param::video::VideoFormat::RGB,
            pw::spa::param::video::VideoFormat::RGB,
            pw::spa::param::video::VideoFormat::RGBA,
            pw::spa::param::video::VideoFormat::RGBx,
            pw::spa::param::video::VideoFormat::BGRx,
            pw::spa::param::video::VideoFormat::YUY2,
            pw::spa::param::video::VideoFormat::I420,
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            pw::spa::utils::Rectangle {
                width: 320,
                height: 240
            },
            pw::spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            pw::spa::utils::Rectangle {
                width: 4096,
                height: 4096
            }
        ),
        pw::spa::pod::property!(
            pw::spa::param::format::FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            pw::spa::utils::Fraction { num: 25, denom: 1 },
            pw::spa::utils::Fraction { num: 0, denom: 1 },
            pw::spa::utils::Fraction {
                num: 1000,
                denom: 1
            }
        ),
    );
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .unwrap()
    .0
    .into_inner();

    let mut params = [Pod::from_bytes(&values).unwrap()];

    stream.connect(spa::utils::Direction::Input, Some(screencast.streams().nth(0).unwrap().pipewire_node()), pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS, &mut params)?;

    pw_mainloop.start();

    gstreamer_rtsp_server::gst::init()?;
    gstreamer_rtsp_server::gst::log::set_threshold_from_string("*:3", false);

    let rtsp_server = RTSPServer::new();
    let main_loop = glib::MainLoop::new(None, false);

    let mounts = rtsp_server.mount_points().ok_or(NoMountPoints)?;

    let factory = RTSPMediaFactory::new();
    factory.set_launch("videotestsrc ! x264enc tune=zerolatency ! rtph264pay config-interval=1 name=pay0 pt=96");
    factory.set_shared(true);

    mounts.add_factory("/test", factory);

    let id = rtsp_server.attach(None);

    println!("Stream ready at rtsp://127.0.0.1:{}/test", rtsp_server.bound_port());

    main_loop.run();

    app.run();

    pw_mainloop.stop();

    Ok(())
}
