// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

#[macro_use] extern crate num_derive;

mod decoder;
mod light;
mod runner;
mod peripheral;

use btleplug::api::CharPropFlags;
use btleplug::api::{bleuuid::uuid_from_u16, WriteType, Central, Manager as _, Peripheral};
use btleplug::platform::Manager;
use futures::stream::StreamExt;
use std::error::Error;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

use std::sync::mpsc;

use decoder::HomeLightDecoder;


/// Only devices whose name contains this string will be tried.
//const PERIPHERAL_NAME_MATCH_FILTER: &str = "Bluno";
const PERIPHERAL_NAME_MATCH_FILTER: &str = "TEST_DEVICE";
/// UUID of the characteristic for which we should subscribe to notifications.
const NOTIFY_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0xDFB1);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }

    for adapter in adapter_list.iter() {
        println!("Starting scan...");
        adapter
            .start_scan()
            .await
            .expect("Can't scan BLE adapter for connected devices...");
        time::sleep(Duration::from_secs(2)).await;
        let peripherals = adapter.peripherals().await?;

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
                if local_name.contains(PERIPHERAL_NAME_MATCH_FILTER) {
                    runner::start(peripheral).await;
                } else {
                    println!("Skipping unknown peripheral {:?}", peripheral);
                }
            }
        }
    }
    Ok(())
}
