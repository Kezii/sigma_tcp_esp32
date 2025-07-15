mod wifi_handler;

/*
 * HTTP API Documentation
 * ======================
 *
 * This ESP32 provides an HTTP API to interact with the DSP via I2C.
 * All endpoints support both hexadecimal (with 0x prefix) and decimal values.
 *
 * Endpoints:
 *
 * 1. GET /
 *    Simple health check endpoint that returns "ok" if the server is running.
 *    Response: Plain text "ok"
 *
 * 2. GET /read
 *    Read data from a DSP register.
 *    Parameters:
 *    - addr: Register address (hex or decimal)
 *    - len: Number of bytes to read (hex or decimal)
 *    Example: /read?addr=0x3B&len=4
 *    Returns: JSON with address, length, and data in hex format
 *    Example response:
 *    {
 *      "addr": "0x003b",
 *      "len": 4,
 *      "data": "[0x01, 0x02, 0x03, 0x04]"
 *    }
 *
 *    Error response:
 *    {
 *      "error": "Failed to read from I2C: Device not found"
 *    }
 *
 * 3. GET /write
 *    Write data to a DSP register.
 *    Parameters:
 *    - addr: Register address (hex or decimal)
 *    - data: Data to write as hex string
 *    Example: /write?addr=0x3B&data=01020304
 *    Returns: JSON with status, address, written data, and length
 *    Example response:
 *    {
 *      "status": "ok",
 *      "addr": "0x003b",
 *      "data_written": "[0x01, 0x02, 0x03, 0x04]",
 *      "length": 4
 *    }
 *
 *    Error response:
 *    {
 *      "error": "Failed to write to I2C: Device not found"
 *    }
 */

use anyhow::{bail, Context, Result};
use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::io::EspIOError;
use esp_idf_hal::prelude::*;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::AnyIOPin,
        i2c::{I2c, I2cConfig, I2cDriver},
        peripheral::Peripheral,
        peripherals::Peripherals,
        units::Hertz,
    },
    http::{server::EspHttpServer, Method},
};
use log::{error, info};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};
use wifi_handler::my_wifi;

use sigma_tcp_rs::{ProtocolCommand, ProtocolHandler, ProtocolResponse};

// Definizione dell'indirizzo I2C del DSP
const DSP_I2C_ADDR: u8 = 0x3b;

// Parse HTTP query parameters into a HashMap with smart value parsing
fn parse_http_params(uri: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Extract query part (after ?)
    if let Some(query) = uri.split('?').nth(1) {
        // Split by & to get individual parameters
        for param in query.split('&') {
            // Split by = to get key-value pairs
            if let Some((key, value)) = param.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }
    }

    params
}

// Helper function to parse a string to u16, supporting both hex (0x prefix) and decimal
fn parse_number_to_u16(value: &str) -> Option<u16> {
    if value.starts_with("0x") || value.starts_with("0X") {
        // Parse as hex
        u16::from_str_radix(value.trim_start_matches("0x").trim_start_matches("0X"), 16).ok()
    } else {
        // Parse as decimal
        value.parse::<u16>().ok()
    }
}

// Parse hex data from string, supporting both hex (0x prefix) and space-separated bytes
fn parse_hex_data(hex_str: &str) -> Vec<u8> {
    let mut data = Vec::new();

    // Clean the input string
    let clean_value = hex_str.trim_start_matches("0x").trim_start_matches("0X");

    // Convert hex string to bytes
    for i in (0..clean_value.len()).step_by(2) {
        if i + 1 < clean_value.len() {
            if let Ok(byte) = u8::from_str_radix(&clean_value[i..i + 2], 16) {
                data.push(byte);
            }
        }
    }

    data
}

// I2C abstraction functions
fn read_i2c_register(
    i2c: &Arc<Mutex<I2cDriver<'static>>>,
    addr: u16,
    len: u16,
) -> Result<Vec<u8>, anyhow::Error> {
    let mut i2c = i2c.lock().unwrap();

    // Converti l'indirizzo del parametro in un buffer di 2 byte (formato big-endian)
    let param_addr_bytes = addr.to_be_bytes();

    // Scrivi l'indirizzo del parametro al DSP
    i2c.write(DSP_I2C_ADDR, &param_addr_bytes, BLOCK)?;

    // Ora leggi i dati dal DSP
    let mut data = vec![0u8; len as usize];
    i2c.read(DSP_I2C_ADDR, &mut data, BLOCK)?;

    Ok(data)
}

fn write_i2c_register(
    i2c: &Arc<Mutex<I2cDriver<'static>>>,
    addr: u16,
    data: &[u8],
) -> Result<(), anyhow::Error> {
    let mut i2c = i2c.lock().unwrap();

    // Crea un buffer che contiene l'indirizzo del parametro + i dati da scrivere
    let mut write_buf = Vec::with_capacity(2 + data.len());
    write_buf.extend_from_slice(&addr.to_be_bytes());
    write_buf.extend_from_slice(data);

    i2c.write(DSP_I2C_ADDR, &write_buf, BLOCK)?;

    Ok(())
}

