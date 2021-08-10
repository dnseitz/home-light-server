// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

#[macro_use] extern crate num_derive;

mod decoder;
mod light;
mod runner;

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
                    println!("Found matching peripheral {:?}...", &local_name);
                    while !peripheral.is_connected().await? {
                        if let Err(err) = peripheral.connect().await {
                            eprintln!("Error connecting to peripheral, retrying: {}", err);
                        }
                    }
                    let is_connected = peripheral.is_connected().await?;
                    println!(
                        "Now connected ({:?}) to peripheral {:?}.",
                        is_connected, &local_name
                    );
                    let chars = peripheral.discover_characteristics().await?;
                    if is_connected {
                        println!("Discover peripheral {:?} characteristics...", local_name);
                        for characteristic in chars.into_iter() {
                            println!("Checking characteristic {:?}", characteristic);
                            // Subscribe to notifications from the characteristic with the selected
                            // UUID.
                            if characteristic.uuid == NOTIFY_CHARACTERISTIC_UUID
                                && characteristic.properties.contains(CharPropFlags::NOTIFY)
                            {
                                println!("Subscribing to characteristic {:?}", characteristic.uuid);
                                peripheral.subscribe(&characteristic).await?;
                                let mut notification_stream = peripheral.notifications().await?;

                                peripheral.write(&characteristic, &[0xFE, 0x04, 0x00, 0xFF], WriteType::WithoutResponse).await?;

                                let (tx, rx) = mpsc::channel();
                                tokio::spawn(async move {
                                    runner::start(rx).await;
                                });
                                let mut decoder = HomeLightDecoder::new(tx);
                                // Process while the BLE connection is not broken or stopped.
                                while let Some(data) = notification_stream.next().await {
                                    if data.uuid == NOTIFY_CHARACTERISTIC_UUID {
                                        decoder.consume_data_packet(&data.value);
                                    }
                                    println!(
                                        "Received data from {:?} [{:?}]: {:?}",
                                        local_name, data.uuid, data.value
                                    );
                                }
                            }
                        }
                        println!("Disconnecting from peripheral {:?}...", local_name);
                        peripheral
                            .disconnect()
                            .await
                            .expect("Error disconnecting from BLE peripheral");
                    }
                } else {
                    println!("Skipping unknown peripheral {:?}", peripheral);
                }
            }
        }
    }
    Ok(())
}
