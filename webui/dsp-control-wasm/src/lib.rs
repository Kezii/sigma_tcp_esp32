use log::{error, info};
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlElement, HtmlInputElement, Window};

use crate::reg_io::{read_registers, write_registers};

mod reg_io;

#[wasm_bindgen(start)]
pub fn start() {
    wasm_logger::init(wasm_logger::Config::default());
    info!("DSP Control WASM module initialized");
}

// https://ez.analog.com/dsp/sigmadsp/w/documents/5169/what-are-the-number-formats-for-sigmadsp
// pag 81 of the datasheet
#[derive(Clone, Debug)]
pub enum DataType {
    //Int5_23, // 5.23 integer fixed point decimal format, this is used for audio samples, 4 bytes
    Int8_24,
    Int28_0, // 28.0 bit integer for dsp, 4 bytes
    Int32_0,
    //Int5_19, // 5.19 hardware readback format, 3 bytes
    //Double,
    //Float,
}

impl DataType {
    #[deprecated]
    pub fn get_size(&self) -> u16 {
        match self {
            //DataType::Int5_23 => 4,
            DataType::Int8_24 => 4,
            DataType::Int28_0 => 4,
            DataType::Int32_0 => 4,
            //DataType::Int5_19 => 3,
            //DataType::Double => 8,
            //DataType::Float => 4,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            DataType::Int8_24 => "Int8.24".to_string(),
            DataType::Int28_0 => "Int28.0".to_string(),
            DataType::Int32_0 => "Int32.0".to_string(),
        }
    }

    pub fn value_to_bytes(&self, value: f64) -> Vec<u8> {
        match self {
            DataType::Int8_24 => {
                // For 8.24 format, multiply by 2^24 to get the fixed point representation
                let scaled_value = value * 16777216.0; // 2^24
                let int_value = scaled_value as i32;

                int_value.to_be_bytes().to_vec()
            }
            DataType::Int32_0 => {
                let int_value = value as i32;

                int_value.to_be_bytes().to_vec()
            }
            DataType::Int28_0 => {
                let int_value = value as i32;

                int_value.to_be_bytes().to_vec()
            }
        }
    }

