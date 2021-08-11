
use rocket::State;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use btleplug::platform::Peripheral;

use tokio::time::{sleep, Duration};

use crate::decoder::HomeLightMessageType;
use crate::light::LightInfo;
use crate::peripheral;

struct RunState {
    light_info: Option<(LightInfo, u128)>,
}

type RocketRunState = Arc<Mutex<RunState>>;
type RocketCommandChannel = Arc<Mutex<Sender<peripheral::Command>>>;

//static LIGHT_STATE_REQUEST_ID: AtomicUsize = AtomicUsize::new(0);
pub static LIGHT_STATE_REQUEST_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[get("/light_state")]
async fn light_state(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    let light_info = _get_latest_device_info(state, command_channel, true).await;

    format!("{:?}", light_info)
}

#[get("/power_state")]
async fn get_power_state(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    let light_info = get_latest_device_info(state, command_channel).await;

    format!("{}", if light_info.is_on { 1 } else { 0 })
}

#[put("/power_state", data = "<value>")]
async fn set_power_state(value: String, command_channel: &State<RocketCommandChannel>) -> String {
    let new_value = match value.as_ref() {
        "ON" => { Some(1.0) }
        "OFF" => { Some(0.0) }
        _ => { None }
    };

    if let Some(new_value) = new_value {
        command_channel.lock().unwrap().send(peripheral::Command::SetBrightness(new_value)).unwrap();

        format!("Power state set")
    } else {
        format!("Unexpected Input, requires \"ON\" or \"OFF\"")
    }
}

#[get("/brightness")]
async fn get_brightness(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    let light_info = get_latest_device_info(state, command_channel).await;

    let normalized_brightness = light_info.color.v;

    let brightness = (normalized_brightness * 100.0).round().clamp(0.0, 100.0) as u8;

    format!("{}", brightness)
}

#[put("/brightness", data = "<value>")]
async fn set_brightness(value: String, state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    // TODO: Add Error type for failure to parse

    match value.parse::<u8>() {
        Err(error) => { format!("Parsing Error: {}", error) }
        Ok(new_value) => {
            let light_info = get_latest_device_info(state, command_channel).await;

            let mut new_color = light_info.color.clone();
            new_color.v = (new_value as f64 / 100.0).clamp(0.0, 1.0);

            command_channel.lock().unwrap().send(peripheral::Command::SetLEDColor(new_color.clone())).unwrap();
            if let Some((light_info, _)) = &mut state.lock().unwrap().light_info {
                light_info.color.v = new_color.v;
            }

            format!("Brightness Set")
        }
    }
}

#[get("/hue")]
async fn get_hue(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    let light_info = get_latest_device_info(state, command_channel).await;

    let hue = light_info.color.h.round().clamp(0.0, 360.0) as u16;

    format!("{}", hue)
}

#[put("/hue", data = "<value>")]
async fn set_hue(value: String, state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    // TODO: Add Error type for failure to parse
    
    match value.parse::<f64>() {
        Err(error) => { format!("Parsing Error: {}", error) }
        Ok(new_value) => {
            let light_info = get_latest_device_info(state, command_channel).await;

            let mut new_color = light_info.color.clone();
            new_color.h = new_value.clamp(0.0, 360.0);

            command_channel.lock().unwrap().send(peripheral::Command::SetLEDColor(new_color.clone())).unwrap();
            if let Some((light_info, _)) = &mut state.lock().unwrap().light_info {
                light_info.color.h = new_color.h;
            }

            format!("Hue Set")
        }
    }
}

#[get("/saturation")]
async fn get_saturation(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    let light_info = get_latest_device_info(state, command_channel).await;

    let normalized_saturation = light_info.color.s;

    let saturation = (normalized_saturation * 100.0).round().clamp(0.0, 100.0) as u8;

    format!("{}", saturation)
}

#[put("/saturation", data = "<value>")]
async fn set_saturation(value: String, state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> String {
    // TODO: Add Error type for faliure to parse
    
    match value.parse::<u8>() {
        Err(error) => { format!("Parsing Error: {}", error) },
        Ok(new_value) => {
            let light_info = get_latest_device_info(state, command_channel).await;

            let mut new_color = light_info.color.clone();
            new_color.s = (new_value as f64 / 100.0).clamp(0.0, 1.0);

            command_channel.lock().unwrap().send(peripheral::Command::SetLEDColor(new_color.clone())).unwrap();
            if let Some((light_info, _)) = &mut state.lock().unwrap().light_info {
                light_info.color.s = new_color.s;
            }

            format!("Saturation Set")
        }
    }
}

async fn get_latest_device_info(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>) -> LightInfo {
    _get_latest_device_info(state, command_channel, false).await
}

async fn _get_latest_device_info(state: &State<RocketRunState>, command_channel: &State<RocketCommandChannel>, force_load: bool) -> LightInfo {
    if force_load == false {
        if let Some((light_info, timestamp)) = &state.lock().unwrap().light_info {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();
            if current_time - *timestamp < 300_000 {
                return light_info.clone();
            }
        }
    }

    loop {
        if LIGHT_STATE_REQUEST_IN_FLIGHT.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            println!("Request out of date and no request in flight, sending");
            command_channel.lock().unwrap().send(peripheral::Command::GetDeviceInfo).unwrap();
        }

        // TODO: Should add some timeout
        sleep(Duration::from_millis(50)).await;
        {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();
            if let Some((light_info, timestamp)) = &state.lock().unwrap().light_info {
                if current_time - *timestamp < 300_000 {
                    return light_info.clone();
                }
            }
        }
    }
}

pub(crate) async fn start(peripheral: &Peripheral) -> btleplug::Result<()> {
    let (mut home_light_peripheral, command_tx) = peripheral::HomeLightPeripheral::new(peripheral.clone());

    let data_rx = home_light_peripheral.start_listening().await?;

    let run_state = Arc::new(Mutex::new(RunState::new()));
    
    println!("Setting up decoder thread");
    let data_run_state = run_state.clone();
    tokio::spawn(async move {
        for message in data_rx {
            println!("Message Received: ({:?}) - {:?}", message.message_type, message.data);

            match message.message_type {
                HomeLightMessageType::DeviceInfo => {
                    if let Ok(info) = LightInfo::from_raw_data(&message.data) {
                        println!("{:?}", info);
                        let mut state = data_run_state.lock().unwrap();
                        //let request_id = LIGHT_STATE_REQUEST_ID.load(Ordering::Relaxed);
                        //println!("Recieved Info Response with request ID: {}", request_id);
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
    });

    println!("Launching Rocket");
    rocket::build()
        .manage(run_state)
        .manage(Arc::new(Mutex::new(command_tx.clone())))
        .mount("/", routes![
            light_state, 
            get_power_state,
            set_power_state,
            get_brightness, 
            set_brightness,
            get_hue,
            set_hue,
            get_saturation,
            set_saturation
        ])
        .launch().await.unwrap();

    Ok(())
}

impl RunState {
    fn new() -> Self {
        RunState {
            light_info: None
        }
    }
}