fn i2c_master_init<'d>(
    i2c: impl Peripheral<P = impl I2c> + 'd,
    sda: AnyIOPin,
    scl: AnyIOPin,
    baudrate: Hertz,
) -> anyhow::Result<I2cDriver<'d>> {
    let config = I2cConfig::new().baudrate(baudrate);
    let driver = I2cDriver::new(i2c, sda, scl, &config)?;
    Ok(driver)
}

fn main() -> Result<()> {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    let sysloop = EspSystemEventLoop::take()?;

    let peripherals = Peripherals::take().unwrap();

    // Inizializza I2C master
    let mut i2c_master = i2c_master_init(
        peripherals.i2c0,
        peripherals.pins.gpio2.into(),
        peripherals.pins.gpio5.into(),
        400.kHz().into(),
    )?;

    log::info!("I2C initialized");

    // scan all I2C devices

    loop {
        let mut found = false;

        for i in 0..127 {
            let mut buf = [0u8; 1];
            match i2c_master.read(i, &mut buf, BLOCK) {
                Ok(_) => {
                    log::info!("Found I2C device at address: {i:#04x}");
                    found = true;
                }
                Err(_e) => {
                    // log::error!("Error reading I2C device at address {:#04x}: {:?}", i, e);
                }
            }
        }

        if found {
            break;
        } else {
            log::error!("No I2C devices found");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    let _wifi = match my_wifi(peripherals.modem, sysloop) {
        Ok(inner) => inner,
        Err(err) => {
            bail!("Could not connect to Wi-Fi network: {:?}", err)
        }
    };

    let i2c = Arc::new(Mutex::new(i2c_master));
    let i2c_http = i2c.clone();

    thread::spawn(move || {
        let mut server =
            EspHttpServer::new(&esp_idf_svc::http::server::Configuration::default()).unwrap();

        server
            .fn_handler("/", Method::Get, |request| {
                let mut response = request.into_response(
                    200,
                    Some("OK"),
                    &[
                        ("Access-Control-Allow-Origin", "*"),
                        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
                        ("Access-Control-Allow-Headers", "Content-Type"),
                    ],
                )?;

                esp_idf_hal::io::Write::write_all(&mut response, "ok".as_bytes())?;
                Ok::<(), EspIOError>(())
            })
            .unwrap();

        // Read endpoint
        let i2c_read = i2c_http.clone();
        server
            .fn_handler("/read", Method::Get, move |request| {
                // Get the URI as a string
                let uri = request.uri().to_string();

                // Parse parameters using our abstracted function
                let params = parse_http_params(&uri);

                // Extract and parse specific parameters
                let addr = params
                    .get("addr")
                    .and_then(|v| parse_number_to_u16(v))
                    .unwrap_or(0);

                let len = params
                    .get("len")
                    .and_then(|v| parse_number_to_u16(v))
                    .unwrap_or(0);

                info!("Reading from I2C address: 0x{:04x} length: {}", addr, len);

                let mut response = request.into_response(
                    200,
                    Some("OK"),
                    &[
                        ("Access-Control-Allow-Origin", "*"),
                        ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
                        ("Access-Control-Allow-Headers", "Content-Type"),
                    ],
                )?;

                // Use the abstracted I2C read function
                let result = match read_i2c_register(&i2c_read, addr, len) {
                    Ok(data) => {
                        format!(
                            "{{\"addr\": \"0x{:04x}\", \"len\": {}, \"data\": \"{:02X?}\" }}",
                            addr, len, data
                        )
                    }
                    Err(e) => {
                        format!("{{\"error\": \"Failed to read from I2C: {}\"}}", e)
                    }
                };

                esp_idf_hal::io::Write::write_all(&mut response, result.as_bytes())?;
                Ok::<(), EspIOError>(())
            })
            .unwrap();

        // Write endpoint
        let i2c_write = i2c_http.clone();
        server.fn_handler("/write", Method::Get, move |request| {
            // Get the URI as a string
            let uri = request.uri().to_string();

            // Parse parameters using our abstracted function
            let params = parse_http_params(&uri);

            // Extract and parse specific parameters
            let addr = params.get("addr")
                .and_then(|v| parse_number_to_u16(v))
                .unwrap_or(0);

            // Parse data from hex string
            let data = params.get("data")
                .map(|v| parse_hex_data(v))
                .unwrap_or_else(Vec::new);

            info!("Writing to I2C address: 0x{:04x} length: {}", addr, data.len());

            let mut response = request.into_response(200, Some("OK"), &[
                ("Access-Control-Allow-Origin", "*"),
                ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
                ("Access-Control-Allow-Headers", "Content-Type"),
            ])?;

            // Use the abstracted I2C write function
            let result = match write_i2c_register(&i2c_write, addr, &data) {
                Ok(_) => {
                    format!("{{\"status\": \"ok\", \"addr\": \"0x{:04x}\", \"data_written\": \"{:02X?}\", \"length\": {} }}", 
                        addr, data, data.len())
                },
                Err(e) => {
                    format!("{{\"error\": \"Failed to write to I2C: {}\"}}", e)
                }
            };

            esp_idf_hal::io::Write::write_all(&mut response, result.as_bytes())?;
            Ok::<(), EspIOError>(())
        }).unwrap();

        // Add OPTIONS handler to support preflight requests
        server
            .fn_handler("/*", Method::Options, |request| Ok::<(), EspIOError>(()))
            .unwrap();

        loop {
            std::thread::sleep(std::time::Duration::from_millis(5000));
        }
    });

    // Passa l'I2C master al server TCP
    tcp_server(i2c)?;

    Ok(())
}

fn tcp_server(i2c: Arc<Mutex<I2cDriver<'static>>>) -> Result<(), io::Error> {
    fn accept(i2c: Arc<Mutex<I2cDriver<'static>>>) -> Result<(), io::Error> {
        let listener = TcpListener::bind("0.0.0.0:8086")?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    info!("Accepted client");
                    let i2c_clone = i2c.clone();
                    thread::spawn(move || {
                        handle(stream, i2c_clone);
                    });
                }
                Err(e) => {
                    error!("Error: {e}");
                }
            }
        }

        unreachable!()
    }

    fn handle(mut stream: TcpStream, i2c: Arc<Mutex<I2cDriver<'static>>>) {
        // we size the buffer to the size of the ADAU1452 memory partition
        // this is very wasteful, a proper implementation would just stream the data
        let mut buf = Box::new([0u8; 20480 * 4 + 14]);

        let mut count = 0;

        loop {
            let n = stream.read(&mut buf[count..]).unwrap();
            if n == 0 {
                break;
            }
            count += n;

            //info!("rx {:x?}", &buf[..count]);

            let mut processed_bytes = 0;
            while processed_bytes < count {
                //info!("Processing bytes: {:?}", &buf[processed_bytes..count]);
                let bytes = &buf[processed_bytes..count];
                let result = process_command(bytes, &i2c);
                match result {
                    Ok((response, bytes_read)) => {
                        if bytes_read == 0 {
                            // Non ci sono abbastanza dati per un comando completo
                            break;
                        }

                        processed_bytes += bytes_read;
                        let response_bytes = response.to_bytes();

                        stream.write_all(&response_bytes).unwrap();
                        stream.flush().unwrap();
                    }
                    Err(e) => {
                        error!("Process command error: {e}");
                        //error!("{bytes:?}");
                        // In caso di errore, interrompiamo l'elaborazione
                        break;
                    }
                }
            }

            // Sposta i dati non processati all'inizio del buffer
            if processed_bytes > 0 {
                if processed_bytes < count {
                    buf.copy_within(processed_bytes..count, 0);
                }
                count -= processed_bytes;
            }
        }
    }

    accept(i2c)
}

