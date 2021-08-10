#[macro_use] extern crate log;
extern crate tokio;

//use tokio::prelude::*;
//use tokio::timer::Interval;
use tokio::time;
use core::pin::Pin;
use futures::select;

use std::time::{Duration, Instant};
use btleplug::api::{
    bleuuid::uuid_from_u16, 
    bleuuid::BleUuid, 
    BDAddr, 
    Central, 
    CentralEvent, 
    Manager as _, 
    Peripheral,
    WriteType
};
use btleplug::platform::{Adapter, Manager};
use futures::stream::StreamExt;
use std::error::Error;
use uuid::Uuid;

const BLUNO_SERVICE: Uuid = uuid_from_u16(0xDFB0);
const BLUNO_CHARACTERISTIC: Uuid = uuid_from_u16(0xDFB1);

async fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().await.unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await?;

    // get the first bluetooth adapter
    // connect to the adapter
    let central = get_central(&manager).await;

    //let mut events = central.events().await?;

    // Start scanning for devices
    central.start_scan().await?;
    time::sleep(Duration::from_secs(2)).await;

    let peripherals = central.peripherals().await?;
    for peripheral in peripherals.iter() {
        handle_device_discovery(peripheral.clone()).await.expect("Error connecting to device");
    }

    loop {}
/*
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(bd_addr) => {
                println!("Device Discovered: {:?}", bd_addr);
                let peripheral = central.peripheral(bd_addr).await?;
                //tokio::spawn(async {
                    handle_device_discovery(peripheral).await.expect("Error connecting to device");
                //});
            }
            CentralEvent::DeviceUpdated(bd_addr) => {
                println!("Device Updated: {:?}", bd_addr);
            }
            CentralEvent::ServiceDataAdvertisement {
                address,
                service_data,
            } => {
                println!(
                    "ServiceDataAdvertisement: {:?}, {:?}",
                    address, service_data
                );
            }
            CentralEvent::ServicesAdvertisement { address, services } => {
                let services: Vec<String> = services.into_iter().map(|s| s.to_short_string()).collect();
                println!("ServicesAdvertisement: {:?}, {:?}", address, services);
            }
            _ => {}
        }
    }
*/

    Ok(())
}

async fn handle_device_discovery(peripheral: impl Peripheral + Clone) -> Result<(), Box<dyn Error>> {
    //let peripheral = central.peripheral(bd_addr).await?;
    let bd_addr = peripheral.address();
    let services = peripheral
        .properties().await?.unwrap()
        .services;
    if services.contains(&BLUNO_SERVICE) {
        println!("Bluno Device Found! {:?}", bd_addr);

        while !peripheral.is_connected().await? {
            if let Err(error) = peripheral.connect().await {
                println!("Error connecting to Bluno: {:?}", error);
            }
        }
        if peripheral.is_connected().await? {
            println!("Connected to Bluno: {:?}", bd_addr);
        }
        
        let characteristics = peripheral.discover_characteristics().await?;
        //info!("Characteristics for device: {:?}, {:?}", bd_addr, characteristics);
        let bluno_characteristic = characteristics.iter()
            .filter(|c| c.uuid == BLUNO_CHARACTERISTIC)
            .nth(0);

        if let Some(characteristic) = bluno_characteristic {
            println!("Bluno Characteristic (0xDFB1) Discovered: {:?}", bd_addr);
            let mut notifications = peripheral.notifications().await?.fuse();
            peripheral.subscribe(characteristic).await?;

            loop {
                select! {
                    notification = notifications.next() => {
                        println!("Notification Received: {:?}, {:?}", bd_addr, notification);
                    },
                    default => {
                        let get_device_info_command = generate_get_device_info_command();
                        println!("Sending command: {:?}, {:?}", bd_addr, get_device_info_command);
                        peripheral.write(characteristic, &get_device_info_command, WriteType::WithResponse).await.expect("Write");
                        println!("Command sent");

                        time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
//            let mut notifications = peripheral.notifications().await?.fuse();
//
//            loop {
//                if let Ok(data) = peripheral.read(characteristic).await {
//                    println!("Data read: {:?}, {:?}", bd_addr, data);
//                }
//                select! {
//                    notification = notifications.next() => {
//                        println!("Notification Received: {:?}, {:?}", bd_addr, notification);
//                    }, 
//                    default => {
//                        let get_device_info_command = generate_get_device_info_command();
//                        println!("Sending command: {:?}, {:?}", bd_addr, get_device_info_command);
//                        peripheral.write(characteristic, &get_device_info_command, WriteType::WithoutResponse).await.expect("Write");
//                        println!("Command sent");
//                        time::sleep(Duration::from_millis(500)).await;
//                    },
//                }

                //if let Some(notification) = notifications.next().await {
                //if let Ok(Ready(Some(notification))) = notifications.poll_next().await {
                    //println!("Notification Received: {:?}, {:?}", bd_addr, notification);
                //}
//            }

        } else {
            println!("Couldn't find Bluno Characteristic (0xDFB1): {:?}", bd_addr);
        }
    }

    Ok(())
}

// Taken from iOS app
//private enum Command: UInt8 {
//    case setName = 0x00
//    case echo = 0x01
//    case setSolidLEDColor = 0x02
//    case setBrightness = 0x03
//
//    case getDeviceInfo = 0x04
//
//    case setAnimation = 0x05
//
//    case getColorInfo = 0x06
//
//    case setSchedule = 0x07
//
//    case clearSchedule = 0x08
//}
//
const COMMAND_BYTE: u8 = 0xFE;
const TERMINATOR_BYTE: u8 = 0xFF;

fn generate_get_device_info_command() -> Vec<u8> {
    vec![COMMAND_BYTE, 0x04, 0x00, TERMINATOR_BYTE]
}
