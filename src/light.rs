
use num::FromPrimitive;
use std::error::Error;

#[derive(FromPrimitive, Clone, Copy)]
enum ColorState {
    Solid = 0x00,
    Animating = 0x01,
}

#[derive(Debug, Clone)]
pub struct HSVColor {
    pub h: f64,
    pub s: f64,
    pub v: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct LightInfo {
    pub name: String,
    pub is_on: bool,
    pub color: HSVColor,
}

impl LightInfo {
    pub fn from_raw_data(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut info = LightInfo {
            name: String::from(""),
            is_on: false,
            color: HSVColor { h: 0.0, s: 0.0, v: 0.0 }
        };

        info.name = String::from_utf8(data.iter().map(|byte| *byte).take_while(|&byte| byte != 0).collect())?;

        let mut remaining_data = data.iter().skip(info.name.len() + 1);
        
        if let Some(is_on) = remaining_data.nth(0) {
            info.is_on = *is_on != 0;
        }

        if let Some(color_state) = remaining_data.nth(0) {
            match ColorState::from_u8(*color_state) {
                Some(ColorState::Solid) => {
                    if let (Some(first), Some(second), Some(third)) = (remaining_data.nth(0), remaining_data.nth(0), remaining_data.nth(0)) {
                        let h = (f64::from(*first) / 255.0) * 360.0;
                        let s = f64::from(*second) / 255.0;
                        let v = f64::from(*third) / 255.0;

                        info.color = HSVColor { h, s, v };
                    }
                }
                _ => {}
            }
        }
        
        // Ignore the rest of the data, it contains animation/schedule info which we don't support

        Ok(info)
    }
}
