use std::{thread::sleep, time::Duration};

pub fn scan_devices() -> Vec<String> {
    let mut devices: Vec<String> = Vec::new();
    
    /* pretend there's a very interesting loop here that automagically finds every device */
    sleep(Duration::from_secs(5));
    devices.push("Test!".to_owned());

    return devices;
}
