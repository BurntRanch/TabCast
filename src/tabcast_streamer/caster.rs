use portal_screencast::{self, ActiveScreenCast, SourceType};

pub fn start_casting() -> Result<ActiveScreenCast, Box<dyn std::error::Error>> {
    let mut screen_cast = portal_screencast::ScreenCast::new().unwrap();

    screen_cast.set_source_types(SourceType::MONITOR | SourceType::WINDOW);
    screen_cast.enable_multiple();

    let active_cast = screen_cast.start(None);

    match active_cast {
        Ok(active_screen_cast) => { Ok(active_screen_cast) }

        Err(err) => { Err(err)? }
    }
}
