
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use btleplug::platform::Peripheral;

use tokio::time::{sleep, Duration};

use crate::decoder::{HomeLightMessage, HomeLightMessageType};
use crate::light::{LightInfo, HSVColor};
use crate::peripheral;

struct RunState {
    light_info: Option<LightInfo>,
}

pub(crate) async fn start(peripheral: &Peripheral) -> btleplug::Result<()> {
    let (mut home_light_peripheral, command_tx) = peripheral::HomeLightPeripheral::new(peripheral.clone());

    let data_rx = home_light_peripheral.start_listening().await?;

    let run_state = Arc::new(Mutex::new(RunState::new()));
    
    let data_run_state = run_state.clone();
    let data_handle = tokio::spawn(async move {
        for message in data_rx {
            println!("Message Received: ({:?}) - {:?}", message.message_type, message.data);

            match message.message_type {
                HomeLightMessageType::DeviceInfo => {
                    if let Ok(info) = LightInfo::from_raw_data(&message.data) {
                        println!("{:?}", info);
                        let mut state = run_state.lock().unwrap();
                        state.light_info = Some(info);
                    }
                }
                _ => { println!("Unhandled!") }
            }
        }
    });

    command_tx.send(peripheral::Command::GetDeviceInfo);
    command_tx.send(peripheral::Command::SetBrightness(1.0));
    sleep(Duration::from_millis(500)).await;
    command_tx.send(peripheral::Command::GetDeviceInfo);
    sleep(Duration::from_millis(500)).await;
    command_tx.send(peripheral::Command::SetLEDColor(HSVColor {
        h: 180.0,
        s: 0.90,
        v: 1.0
    }));
    sleep(Duration::from_millis(500)).await;
    command_tx.send(peripheral::Command::GetDeviceInfo);

    data_handle.await;

    Ok(())
}

impl RunState {
    fn new() -> Self {
        RunState {
            light_info: None
        }
    }
}
