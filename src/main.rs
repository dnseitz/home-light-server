// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

#[macro_use] extern crate num_derive;
#[macro_use] extern crate rocket;

mod decoder;
mod light;
mod runner;
mod peripheral;

//use rocket::config::{Config, Environment};

use btleplug::api::{bleuuid::uuid_from_u16, Central, Manager as _, Peripheral};
use btleplug::platform::Manager;
use std::error::Error;
use std::time::Duration;
use std::sync::mpsc::Sender;
use tokio::time;
use uuid::Uuid;

use futures::StreamExt;

/// Only devices whose name contains this string will be tried.
const PERIPHERAL_NAME_MATCH_FILTER_1: &str = "Bluno";
const PERIPHERAL_NAME_MATCH_FILTER_2: &str = "TEST_DEVICE";
/// UUID of the characteristic for which we should subscribe to notifications.
const NOTIFY_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0xDFB1);

#[tokio::main]
async fn main() {
    if let Err(err) = start().await {
        println!("Error starting server: {}", err);
    }
}

async fn start() -> Result<(), Box<dyn Error>> {
    //pretty_env_logger::init();
    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    for adapter in adapter_list.iter() {
        println!("Adapter: {:?}", adapter);
        println!("Starting scan...");
        adapter
            .start_scan()
            .await
            .expect("Can't scan BLE adapter for connected devices...");
        time::sleep(Duration::from_secs(2)).await;
        adapter.stop_scan().await.expect("Can't stop scan...");
        let peripherals = adapter.peripherals().await?;

        /*
        let run_states: Vec<(runner::RunState, Sender<peripheral::Command>)> = futures::stream::iter(peripherals.iter())
            .filter_map(|p| async {
                let properties = p.properties().await.unwrap();
                let is_connected = p.is_connected().await.unwrap();
                let local_name = properties
                    .unwrap()
                    .local_name
                    .unwrap_or(String::from("(peripheral name unknown)"));
                println!("Peripheral {:?} is connected: {:?}", &local_name, is_connected);

                if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_1) || local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_2) {
                    Some(runner::start(&p.clone()).await.unwrap())
                } else {
                    None
                }
            })
            //.map(|p| async { runner::start(&p.clone()).await.unwrap() })
            .collect()
            .await;
        */

        let mut run_states = Vec::new();
        let mut available_peripherals = Vec::new();

        for peripheral in peripherals.into_iter() {
            println!("Checking next peripheral...");
            let properties = peripheral.properties().await.unwrap();
            println!("Properties found...");
            let is_connected = peripheral.is_connected().await.unwrap();
            println!("Connection state found...");
            let local_name = properties
                .unwrap()
                .local_name
                .unwrap_or(String::from("(peripheral name unknown)"));
            println!("Local Name found...");
            println!("Peripheral {:?} is connected: {:?}", &local_name, is_connected);

            if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_1) || local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_2) {
                available_peripherals.push(peripheral);
                //run_states.push(runner::start(&peripheral).await.unwrap());
                //println!("Set up peripheral: {:?}", &local_name);
            }
        }

        for peripheral in available_peripherals.into_iter() {
            run_states.push(runner::start(&peripheral).await.unwrap());
        }

        println!("Finished checking peripherals");

        let peripheral_state = runner::PeripheralState::new(run_states);

        println!("Launching Rocket!");

        let figment = rocket::Config::figment()
            .merge(("workers", 2))
            .merge(("port", 8001));
        rocket::custom(figment)
            .manage(peripheral_state)
          //.manage(run_state)
          //.manage(Arc::new(Mutex::new(command_tx_state)))
            .mount("/", routes![
                runner::light_state,
                runner::get_power_state,
                runner::set_power_state,
                runner::get_brightness,
                runner::set_brightness,
                runner::get_hue,
                runner::set_hue,
                runner::get_saturation,
                runner::set_saturation
            ])
            .launch().await.unwrap();
/*
        if peripherals.is_empty() {
            eprintln!("->>> BLE peripheral devices were not found, sorry. Exiting...");
        } else {
            // All peripheral devices in range.
            for peripheral in peripherals.iter() {
                let properties = peripheral.properties().await?;
                let is_connected = peripheral.is_connected().await?;
                let local_name = properties
                    .unwrap()
                    .local_name
                    .unwrap_or(String::from("(peripheral name unknown)"));
                println!(
                    "Peripheral {:?} is connected: {:?}",
                    &local_name, is_connected
                );
                // Check if it's the peripheral we want.
                if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_1) || local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_2) {
                    let index = handles.len();
                    let local_peripheral = peripheral.clone();
                    handles.push(tokio::spawn(async move {
                        runner::start(&local_peripheral, index).await.unwrap();
                    }));
                } else {
                    println!("Skipping unknown peripheral {:?}", peripheral);
                }
            }
        }
*/
    }
    Ok(())
}

