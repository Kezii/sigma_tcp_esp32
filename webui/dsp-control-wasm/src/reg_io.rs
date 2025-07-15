use js_sys::{Array, Function, Object, Promise, Reflect};
use log::{error, info};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Document, Element, HtmlElement, HtmlInputElement, Request, RequestInit, RequestMode, Response,
    Window,
};

use crate::get_window;

// Global configuration
/// The base URL for the API endpoints
/// If empty, requests will be made to the relative paths on the current domain
static mut API_BASE_URL: &str = "http://192.168.71.1";

fn get_api_base_url() -> &'static str {
    unsafe { API_BASE_URL }
}

/// Legge un registro DSP
pub async fn read_registers(address: u16, size: u16) -> Result<Vec<u8>, JsValue> {
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let url = format!(
        "{}/read?addr=0x{:04x}&len={}",
        get_api_base_url(),
        address,
        size
    );
    let request = Request::new_with_str_and_init(&url, &opts)?;

    let window = get_window()?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into().unwrap();

    let json = JsFuture::from(resp.json()?).await?;

    #[derive(Debug, Serialize, Deserialize)]
    struct ReadRegisterResponse {
        addr: String,
        len: u16,
        data: String,
    }

    // Convert JsValue to our Rust struct
    let response: ReadRegisterResponse = serde_wasm_bindgen::from_value(json)?;

    // Parse the data string into bytes
    let data_str = &response.data;
    info!("Received data: {}", data_str);

    // Rimuovi le parentesi quadre e dividi per virgole
    let brackets = ['[', ']'];
    let cleaned_str = data_str.replace(&brackets[..], "");
    let hex_values: Vec<&str> = cleaned_str.split(',').map(|s| s.trim()).collect();

    // Converti le stringhe esadecimali in array di byte
    let mut byte_array = Vec::new();
    for hex_str in hex_values {
        if let Ok(byte) = u8::from_str_radix(&hex_str.replace("0x", ""), 16) {
            byte_array.push(byte);
        }
    }

    Ok(byte_array)
}

/// Scrive dei bytes in un registro DSP
pub async fn write_registers(address: u16, bytes: &[u8]) -> Result<bool, JsValue> {
    // Converti il valore in stringa esadecimale per l'API
    let mut hex_value = String::new();
    for byte in bytes {
        hex_value.push_str(&format!("{:02x}", byte));
    }

    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let url = format!(
        "{}/write?addr=0x{:04x}&data={}",
        get_api_base_url(),
        address,
        hex_value
    );
    let request = Request::new_with_str_and_init(&url, &opts)?;

    let window = get_window()?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into().unwrap();

    let json = JsFuture::from(resp.json()?).await?;

    #[derive(Debug, Serialize, Deserialize)]
    struct WriteRegisterResponse {
        status: String,
        addr: String,
        data_written: String,
        length: u16,
    }
    // Convert JsValue to our Rust struct
    let response: WriteRegisterResponse = serde_wasm_bindgen::from_value(json)?;

    // Check if the write was successful
    let success = response.status == "ok";
    Ok(success)
}
