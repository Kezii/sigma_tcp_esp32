use anyhow::Result;
use async_trait::async_trait;

pub mod debug;

#[async_trait]
pub trait Backend: Send + Sync {
    async fn read(&mut self, addr: u16, len: u32) -> Result<Vec<u8>>;
    async fn write(&mut self, addr: u16, data: &[u8]) -> Result<()>;
}
