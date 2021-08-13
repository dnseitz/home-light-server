
use rocket::State;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use btleplug::platform::Peripheral;

use tokio::sync::mpsc::UnboundedSender;

use rocket::tokio::time::{sleep, Duration};

use crate::decoder::HomeLightMessageType;
use crate::light::LightInfo;
use crate::peripheral;

const LIGHT_INFO_TTL: u128 = 300_000;

pub(crate) struct PeripheralState {
    peripherals: Vec<(RocketRunState, RocketCommandChannel)>
    //peripherals: Vec<Arc<Mutex<(RunState, Sender<peripheral::Command>)>>>
}

impl PeripheralState {
    pub(crate) fn new(peripherals: Vec<(RocketRunState, RocketCommandChannel)>) -> Self {
        PeripheralState {
            peripherals
        }
    }
}

pub(crate) struct RunState {
    light_info: Option<(LightInfo, u128)>,
}

pub(crate) type RocketRunState = Arc<Mutex<RunState>>;
pub(crate) type RocketCommandChannel = Arc<Mutex<UnboundedSender<peripheral::Command>>>;

pub static LIGHT_STATE_REQUEST_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[get("/<index>/light_state")]
pub(crate) async fn light_state(index: usize, state: &State<PeripheralState>) -> String {
    println!("Getting light state for index: {}", index);
    let light_info = _get_latest_device_info(index, state, true).await;

    format!("{:?}", light_info)
}

#[get("/<index>/power_state")]
pub(crate) async fn get_power_state(index: usize, state: &State<PeripheralState>) -> String {
    let light_info = get_latest_device_info(index, state).await;

    format!("{}", if light_info.is_on { 1 } else { 0 })
}

#[put("/<index>/power_state", data = "<value>")]
pub(crate) async fn set_power_state(index: usize, value: String, state: &State<PeripheralState>) -> String {
    let new_value = match value.as_ref() {
        "ON" => { Some(1.0) }
        "OFF" => { Some(0.0) }
        _ => { None }
    };

    if let Some(new_value) = new_value {
        let command_channel = state.peripherals[index].1.lock().unwrap();
        let _ = command_channel.send(peripheral::Command::SetBrightness(new_value));

        if let Some((light_info, _)) = &mut state.peripherals[index].0.lock().unwrap().light_info {
            light_info.is_on = new_value > 0.0;
        }

        format!("Power state set")
    } else {
        format!("Unexpected Input, requires \"ON\" or \"OFF\"")
    }
}

#[get("/<index>/brightness")]
pub(crate) async fn get_brightness(index: usize, state: &State<PeripheralState>) -> String {
    let light_info = get_latest_device_info(index, state).await;

    let normalized_brightness = light_info.color.v;

    let brightness = (normalized_brightness * 100.0).round().clamp(0.0, 100.0) as u8;

    format!("{}", brightness)
}

#[put("/<index>/brightness", data = "<value>")]
pub(crate) async fn set_brightness(index: usize, value: String, state: &State<PeripheralState>) -> String {
    // TODO: Add Error type for failure to parse

    match value.parse::<u8>() {
        Err(error) => { format!("Parsing Error: {}", error) }
        Ok(new_value) => {
            let light_info = get_latest_device_info(index, state).await;

            let mut new_color = light_info.color.clone();
            new_color.v = (new_value as f64 / 100.0).clamp(0.0, 1.0);

            let command_channel = state.peripherals[index].1.lock().unwrap();
            let _ = command_channel.send(peripheral::Command::SetLEDColor(new_color.clone()));
            if let Some((light_info, _)) = &mut state.peripherals[index].0.lock().unwrap().light_info {
                light_info.color.v = new_color.v;
            }

            format!("Brightness Set")
        }
    }
}

#[get("/<index>/hue")]
pub(crate) async fn get_hue(index: usize, state: &State<PeripheralState>) -> String {
    let light_info = get_latest_device_info(index, state).await;

    let hue = light_info.color.h.round().clamp(0.0, 360.0) as u16;

    format!("{}", hue)
}

