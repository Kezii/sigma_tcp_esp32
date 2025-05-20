use anyhow::{Context, Result};
use log::{debug, error, info};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

mod backend;
mod protocol;

use backend::debug::DebugBackend;
use backend::Backend;
use protocol::{ProtocolHandler, ProtocolCommand, ProtocolResponse};

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

        let response = process_command(&buf[..count], &backend).await?;
        match response {
            ProtocolResponse::Read { header, data } => {
                let mut response = header.to_bytes();
                response.extend(data);
                stream.write_all(&response).await?;
                count = 0;
            }
            ProtocolResponse::Write { header } => {
                stream.write_all(&header.to_bytes()).await?;
                count = 0;
            }
            ProtocolResponse::Error(e) => {
                error!("Protocol error: {}", e);
                count = 0;
            }
        }
    }
    
    Ok(())
}

async fn process_command(buf: &[u8], backend: &Arc<Mutex<dyn Backend>>) -> Result<ProtocolResponse> {

    let command = ProtocolHandler::parse_command(buf)
        .context("Failed to parse command")?;

    debug!("Parsed command: {:?}", command);


    match command {
        ProtocolCommand::Read { header } => {

            let mut backend = backend.lock().await;
            let data = backend.read(header.param_addr, header.data_len).await?;

            Ok(ProtocolHandler::create_read_response(
                header.chip_addr,
                header.data_len,
                header.param_addr,
                data,
            ))
        }
        ProtocolCommand::Write { header, data } => {
            let mut backend = backend.lock().await;
            backend.write(header.param_addr, &data).await?;

            Ok(ProtocolHandler::create_write_response(
                header.chip_addr,
                header.data_len,
                header.param_addr,
            ))
        }
        ProtocolCommand::Unknown(cmd) => {
            error!("Unknown command: 0x{:02x}", cmd);
            Ok(ProtocolHandler::create_error_response(format!("Unknown command: 0x{:02x}", cmd)))
        }
    }
}
