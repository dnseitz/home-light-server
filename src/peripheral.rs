use futures::StreamExt;
use std::sync::atomic::Ordering;
use btleplug::api::{Characteristic, CharPropFlags, Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use tokio::task::JoinHandle;
use tokio::sync::mpsc;

use crate::light::HSVColor;
use crate::NOTIFY_CHARACTERISTIC_UUID;
use crate::decoder;
use crate::runner;

const COMMAND_START_BYTE: u8 = 0xFE;
const COMMAND_END_BYTE: u8 = 0xFF;

pub(crate) enum Command {
    SetLEDColor(HSVColor),
    SetBrightness(f64),

    GetDeviceInfo,
}

impl Command {
    fn get_command_code(&self) -> u8 {
        match self {
            Command::SetLEDColor(_) => { 0x02 }
            Command::SetBrightness(_) => { 0x03 }
            Command::GetDeviceInfo => { 0x04 }
        }
    }

    fn get_command_data(&self) -> Vec<u8> {
        match self {
            Command::SetLEDColor(color) => {
                vec![
                    (color.h / 360.0 * 255.0).round() as u8, // H
                    (color.s * 255.0).round() as u8,         // S
                    (color.v * 255.0).round() as u8          // V
                ]
            }
            Command::SetBrightness(brightness) => {
                vec![(brightness.clamp(0.0, 1.0) * 255.0).round() as u8]
            }
            Command::GetDeviceInfo => { 
                vec![] 
            }
        }
    }

    fn get_raw_data(&self) -> Vec<u8> {
        let command_data = self.get_command_data();
        let mut raw_data = vec![COMMAND_START_BYTE, self.get_command_code(), command_data.len() as u8];
        raw_data.extend(command_data);
        raw_data.push(COMMAND_END_BYTE);

        raw_data
    }
}

pub(crate) struct HomeLightPeripheral {
    rx: Option<mpsc::UnboundedReceiver<Command>>,
    raw_peripheral: Peripheral,
    notification_handle: Option<JoinHandle<()>>,
    command_handle: Option<JoinHandle<()>>,
}

impl HomeLightPeripheral {
    pub fn new(raw_peripheral: Peripheral) -> (Self, mpsc::UnboundedSender<Command>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let notification_handle = None;
        let command_handle = None;

        (HomeLightPeripheral { rx: Some(rx), raw_peripheral, notification_handle, command_handle }, tx)
    }

    pub async fn start_listening(&mut self) -> btleplug::Result<mpsc::UnboundedReceiver<decoder::HomeLightMessage>> {
        while Self::connect_if_needed(&self.raw_peripheral).await == false {}
        let chars = self.raw_peripheral.discover_characteristics().await?;
        let is_connected = self.raw_peripheral.is_connected().await?;
        if is_connected {
            for characteristic in chars.into_iter() {
                // Subscribe to notifications from the characteristic with the selected
                // UUID.
                if characteristic.uuid == NOTIFY_CHARACTERISTIC_UUID
                    && characteristic.properties.contains(CharPropFlags::NOTIFY)
                {
                    println!("Subscribing to characteristic {:?}", characteristic.uuid);
                    self.raw_peripheral.subscribe(&characteristic).await?;
                    println!("Subscribed");

                    let (tx, rx) = mpsc::unbounded_channel();
                    let decoder = decoder::HomeLightDecoder::new(tx);
                    let notification_peripheral = self.raw_peripheral.clone();
                    self.notification_handle = Some(tokio::spawn(async move {
                        Self::process_notifications(&notification_peripheral, decoder).await.unwrap();
                        ()
                    }));
                    let command_peripheral = self.raw_peripheral.clone();
                    let command_characteristic = characteristic.clone();
                    let mut command_rx = self.rx.take().unwrap();
                    self.command_handle = Some(tokio::spawn(async move {
                        while let Some(command) = command_rx.recv().await {
                            if let Err(error) = HomeLightPeripheral::send_command(command_peripheral.clone(), &command_characteristic, command).await {
                                println!("Error sending command: {:?}", error);
                            }
                        }
                        ()
                    }));

                    return Ok(rx);
                }
            }
        }

        return Err(btleplug::Error::NotSupported(String::from("Couldn't start listening to peripheral notifications")));
    }

    async fn connect_if_needed(peripheral: &Peripheral) -> bool {
        while !peripheral.is_connected().await.unwrap_or(false) {
            runner::LIGHT_STATE_REQUEST_IN_FLIGHT.store(false, Ordering::Relaxed);
            let address = peripheral.address().to_string();
            if let Err(err) = async_process::Command::new("sudo").arg("hcitool").arg("lecc").arg(address).status().await {
                eprintln!("Error connecting to peripheral through hcitool: {}", err);
            }
            if let Err(err) = peripheral.connect().await {
                eprintln!("Error connecting to peripheral, retrying: {}", err);
            }
        }

        peripheral.is_connected().await.unwrap_or(false)
    }

    async fn process_notifications(peripheral: &Peripheral, mut decoder: decoder::HomeLightDecoder) -> btleplug::Result<()> 
    {
        let mut notification_stream = peripheral.notifications().await?;
        // Process while the BLE connection is not broken or stopped.
        while let Some(data) = notification_stream.next().await {
            if data.uuid == NOTIFY_CHARACTERISTIC_UUID {
                decoder.consume_data_packet(&data.value);
            }
        }

        Ok(())
    }
}

// MARK: - Command Handling

impl HomeLightPeripheral {
    async fn send_command(peripheral: Peripheral, characteristic: &Characteristic, command: Command) -> btleplug::Result<()> {
        let command_data = command.get_raw_data();
        if peripheral.is_connected().await? == false {
            while Self::connect_if_needed(&peripheral).await == false {}
        }
        println!("Peripheral Connection State: {:?}", peripheral.is_connected().await?);
        println!("Sending Command Data: {:?}", command_data);
        peripheral.write(characteristic, &command_data, WriteType::WithoutResponse).await
    }
}