#[put("/<index>/hue", data = "<value>")]
pub(crate) async fn set_hue(index: usize, value: String, state: &State<PeripheralState>) -> String {
    // TODO: Add Error type for failure to parse
    
    match value.parse::<f64>() {
        Err(error) => { format!("Parsing Error: {}", error) }
        Ok(new_value) => {
            let light_info = get_latest_device_info(index, state).await;

            let mut new_color = light_info.color.clone();
            new_color.h = new_value.clamp(0.0, 360.0);

            let command_channel = state.peripherals[index].1.lock().unwrap();
            let _ = command_channel.send(peripheral::Command::SetLEDColor(new_color.clone()));
            if let Some((light_info, _)) = &mut state.peripherals[index].0.lock().unwrap().light_info {
                light_info.color.h = new_color.h;
            }

            format!("Hue Set")
        }
    }
}

#[get("/<index>/saturation")]
pub(crate) async fn get_saturation(index: usize, state: &State<PeripheralState>) -> String {
    let light_info = get_latest_device_info(index, state).await;

    let normalized_saturation = light_info.color.s;

    let saturation = (normalized_saturation * 100.0).round().clamp(0.0, 100.0) as u8;

    format!("{}", saturation)
}

#[put("/<index>/saturation", data = "<value>")]
pub(crate) async fn set_saturation(index: usize, value: String, state: &State<PeripheralState>) -> String {
    // TODO: Add Error type for faliure to parse
    
    match value.parse::<u8>() {
        Err(error) => { format!("Parsing Error: {}", error) },
        Ok(new_value) => {
            let light_info = get_latest_device_info(index, state).await;

            let mut new_color = light_info.color.clone();
            new_color.s = (new_value as f64 / 100.0).clamp(0.0, 1.0);

            let command_channel = &state.peripherals[index].1.lock().unwrap();
            let _ = command_channel.send(peripheral::Command::SetLEDColor(new_color.clone()));
            if let Some((light_info, _)) = &mut state.peripherals[index].0.lock().unwrap().light_info {
                light_info.color.s = new_color.s;
            }

            format!("Saturation Set")
        }
    }
}

async fn get_latest_device_info(index: usize, state: &State<PeripheralState>) -> LightInfo {
    _get_latest_device_info(index, state, false).await
}

async fn _get_latest_device_info(index: usize, state: &State<PeripheralState>, force_load: bool) -> LightInfo {
    if force_load == false {
        if let Some((light_info, timestamp)) = &state.peripherals[index].0.lock().unwrap().light_info {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();
            if current_time - *timestamp < LIGHT_INFO_TTL {
                return light_info.clone();
            }
        }
    }

    //let mut request_in_flight = false;
    loop {
        if LIGHT_STATE_REQUEST_IN_FLIGHT.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
        //if request_in_flight == false {
            println!("Request out of date and no request in flight, sending");
            let command_channel = &state.peripherals[index].1.lock().unwrap();
            let _ = command_channel.send(peripheral::Command::GetDeviceInfo);
            //request_in_flight = true;
        }

        // TODO: Should add some timeout
        sleep(Duration::from_millis(50)).await;
        {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();
            if let Some((light_info, timestamp)) = &state.peripherals[index].0.lock().unwrap().light_info {
                if current_time - *timestamp < LIGHT_INFO_TTL {
                    return light_info.clone();
                }
            }
        }
    }
}

pub(crate) async fn start(peripheral: &Peripheral) -> btleplug::Result<(RocketRunState, RocketCommandChannel)> {
    let (mut home_light_peripheral, command_tx) = peripheral::HomeLightPeripheral::new(peripheral.clone());

    let mut data_rx = home_light_peripheral.start_listening().await?;

    let run_state = Arc::new(Mutex::new(RunState::new()));
    
    println!("Setting up decoder thread");
    let data_run_state = run_state.clone();
    rocket::tokio::spawn(async move {
        loop {
            while let Some(message) = data_rx.recv().await {
                println!("Message Received: ({:?}) - {:?}", message.message_type, message.data);
                match message.message_type {
                    HomeLightMessageType::DeviceInfo => {
                        if let Ok(info) = LightInfo::from_raw_data(&message.data) {
                            println!("{:?}", info);
                            let mut state = data_run_state.lock().unwrap();
                            let current_time = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .expect("Time went backwards")
                                .as_millis();
                            state.light_info = Some((info, current_time));
                            LIGHT_STATE_REQUEST_IN_FLIGHT.store(false, Ordering::Release);
                        }
                    }
                    _ => { println!("Unhandled!") }
                }
            }
        }
    });

    Ok((run_state, Arc::new(Mutex::new(command_tx))))
}

impl RunState {
    fn new() -> Self {
        RunState {
            light_info: None
        }
    }
}
