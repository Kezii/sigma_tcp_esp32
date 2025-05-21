use anyhow::{Context, Result};
use log::{debug, error, info};
use sigma_tcp_rs::{ProtocolCommand, ProtocolHandler, ProtocolResponse};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

mod backend;

use backend::debug::DebugBackend;
use backend::Backend;

const PORT: u16 = 8086;
const MAX_BUF_SIZE: usize = 2048;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();


    let backend = Arc::new(Mutex::new(DebugBackend::new()));
    
    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .await
        .context("Failed to bind to port")?;
    
    info!("Waiting for connections on port {}...", PORT);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("New connection from {}", addr);
                let backend = backend.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, backend).await {
                        error!("Error handling connection: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(mut stream: TcpStream, backend: Arc<Mutex<dyn Backend>>) -> Result<()> {
    let mut buf = [0u8; MAX_BUF_SIZE];
    let mut count = 0;
    
    loop {
        let n = stream.read(&mut buf[count..]).await?;
        if n == 0 {
            break;
        }
        count += n;

        debug!("rx {:x?}", &buf[..count]);

        let mut processed_bytes = 0;
        while processed_bytes < count {
            let (response, bytes_read) = process_command(&buf[processed_bytes..count], &backend).await?;
            if bytes_read == 0 {
                // Non ci sono abbastanza dati per un comando completo
                break;
            }
            
            processed_bytes += bytes_read;
            
            match response {
                ProtocolResponse::Read { header, data } => {
                    let mut response = header.to_bytes();
                    response.extend(data);
                    stream.write_all(&response).await?;
                }
                ProtocolResponse::Write { header } => {
                    stream.write_all(&header.to_bytes()).await?;
                }
                ProtocolResponse::Error(e) => {
                    error!("Protocol error: {}", e);
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
    
    Ok(())
}

async fn process_command(buf: &[u8], backend: &Arc<Mutex<dyn Backend>>) -> Result<(ProtocolResponse, usize)> {
    let parse_result = ProtocolHandler::parse_command(buf);
    
    match parse_result {
        Ok((command, bytes_read)) => {
            debug!("Parsed command: {:?}", command);

            let response = match command {
                ProtocolCommand::Read { header } => {
                    let mut backend = backend.lock().await;
                    let data = backend.read(header.param_addr, header.data_len).await?;

                    ProtocolHandler::create_read_response(
                        header.chip_addr,
                        header.data_len,
                        header.param_addr,
                        data,
                    )
                }
                ProtocolCommand::Write { header, data } => {
                    let mut backend = backend.lock().await;
                    backend.write(header.param_addr, &data).await?;

                    ProtocolHandler::create_write_response(
                        header.chip_addr,
                        header.data_len,
                        header.param_addr,
                    )
                }
                ProtocolCommand::Unknown(cmd) => {
                    error!("Unknown command: 0x{:02x}", cmd);
                    ProtocolHandler::create_error_response(format!("Unknown command: 0x{:02x}", cmd))
                }
            };
            
            Ok((response, bytes_read))
        },
        Err(e) if buf.len() < 3 => {
            // Non ci sono abbastanza dati per un comando completo
            Ok((ProtocolResponse::Error("Incomplete command".to_string()), 0))
        },
        Err(e) => {
            error!("Failed to parse command: {}", e);
            Ok((ProtocolHandler::create_error_response(format!("Parse error: {}", e)), 0))
        }
    }
}
