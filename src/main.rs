use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    pin::Pin,
    str::FromStr,
    sync::mpsc::{self, Receiver, TryRecvError},
    task::{Context, Poll},
    time::Duration,
};

use clap::{Parser, Subcommand};
use openrgb2::{Color, Controller, DeviceType, OpenRgbClient, OpenRgbError, OpenRgbResult};
use smol::Timer;

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

#[derive(Parser, Debug)]
pub struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug, Default)]
enum Command {
    #[default]
    Run,
    Setup {
        // Which device to set up. Should be a device file path (aka /dev/hidrawX).
        device: String,
    },
}

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
        if let Ok(client) = OpenRgbClient::connect_to(("0.0.0.0", 6742), 5).await {
            return client;
        } else {
            let ms = (250 * i).min(10000);
            println!("Could not connect to OpenRGB SDK server. Retrying in {ms}ms.");
            Timer::after(Duration::from_millis(ms)).await;
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

#[tokio::main]
async fn main() -> OpenRgbResult<()> {
    let mut client = wait_for_server().await;
    client.set_name("openrgb-fade").await?;

    let config = Config::load_from_first().unwrap();

    let arguments = Arguments::parse();

    match arguments.command.unwrap_or_default() {
        Command::Setup {
            device: device_path,
        } => {
            let device = client
                .get_all_controllers()
                .await?
                .into_iter()
                .find(|c| c.location() == &device_path);

            if let Some(device) = device {
                let out_file = get_keymap_filepath(&device);
                if std::fs::exists(&out_file).unwrap_or(false) {
                    println!(
                        "This device already has a keymap file! If you wish to redo the setup, please delete the old file at {} and then rerun the setup.",
                        out_file.to_string_lossy()
                    );
                    std::process::exit(1);
                }

                let keymap = setup_device(&device).await?;
                println!("Finished setting up {} at {}.", device.name(), "");

                std::fs::write(out_file, keymap.as_file_string()).unwrap();
                std::process::exit(0);
            } else {
                println!("OpenRGB can't find a compatible device at {device_path}. Aborting.");
                std::process::exit(2);
            }
        }
        Command::Run => {}
    }

    let sleep_time = 1000 / config.fps() as u64;
    println!("Frame time: {sleep_time}ms");

    let try_setup_thread = |controller: Controller| -> Option<(String, Receiver<()>)> {
        let keymap_file = std::fs::read_to_string(get_keymap_filepath(&controller)).ok()?;
        let keymap = KeyMapping::parse_from_file(keymap_file)?;

        let mut hid = HidReader::<256>::new_from_path(controller.location())?;

        let (tx, hid_event_reader) = mpsc::channel();

        std::thread::spawn(move || {
            while let Ok(event) = hid.read_blocking() {
                if tx.send(event).is_err() {
                    return;
                }
            }
        });

        let (tx, thread_exited) = mpsc::channel();

        let location = controller.location().to_string();
        let config = config.clone();

        println!(
            "Spawning thread for {} (at {})",
            controller.name(),
            controller.location()
        );

        std::thread::spawn(move || {
            if smol::block_on(async {
                controller.init().await?;
                controller.turn_off_leds().await?;
                Ok::<(), OpenRgbError>(())
            })
            .is_err()
            {
                tx.send(()).unwrap();
                return;
            };

            let mut func = FadeLeds::new(&controller);
            'outer: loop {
                std::thread::sleep(Duration::from_millis(sleep_time));
                let mut events = Vec::new();

                loop {
                    match hid_event_reader.try_recv() {
                        Err(TryRecvError::Empty) => break,
                        Ok(event) => {
                            events.push(event);
                        }
                        Err(TryRecvError::Disconnected) => break 'outer,
                    }
                }

                if func.update(&config, &events, &keymap, &controller).is_err() {
                    break;
                }
            }

            tx.send(()).unwrap();
        });

        Some((location, thread_exited))
    };

    let mut active_devices: HashMap<String, Receiver<()>> = HashMap::new();

    loop {
        let to_remove = active_devices
            .iter()
            .filter_map(|(path, recv)| match recv.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    println!("Thread for {path} closed. Removing from active.");

                    Some(path.clone())
                }
                Err(TryRecvError::Empty) => None,
            })
            .collect::<Vec<_>>();

        for path in to_remove.into_iter() {
            active_devices.remove(&path);
        }

        let controllers = client
            .get_controllers_of_type(DeviceType::Keyboard)
            .await
            .unwrap()
            .into_iter()
            .filter(|c| !active_devices.contains_key(c.location()))
            .collect::<Vec<_>>();

        // FIXME(upstream): openrgb2 currently doesn't handle `DeviceListUpdated` packets correctly (in fact, it just panics!)
        // this means that we can't get updated device locations if a device is un- and then replugged while the server is running.
        for mut controller in controllers {
            controller.sync_controller_data().await.unwrap();
            if let Some((key, value)) = try_setup_thread(controller) {
                active_devices.insert(key, value);
            }
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}
