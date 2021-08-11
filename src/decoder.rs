
use num::FromPrimitive;

use std::sync::mpsc::Sender;

const DATA_BEGIN_BYTE: u8 = 0xFE;
const DATA_END_BYTE: u8 = 0xFF;

#[derive(FromPrimitive, Clone, Copy, Debug)]
pub(crate) enum HomeLightMessageType {
    DeviceInfo = 0x01,
    DeviceColor = 0x02,
}

pub(crate) struct HomeLightMessage {
    pub message_type: HomeLightMessageType,
    pub data: Vec<u8>,
}

pub(crate) struct HomeLightDecoder {
    is_in_readable_command: bool,
    current_message_type: Option<HomeLightMessageType>,
    data_remaining: Option<u8>,
    current_data: Vec<u8>,

    tx: Sender<HomeLightMessage>
}

impl HomeLightDecoder {
    pub fn new(tx: Sender<HomeLightMessage>) -> Self {
        HomeLightDecoder {
            is_in_readable_command: false,
            current_message_type: None,
            data_remaining: None,
            current_data: Vec::new(),
            tx
        }
    }

    pub fn consume_data_packet(&mut self, data_packet: &[u8]) {
        for current_byte in data_packet {
            if self.is_in_readable_command == false {
                if *current_byte != DATA_BEGIN_BYTE {
                    continue;
                } else {
                    self.is_in_readable_command = true;
                    continue;
                }
            } else {
                // In a readable command
                //
                match self.current_message_type {
                    None => {
                        // Need to grab the message type
                        //
                        if let Some(message_type) = HomeLightMessageType::from_u8(*current_byte) {
                            self.current_message_type = Some(message_type);
                        } else {
                            self.reset_message_state();
                        }
                        continue;
                    }
                    Some(message_type) => {
                        match self.data_remaining {
                            None => {
                                // Need to grab the size of the data packet
                                //
                                if self.data_length_is_valid_for_type(*current_byte, message_type) {
                                    self.data_remaining = Some(*current_byte);
                                } else {
                                    self.reset_message_state();
                                }
                                continue;
                            }
                            Some(data_remaining) => {
                                if data_remaining > 0 {
                                    self.current_data.push(*current_byte);
                                    self.data_remaining = Some(data_remaining - 1);
                                } else {
                                    // No data remaining, check for termination byte
                                    //
                                    self.reset_message_state();

                                    if *current_byte == DATA_END_BYTE {
                                        // We found the expected byte, we have a full packet of
                                        // data.
                                        //
                                        println!("Received data packet: ({:?}) - {:?}", message_type, self.current_data);
                                        self.tx.send(HomeLightMessage { message_type, data: self.current_data.clone() }).unwrap();
                                    }

                                    self.current_data = Vec::new();
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn reset_message_state(&mut self) {
        self.is_in_readable_command = false;
        self.current_message_type = None;
        self.data_remaining = None;
    }

    fn data_length_is_valid_for_type(&self, length: u8, message_type: HomeLightMessageType) -> bool {
        match message_type {
            HomeLightMessageType::DeviceColor => { length == 3 }
            _ => { true }
        }
    }
}
