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
use tokio::time;
use uuid::Uuid;

/// Only devices whose name contains this string will be tried.
const PERIPHERAL_NAME_MATCH_FILTER_1: &str = "TEST_DEVICE";
const PERIPHERAL_NAME_MATCH_FILTER_2: &str = "Bluno";
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

        let mut run_states: [Option<_>; 2] = [None, None];
        //let mut run_states = Vec::new();

        for peripheral in peripherals.into_iter() {
            let properties = peripheral.properties().await.unwrap();
            let local_name = properties
                .unwrap()
                .local_name
                .unwrap_or(String::from("(peripheral name unknown)"));

            println!("Addr: {}", peripheral.address().to_string());
            if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_1) {
                println!("Found: {:?}", &local_name);
                run_states[0] = Some(runner::start(&peripheral).await.unwrap());
            }
//            if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER_2) {
//                println!("Found: {:?}", &local_name);
//                run_states[1] = Some(runner::start(&peripheral).await.unwrap());
//            }
        }

        println!("Finished checking peripherals");

        let peripheral_state = runner::PeripheralState::new(run_states.iter_mut().filter_map(|x| x.take()).collect());

        println!("Launching Rocket!");

        let figment = rocket::Config::figment()
            .merge(("port", 8000));
        rocket::custom(figment)
            .manage(peripheral_state)
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
    }

    Ok(())
}