    pub fn bytes_to_value(&self, bytes: &[u8]) -> f64 {
        match self {
            DataType::Int8_24 => {
                let int_value = i32::from_be_bytes(bytes.try_into().unwrap());
                int_value as f64 / 16777216.0
            }
            DataType::Int32_0 => {
                let int_value = i32::from_be_bytes(bytes.try_into().unwrap());
                int_value as f64
            }
            DataType::Int28_0 => {
                let int_value = i32::from_be_bytes(bytes.try_into().unwrap());
                int_value as f64
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum MeasurementUnit {
    Decibel,
    None,
}

impl MeasurementUnit {
    pub fn to_string(&self) -> String {
        match self {
            MeasurementUnit::Decibel => "dB".to_string(),
            MeasurementUnit::None => "".to_string(),
        }
    }
}

/// Rappresenta un registro DSP
#[derive(Clone)]
pub struct DspRegister {
    pub name: String,
    pub address: u16,
    pub data_type: DataType,
    pub min: i32,
    pub max: i32,
    pub read_only: bool,
    pub unit: MeasurementUnit,
}

impl DspRegister {
    pub fn unit_to_raw_value(&self, value: f64) -> f64 {
        match self.unit {
            // this is not consistent? from the gain slider vs the level meter
            MeasurementUnit::Decibel => {
                // Convert decibels to linear scale
                let linear_value = 10.0f64.powf(value / 20.0);
                linear_value
            }
            MeasurementUnit::None => value,
        }
    }

    pub fn raw_value_to_unit(&self, value: f64) -> f64 {
        match self.unit {
            MeasurementUnit::Decibel => {
                // Convert linear scale to decibels
                let decibel_value = 10.0 * value.log10();
                decibel_value
            }
            MeasurementUnit::None => value,
        }
    }
}

/// Configurazione dei registri DSP

fn get_dsp_registers() -> Vec<DspRegister> {
if false {
    vec![
        DspRegister {
            name: "Gain".to_string(),
            address: 0x007E,
            data_type: DataType::Int8_24,
            //min: 0,
            //max: 16777216,
            min: -80,
            max: 10,
            read_only: false,
            unit: MeasurementUnit::Decibel,
        },
        DspRegister {
            name: "Signal Level - Input".to_string(),
            address: 115,
            data_type: DataType::Int8_24,
            //min: 0,
            //max: 1 << 30,
            min: 0,
            max: 100,
            read_only: true,
            unit: MeasurementUnit::Decibel,
        },
        DspRegister {
            name: "Signal Level - Aux ADC".to_string(),
            address: 87,
            data_type: DataType::Int32_0,
            min: 0,
            max: 268435456,
            read_only: true,
            unit: MeasurementUnit::None,
        },
    ];
}


    vec![

        DspRegister {
            name: "Signal Level - Source".to_string(),
            address: 61,
            data_type: DataType::Int8_24,
            //min: 0,
            //max: 1 << 30,
            min: -96,
            max: 0,
            read_only: true,
            unit: MeasurementUnit::Decibel,
        },
        DspRegister {
            name: "Gain".to_string(),
            address: 0x0043,
            data_type: DataType::Int8_24,
            //min: 0,
            //max: 16777216,
            min: -80,
            max: 0,
            read_only: false,
            unit: MeasurementUnit::Decibel,
        },
        DspRegister {
            name: "Signal Level - Dest".to_string(),
            address: 79,
            data_type: DataType::Int8_24,
            //min: 0,
            //max: 1 << 30,
            min: -96,
            max: 0,
            read_only: true,
            unit: MeasurementUnit::Decibel,
        },
        DspRegister {
            name: "Signal Level - Aux ADC".to_string(),
            address: 41,
            data_type: DataType::Int32_0,
            min: 0,
            max: 268435456,
            read_only: true,
            unit: MeasurementUnit::None,
        },
        DspRegister {
            name: "Signal Level - MP7".to_string(),
            address: 65,
            data_type: DataType::Int32_0,
            min: 0,
            max: 268435456,
            read_only: true,
            unit: MeasurementUnit::None,
        },
    ]
}

fn get_dsp_register_by_address(address: u16) -> Option<DspRegister> {
    get_dsp_registers()
        .iter()
        .find(|r| r.address == address)
        .cloned()
}

/// Converte un valore in una rappresentazione di byte
#[deprecated]
pub fn value_to_bytes(value: i32, size: u16) -> Vec<u8> {
    let mut raw_value = value;

    // Gestisci i numeri negativi (converti in complemento a due)
    if raw_value < 0 {
        // Calcola il valore massimo per la larghezza di bit data (size * 8 bit)
        let max_val = 1 << (size * 8);
        raw_value = max_val + raw_value;
    }

    // Formatta come array di byte
    let mut bytes = Vec::with_capacity(size as usize);
    for i in (0..size).rev() {
        let byte = ((raw_value >> (i * 8)) & 0xFF) as u8;
        bytes.push(byte);
    }

    bytes
}

/// Converte un array di byte in un valore
#[deprecated]
pub fn bytes_to_value(bytes: &[u8], size: u16) -> i32 {
    let mut value: i32 = 0;

    for &byte in bytes {
        value = (value << 8) | (byte as i32 & 0xFF);
    }

    // Controlla se questo è un valore negativo (complemento a due)
    // Se il bit più significativo è impostato
    if size > 0 && !bytes.is_empty() {
        if (bytes[0] & 0x80) != 0 {
            // Calcola il valore massimo per questa larghezza di bit
            let max_val = 1 << (size * 8);
            // Converti dal complemento a due
            value = value - max_val;
        }
    }

    info!("Converted bytes to value: {}", value);
    value
}

/// Formatta un valore come stringa esadecimale
pub fn format_hex_bytes(bytes: &[u8]) -> String {
    let mut hex_string = String::new();

    // Ensure we display the correct number of bytes based on the size
    for (i, byte) in bytes.iter().enumerate() {
        hex_string.push_str(&format!("{:02X}", byte));
        if i < bytes.len() - 1 {
            hex_string.push(' ');
        }
    }

    hex_string
}

/// Format a float value with up to 3 decimal places, removing trailing zeros
fn format_value(value: f64) -> String {
    // First format with fixed precision
    let formatted = format!("{:.3}", value);
    
    // Remove trailing zeros after decimal point
    if formatted.contains('.') {
        let trimmed = formatted.trim_end_matches('0');
        // If we trimmed all the way to the decimal point, remove it too
        if trimmed.ends_with('.') {
            return trimmed.trim_end_matches('.').to_string();
        }
        return trimmed.to_string();
    }
    
    formatted
}

/// Aggiorna l'interfaccia utente per un registro
pub fn update_ui_for_register(register: &DspRegister, value: f64) -> Result<(), JsValue> {
    let document = get_document()?;

    // Aggiorna il valore decimale
    if let Some(value_box) = document.get_element_by_id(&format!("value-{}", register.address)) {
        value_box.set_text_content(Some(&format_value(value)));
    }

    // Aggiorna il valore esadecimale
    if let Some(hex_value) = document.get_element_by_id(&format!("hex-value-{}", register.address))
    {
        let raw_value = register.unit_to_raw_value(value);
        let hex_string = format_hex_bytes(&register.data_type.value_to_bytes(raw_value));
        hex_value.set_text_content(Some(&hex_string));
    }

    // Aggiorna il valore dello slider
    if let Some(slider_element) =
        document.get_element_by_id(&format!("slider-{}", register.address))
    {
        let slider = slider_element.dyn_into::<HtmlInputElement>()?;
        slider.set_value(&value.to_string());

        // For readonly sliders, set a CSS custom property to visualize the value
        if slider.disabled() {
            let min = register.min as f64;
            let max = register.max as f64;
            let val = value as f64;

            // Calculate percentage (clamped between 0-100%)
            let percentage = if max > min {
                ((val - min) / (max - min) * 100.0).max(0.0).min(100.0)
            } else {
                0.0
            };

            // Set the custom property for the gradient
            let html_slider = slider.dyn_into::<web_sys::HtmlElement>()?;
            html_slider
                .style()
                .set_property("--slider-value", &format!("{}%", percentage))?;
        }
    }

    Ok(())
}

/// Inizializza l'interfaccia utente
pub fn init_ui() -> Result<(), JsValue> {
    let document = get_document()?;

    // Ottieni il contenitore dei controlli
    let controls_container = document
        .get_element_by_id("controlsContainer")
        .ok_or_else(|| JsValue::from_str("Controls container not found"))?;

    // Ottieni i registri
    let registers = get_dsp_registers();

    // Crea elementi di controllo per ogni registro
    for register in registers.iter() {
        let control_item = create_control_item(&document, register)?;
        controls_container.append_child(&control_item)?;
    }

    // Configura il toggle di auto-refresh
    if let Some(auto_refresh_toggle) = document.get_element_by_id("autoRefreshToggle") {
        let toggle = auto_refresh_toggle.dyn_into::<HtmlInputElement>()?;

        let on_change = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Ok(document) = get_document() {
                if let Some(toggle_element) = document.get_element_by_id("autoRefreshToggle") {
                    if let Ok(toggle) = toggle_element.dyn_into::<HtmlInputElement>() {
                        if toggle.checked() {
                            let _ = start_auto_refresh();
                            let _ = set_status("Auto refresh enabled", false);
                        } else {
                            let _ = stop_auto_refresh();
                            let _ = set_status("Auto refresh disabled", false);
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        toggle.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
    }

    Ok(())
}

/// Crea un elemento di controllo per un registro
pub fn create_control_item(
    document: &Document,
    register: &DspRegister,
) -> Result<Element, JsValue> {
    // Crea l'elemento di controllo principale
    let control_item = document.create_element("div")?;
    control_item.set_class_name("dsp-control__control-item");
    control_item.set_id(&format!("control-{}", register.address));

    // Crea l'intestazione del controllo
    let header = document.create_element("div")?;
    header.set_class_name("dsp-control__control-header");

    let title_container = document.create_element("div")?;
    title_container.set_class_name("dsp-control__control-title-container");

    let title = document.create_element("div")?;
    title.set_class_name("dsp-control__control-title");
    title.set_text_content(Some(&register.name));

    // Add data type information
    let type_info = document.create_element("div")?;
    type_info.set_class_name("dsp-control__type-info");
    type_info.set_text_content(Some(&register.data_type.to_string()));

    title_container.append_child(&title)?;
    title_container.append_child(&type_info)?;

    let address = document.create_element("div")?;
    address.set_class_name("dsp-control__control-address");
    address.set_text_content(Some(&format!("0x{:02X}", register.address)));

    header.append_child(&title_container)?;
    header.append_child(&address)?;

    // Crea il corpo del controllo
    let body = document.create_element("div")?;
    body.set_class_name("dsp-control__control-body");

    // Contenitore dello slider
    let range_container = document.create_element("div")?;
    range_container.set_class_name("dsp-control__range-container");

    // Slider
    let slider = document.create_element("input")?;
    let slider_input = slider.dyn_into::<HtmlInputElement>()?;
    slider_input.set_type("range");
    slider_input.set_class_name("dsp-control__range");
    slider_input.set_min(&register.min.to_string());
    slider_input.set_max(&register.max.to_string());
    slider_input.set_value("0");
    slider_input.set_id(&format!("slider-{}", register.address));

    slider_input.set_attribute("data-address", &register.address.to_string())?;

    if register.read_only {
        slider_input.set_disabled(true);
        slider_input
            .set_class_name(format!("{} {}", slider_input.class_name(), "readonly").as_str());
    } else {
        // Aggiungi gestori di eventi
        let address = register.address;

        let register_clone = register.clone();

        let on_input = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            let value = input.value().parse::<f64>().unwrap_or(0.0);
            let _ = update_ui_for_register(&register_clone, value);
        }) as Box<dyn FnMut(_)>);


        let on_change = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let input = target.dyn_into::<HtmlInputElement>().unwrap();
            let value = input.value().parse::<f64>().unwrap_or(0.0);

            wasm_bindgen_futures::spawn_local(async move {
                let register = get_dsp_register_by_address(address).unwrap();
                let raw_value = register.unit_to_raw_value(value);
                let bytes = register.data_type.value_to_bytes(raw_value);

                match write_registers(address, &bytes).await {
                    Ok(success) => {
                        if success {
                            let bytes_str = format_hex_bytes(&bytes);
                            set_status(
                                &format!(
                                    "Wrote {} ({}) to register 0x{:02X}",
                                    value, bytes_str, address
                                ),
                                false,
                            )
                            .ok();
                        } else {
                            set_status(
                                &format!("Failed to write {} to register 0x{:02X}", value, address),
                                true,
                            )
                            .ok();
                        }
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Error: {}",
                            e.as_string().unwrap_or_else(|| "Unknown error".to_string())
                        );
                        set_status(&error_msg, true).ok();
                    }
                }
            });
        }) as Box<dyn FnMut(_)>);

        slider_input.set_oninput(Some(on_input.as_ref().unchecked_ref()));
        slider_input.set_onchange(Some(on_change.as_ref().unchecked_ref()));

        on_change.forget();
        on_input.forget();
    }

    // Min/Max labels
    let range_min_max = document.create_element("div")?;
    range_min_max.set_class_name("dsp-control__range-min-max");

    let min = document.create_element("span")?;
    let unit_text = register.unit.to_string();
    let unit_suffix = if !unit_text.is_empty() { format!(" {}", unit_text) } else { "".to_string() };
    min.set_text_content(Some(&format!("{}{}", register.min, unit_suffix)));

    let max = document.create_element("span")?;
    max.set_text_content(Some(&format!("{}{}", register.max, unit_suffix)));

    range_min_max.append_child(&min)?;
    range_min_max.append_child(&max)?;

    // Add data type information
    
    range_container.append_child(&slider_input)?;
    range_container.append_child(&range_min_max)?;

    // Display del valore
    let value_display = document.create_element("div")?;
    value_display.set_class_name("dsp-control__value-display");

    // Colonna valore decimale
    let dec_column = document.create_element("div")?;
    dec_column.set_class_name("dsp-control__value-column");

    let value_box = document.create_element("div")?;
    value_box.set_class_name("dsp-control__value-box");
    value_box.set_id(&format!("value-{}", register.address));
    value_box.set_text_content(Some("0"));

    let dec_label = document.create_element("div")?;
    dec_label.set_class_name("dsp-control__value-label");
    dec_label.set_text_content(Some("Decimal"));

    dec_column.append_child(&value_box)?;
    dec_column.append_child(&dec_label)?;

    // Colonna valore esadecimale
    let hex_column = document.create_element("div")?;
    hex_column.set_class_name("dsp-control__value-column");

    let hex_value = document.create_element("div")?;
    hex_value.set_class_name("dsp-control__hex-box");
    hex_value.set_id(&format!("hex-value-{}", register.address));

    let hex_label = document.create_element("div")?;
    hex_label.set_class_name("dsp-control__value-label");
    hex_label.set_text_content(Some("Raw Bytes (MSB→LSB)"));

    hex_column.append_child(&hex_value)?;
    hex_column.append_child(&hex_label)?;

    // Aggiunta delle colonne al display del valore
    value_display.append_child(&dec_column)?;
    value_display.append_child(&hex_column)?;

    body.append_child(&range_container)?;
    body.append_child(&value_display)?;

    // Assemblaggio dell'elemento di controllo
    control_item.append_child(&header)?;
    control_item.append_child(&body)?;

    Ok(control_item)
}

/// Imposta il messaggio di stato
pub fn set_status(message: &str, is_error: bool) -> Result<(), JsValue> {
    let document = get_document()?;

    if let Some(status_message) = document.get_element_by_id("statusMessage") {
        status_message.set_text_content(Some(message));

        let html_element = status_message.dyn_into::<HtmlElement>()?;
        if is_error {
            html_element.class_list().add_1("error")?;
        } else {
            html_element.class_list().remove_1("error")?;
        }
    }

    Ok(())
}

/// Mostra l'indicatore di caricamento
pub fn show_loading() -> Result<(), JsValue> {
    let document = get_document()?;

    if let Some(loading_indicator) = document.get_element_by_id("loadingIndicator") {
        let html_element = loading_indicator.dyn_into::<HtmlElement>()?;
        html_element.class_list().remove_1("hidden")?;
    }

    Ok(())
}

/// Nasconde l'indicatore di caricamento
pub fn hide_loading() -> Result<(), JsValue> {
    let document = get_document()?;

    if let Some(loading_indicator) = document.get_element_by_id("loadingIndicator") {
        let html_element = loading_indicator.dyn_into::<HtmlElement>()?;
        html_element.class_list().add_1("hidden")?;
    }

    Ok(())
}

/// Legge il valore di un registro e aggiorna l'UI
pub async fn read_register_and_update_ui(register: &DspRegister) -> Result<f64, JsValue> {
    info!(
        "Reading register at address 0x{:02X} with size {}",
        register.address,
        register.data_type.get_size()
    );
    //show_loading()?;

    match read_registers(register.address, register.data_type.get_size()).await {
        Ok(bytes) => {
            let raw_value = register.data_type.bytes_to_value(&bytes);
            let value = register.raw_value_to_unit(raw_value);

            info!(
                "got value: {:?}, trying to parse a {:?}",
                raw_value, register.unit
            );

            if value.is_nan() {
                error!(
                    "got nan from value: {:?}, trying to parse a {:?}",
                    raw_value, register.unit
                );
            }

            // Aggiorna l'UI con il corretto tipo di dati
            let display_size = register.data_type.get_size();
            info!(
                "Updating UI with value {} using display size {}",
                value, display_size
            );
            update_ui_for_register(register, value)?;

            //hide_loading()?;
            set_status(
                &format!("Read register 0x{:02X}: {}", register.address, value),
                false,
            )?;

            Ok(value)
        }
        Err(error) => {
            //hide_loading()?;
            let error_msg = error
                .as_string()
                .unwrap_or_else(|| "Unknown error".to_string());
            set_status(
                &format!(
                    "Error reading register 0x{:02X}: {}",
                    register.address, error_msg
                ),
                true,
            )?;
            Err(error)
        }
    }
}

/// Legge tutti i registri e aggiorna l'UI
pub async fn read_all_registers_and_update_ui(read_only: bool) -> Result<(), JsValue> {
    let registers = get_dsp_registers();

    if read_only {

        for register in registers.iter().filter(|r| r.read_only) {
            read_register_and_update_ui(register).await?;
        }
    } else {
        for register in registers.iter() {
            read_register_and_update_ui(register).await?;
        }
    }

    Ok(())
}

// Variabili globali per l'auto-refresh
static mut AUTO_REFRESH_HANDLE: Option<i32> = None;
static AUTO_REFRESH_RATE: i32 = 100; // ms

/// Avvia l'auto-refresh
pub fn start_auto_refresh() -> Result<(), JsValue> {
    stop_auto_refresh()?;

    let window = get_window()?;

    let callback = Closure::wrap(Box::new(move || {
        wasm_bindgen_futures::spawn_local(async {
            let _ = read_all_registers_and_update_ui(true).await;
        });
    }) as Box<dyn FnMut()>);

    let handle = window.set_interval_with_callback_and_timeout_and_arguments(
        callback.as_ref().unchecked_ref(),
        AUTO_REFRESH_RATE,
        &js_sys::Array::new(),
    )?;

    unsafe {
        AUTO_REFRESH_HANDLE = Some(handle);
    }

    callback.forget();

    Ok(())
}

/// Ferma l'auto-refresh
pub fn stop_auto_refresh() -> Result<(), JsValue> {
    let window = get_window()?;

    unsafe {
        if let Some(handle) = AUTO_REFRESH_HANDLE {
            window.clear_interval_with_handle(handle);
            AUTO_REFRESH_HANDLE = None;
        }
    }

    Ok(())
}

/// Inizializza l'applicazione
#[wasm_bindgen]
pub async fn initialize_app() -> Result<(), JsValue> {
    // Inizializza l'UI
    init_ui()?;

    // Leggi tutti i registri all'avvio
    read_all_registers_and_update_ui(false).await?;

    Ok(())
}

/// Helper function to get the window
fn get_window() -> Result<Window, JsValue> {
    web_sys::window().ok_or_else(|| JsValue::from_str("No window found"))
}

/// Helper function to get the document
fn get_document() -> Result<Document, JsValue> {
    let window = get_window()?;
    window
        .document()
        .ok_or_else(|| JsValue::from_str("No document found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int8_24_format() {
        let dtype = DataType::Int8_24;

        assert_eq!(dtype.value_to_bytes(-128.0), vec![0x80, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(-32.0), vec![0xE0, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(-8.0), vec![0xF8, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(-2.0), vec![0xFE, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(-1.0), vec![0xFF, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(-0.5), vec![0xFF, 0x80, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(0.0), vec![0x00, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(0.25), vec![0x00, 0x40, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(0.5), vec![0x00, 0x80, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(1.0), vec![0x01, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(2.0), vec![0x02, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_int32_0_format() {
        let dtype = DataType::Int32_0;

        assert_eq!(
            dtype.value_to_bytes(-2147483648.0),
            vec![0x80, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            dtype.value_to_bytes(-2147483647.0),
            vec![0x80, 0x00, 0x00, 0x01]
        );
        assert_eq!(
            dtype.value_to_bytes(-2147483646.0),
            vec![0x80, 0x00, 0x00, 0x02]
        );
        assert_eq!(
            dtype.value_to_bytes(-1073741824.0),
            vec![0xC0, 0x00, 0x00, 0x00]
        );
        assert_eq!(
            dtype.value_to_bytes(-536870912.0),
            vec![0xE0, 0x00, 0x00, 0x00]
        );
        assert_eq!(dtype.value_to_bytes(-4.0), vec![0xFF, 0xFF, 0xFF, 0xFC]);
        assert_eq!(dtype.value_to_bytes(-2.0), vec![0xFF, 0xFF, 0xFF, 0xFE]);
        assert_eq!(dtype.value_to_bytes(-1.0), vec![0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(dtype.value_to_bytes(0.0), vec![0x00, 0x00, 0x00, 0x00]);
        assert_eq!(dtype.value_to_bytes(1.0), vec![0x00, 0x00, 0x00, 0x01]);
        assert_eq!(dtype.value_to_bytes(2.0), vec![0x00, 0x00, 0x00, 0x02]);
        assert_eq!(dtype.value_to_bytes(3.0), vec![0x00, 0x00, 0x00, 0x03]);
        assert_eq!(dtype.value_to_bytes(4.0), vec![0x00, 0x00, 0x00, 0x04]);
        assert_eq!(
            dtype.value_to_bytes(2147483646.0),
            vec![0x7F, 0xFF, 0xFF, 0xFE]
        );
        assert_eq!(
            dtype.value_to_bytes(2147483647.0),
            vec![0x7F, 0xFF, 0xFF, 0xFF]
        );
    }
}