fn process_command(
    buf: &[u8],
    i2c: &Arc<Mutex<I2cDriver<'static>>>,
) -> Result<(ProtocolResponse, usize)> {
    let (command, bytes_read) =
        ProtocolHandler::parse_command(buf).context("Failed to parse command")?;

    //info!("Parsed command: {:?}", command);

    match command {
        ProtocolCommand::Read { header } => {
            info!(
                "read at addr 0x{:04x} size {:?}",
                header.param_addr, header.data_len
            );

            // Use the abstracted I2C read function
            match read_i2c_register(i2c, header.param_addr, header.data_len as u16) {
                Ok(data) => Ok((
                    ProtocolHandler::create_read_response(
                        header.chip_addr,
                        header.data_len,
                        header.param_addr,
                        data,
                    ),
                    bytes_read,
                )),
                Err(e) => {
                    error!("I2C read failed: {e:?}");
                    Ok((
                        ProtocolHandler::create_error_response(format!("I2C read error: {e:?}")),
                        bytes_read,
                    ))
                }
            }
        }
        ProtocolCommand::Write { header, data } => {
            info!(
                "write at addr 0x{:04x} size {:?}",
                header.param_addr, header.data_len
            );

            // Use the abstracted I2C write function
            match write_i2c_register(i2c, header.param_addr, &data) {
                Ok(_) => Ok((ProtocolResponse::Write, bytes_read)),
                Err(e) => {
                    error!("I2C write failed: {e:?}");
                    Ok((
                        ProtocolHandler::create_error_response(format!("I2C write error: {e:?}")),
                        bytes_read,
                    ))
                }
            }
        }
        ProtocolCommand::Unknown(cmd) => {
            error!("Unknown command: 0x{cmd:02x}");
            Ok((
                ProtocolHandler::create_error_response(format!("Unknown command: 0x{cmd:02x}")),
                bytes_read,
            ))
        }
    }
}
