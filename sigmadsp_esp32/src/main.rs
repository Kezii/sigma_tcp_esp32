mod wifi_handler;
use anyhow::{bail, Context, Result};
use esp_idf_hal::delay::BLOCK;
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
use log::{debug, error, info};
use smallvec::SmallVec;
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    thread,
};
use wifi_handler::my_wifi;

use sigma_tcp_rs::{ProtocolCommand, ProtocolHandler, ProtocolResponse};

// Definizione dell'indirizzo I2C del DSP
const DSP_I2C_ADDR: u8 = 0x3b;

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

    let _wifi = match my_wifi("🦊", "Foxo//Lab", peripherals.modem, sysloop) {
        Ok(inner) => inner,
        Err(err) => {
            bail!("Could not connect to Wi-Fi network: {:?}", err)
        }
    };

    // Inizializza I2C master
    let mut i2c_master = i2c_master_init(
        peripherals.i2c0,
        peripherals.pins.gpio2.into(),
        peripherals.pins.gpio3.into(),
        100.kHz().into(),
    )?;

    log::info!("I2C initialized at 100kHz");
    log::info!("Hello, world!");

    // scan all I2C devices

    for i in 0..127 {
        let mut buf = [0u8; 1];
        match i2c_master.read(i, &mut buf, BLOCK) {
            Ok(_) => {
                log::info!("Found I2C device at address: {:#04x}", i);
            }
            Err(e) => {
                // log::error!("Error reading I2C device at address {:#04x}: {:?}", i, e);
            }
        }
    }

    // Passa l'I2C master al server TCP
    tcp_server(i2c_master)?;

    Ok(())
}

fn tcp_server(i2c: I2cDriver<'static>) -> Result<(), io::Error> {
    fn accept(i2c: I2cDriver<'static>) -> Result<(), io::Error> {
        let i2c = std::sync::Arc::new(std::sync::Mutex::new(i2c));
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

    fn handle(mut stream: TcpStream, i2c: std::sync::Arc<std::sync::Mutex<I2cDriver<'static>>>) {
        let mut buf = [0u8; 1024];
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

                        match response {
                            ProtocolResponse::Read { header, data } => {
                                info!("read at addr 0x{:04x} size {:?} resp {:02x?}", header.param_addr, header.data_len, data);
                                stream.write_all(&response_bytes).unwrap();
                                stream.flush().unwrap();
                            }
                            ProtocolResponse::Write { header } => {
                                info!("write at addr 0x{:04x} size {:?}", header.param_addr, header.data_len);
                            }
                            ProtocolResponse::Error(e) => {
                                error!("Protocol error: {e}");
                                error!("{bytes:?}");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Process command error: {e}");
                        error!("{bytes:?}");
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
    i2c: &std::sync::Arc<std::sync::Mutex<I2cDriver<'static>>>,
) -> Result<(ProtocolResponse, usize)> {
    let (command, bytes_read) =
        ProtocolHandler::parse_command(buf).context("Failed to parse command")?;

    //info!("Parsed command: {:?}", command);

    match command {
        ProtocolCommand::Read { header } => {
            let mut i2c = i2c.lock().unwrap();

            // Per leggere da un registro del DSP:
            // 1. Scrivi l'indirizzo del parametro
            // 2. Leggi i dati

            // Converti l'indirizzo del parametro in un buffer di 2 byte (formato big-endian)
            let param_addr_bytes = header.param_addr.to_be_bytes();

            // Scrivi l'indirizzo del parametro al DSP
            match i2c.write(DSP_I2C_ADDR, &param_addr_bytes, BLOCK) {
                Ok(_) => {
                    // Ora leggi i dati dal DSP
                    let mut data = vec![0u8; header.data_len as usize];
                    match i2c.read(DSP_I2C_ADDR, &mut data, BLOCK) {
                        Ok(_) => {
                            Ok((
                                ProtocolHandler::create_read_response(
                                    header.chip_addr,
                                    header.data_len,
                                    header.param_addr,
                                    data,
                                ),
                                bytes_read,
                            ))
                        }
                        Err(e) => {
                            error!("I2C read failed: {:?}", e);
                            Ok((
                                ProtocolHandler::create_error_response(format!(
                                    "I2C read error: {:?}",
                                    e
                                )),
                                bytes_read,
                            ))
                        }
                    }
                }
                Err(e) => {
                    error!("I2C write address failed: {:?}", e);
                    Ok((
                        ProtocolHandler::create_error_response(format!(
                            "I2C write address error: {:?}",
                            e
                        )),
                        bytes_read,
                    ))
                }
            }
        }
        ProtocolCommand::Write { header, data } => {
            let mut i2c = i2c.lock().unwrap();

            // Per scrivere a un registro del DSP:
            // Invia l'indirizzo del parametro seguito dai dati

            // Crea un buffer che contiene l'indirizzo del parametro + i dati da scrivere
            let mut write_buf = Vec::with_capacity(2 + data.len());
            write_buf.extend_from_slice(&header.param_addr.to_be_bytes());
            write_buf.extend_from_slice(&data);

            match i2c.write(DSP_I2C_ADDR, &write_buf, BLOCK) {
                Ok(_) => {
                    Ok((
                        ProtocolHandler::create_write_response(
                            header.chip_addr,
                            header.data_len,
                            header.param_addr,
                        ),
                        bytes_read,
                    ))
                }
                Err(e) => {
                    error!("I2C write failed: {:?}", e);
                    Ok((
                        ProtocolHandler::create_error_response(format!("I2C write error: {:?}", e)),
                        bytes_read,
                    ))
                }
            }
        }
        ProtocolCommand::Unknown(cmd) => {
            error!("Unknown command: 0x{:02x}", cmd);
            Ok((
                ProtocolHandler::create_error_response(format!("Unknown command: 0x{:02x}", cmd)),
                bytes_read,
            ))
        }
    }
}
