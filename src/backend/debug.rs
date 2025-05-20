use anyhow::Result;
use log::info;
use async_trait::async_trait;

use super::Backend;

pub struct DebugBackend {
}

impl DebugBackend {
    pub fn new() -> Self {
        Self {
        }
    }
}

#[async_trait]
impl Backend for DebugBackend {
    async fn read(&mut self, addr: u16, len: u32) -> Result<Vec<u8>> {

        info!("read: 0x{:04x} {}", addr, len);
        
        let result = vec![12; len as usize];
        Ok(result)
    }

    async fn write(&mut self, addr: u16, data: &[u8]) -> Result<()> {

        info!("write: 0x{:04x} {}", addr, data.len());
        
        Ok(())
    }
} 