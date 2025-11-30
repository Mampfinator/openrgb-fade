use std::{collections::HashSet, path::PathBuf, str::FromStr, time::Duration};

use openrgb2::{Color, Controller, DeviceType, OpenRgbClient, OpenRgbResult};

use crate::{
    config::{Config, get_config_dir},
    fade::FadeLeds,
    hid::{HidReader, KeyEvent},
    key_mappings::KeyMapping,
};

mod config;
mod fade;
mod hid;
mod key_mappings;

static BASE_COLOR: Color = Color::new(255, 100, 255);

fn get_keymap_filepath(controller: &Controller) -> PathBuf {
    let vendor = controller.vendor().to_lowercase().replace(" ", "_");
    let name = controller.name().to_lowercase().replace(" ", "_");

    let config = get_config_dir();
    config.join(PathBuf::from_str(&format!("{}-{}.keymap", vendor, name)).unwrap())
}

async fn setup_device(device: &Controller) -> OpenRgbResult<KeyMapping> {
    let mut hid = HidReader::<512>::new_from_path(device.location()).unwrap();

    println!("Press the keys as they light up.");

    let mut seen = HashSet::new();

    let mut get_next_unique_event = move || {
        loop {
            let event = hid.read_blocking().unwrap();
            let key = event.key_bytes();

            if !seen.contains(&key) {
                seen.insert(key);
                return event;
            }
        }
    };

    let mut keys = Vec::with_capacity(device.num_leds());

    for led in device.led_iter() {
        device.turn_off_leds().await?;
        led.set_led(BASE_COLOR).await?;

        let event = get_next_unique_event();

        keys.push(event.key_bytes());
    }

    Ok(KeyMapping::from(keys))
}

async fn wait_for_server() -> OpenRgbClient {
    for i in 0.. {
        if let Ok(client) = OpenRgbClient::connect().await {
            return client;
        } else {
            let ms = (250 * i).min(10000);
            println!("Could not connect to OpenRGB SDK server. Retrying in {ms}ms.");
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
    }
    unreachable!()
}

pub trait LedFunction {
    fn new(controller: &Controller) -> Self
    where
        Self: Sized;

    fn update(
        &mut self,
        configuration: &Config,
        events: &[KeyEvent],
        key_map: &KeyMapping,
        controller: &Controller,
    ) -> OpenRgbResult<()>;
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> OpenRgbResult<()> {
    let client = wait_for_server().await;

    let config = Config::load_from_first().unwrap();

    let keyboard_controller = client
        .get_controllers_of_type(DeviceType::Keyboard)
        .await?
        .into_first()?;

    keyboard_controller.init().await?;
    keyboard_controller.turn_off_leds().await?;

    let path = get_keymap_filepath(&keyboard_controller);

    println!("Using keymap file at {}", path.to_string_lossy());

    let ledmap = match std::fs::read_to_string(&path) {
        Err(_) => {
            println!("No matching keymap file found. Starting setup.");
            let map = setup_device(&keyboard_controller).await?;
            std::fs::write(path, map.as_file_string()).unwrap();
            map
        }
        Ok(file) => KeyMapping::parse_from_file(file).unwrap(),
    };

    let (tx, rx) = std::sync::mpsc::channel::<KeyEvent>();
    let mut device = HidReader::<512>::new_from_path(keyboard_controller.location()).unwrap();
    tokio::spawn(async move {
        loop {
            tx.send(device.read_blocking().unwrap()).unwrap()
        }
    });

    let sleep_time = 1000 / config.fps() as u64;
    println!("Frame time: {sleep_time}ms");

    let mut func: Box<dyn LedFunction> =
        Box::new(<FadeLeds as LedFunction>::new(&keyboard_controller));

    loop {
        tokio::time::sleep(Duration::from_millis(sleep_time)).await;

        let events = rx.try_iter().collect::<Vec<_>>();

        func.update(&config, &events, &ledmap, &keyboard_controller)?;
    }
}
