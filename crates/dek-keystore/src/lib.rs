use anyhow::Result;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

pub trait Keystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()>;
    fn load_key(&self, alias: &str) -> Result<Vec<u8>>;
    fn delete_key(&self, alias: &str) -> Result<()>;
}

pub fn get_keystore() -> Box<dyn Keystore + Send + Sync> {
    #[cfg(windows)]
    return Box::new(windows::DpapiKeystore::new());
    
    #[cfg(target_os = "macos")]
    return Box::new(macos::KeychainKeystore::new());
    
    #[cfg(target_os = "linux")]
    return Box::new(linux::FileKeystore::new());
    
    #[allow(unreachable_code)]
    Box::new(MockKeystore {})
}

// Fallback for unsupported OS
pub struct MockKeystore {}
impl Keystore for MockKeystore {
    fn store_key(&self, _alias: &str, _data: &[u8]) -> Result<()> {
        anyhow::bail!("Unsupported OS for Keystore")
    }
    fn load_key(&self, _alias: &str) -> Result<Vec<u8>> {
        anyhow::bail!("Unsupported OS for Keystore")
    }
    fn delete_key(&self, _alias: &str) -> Result<()> {
        Ok(())
    }
}
