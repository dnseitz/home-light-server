
use std::sync::mpsc::Receiver;

use crate::decoder::{HomeLightMessage, HomeLightMessageType};
use crate::light::LightInfo;

pub(crate) async fn start(rx: Receiver<HomeLightMessage>) {
    for message in rx {
        println!("Message Received: ({:?}) - {:?}", message.message_type, message.data);

        match message.message_type {
            HomeLightMessageType::DeviceInfo => {
                if let Ok(info) = LightInfo::from_raw_data(&message.data) {
                    println!("{:?}", info);
                }
            }
            _ => { println!("Unhandled!") }
        }
    }
}
