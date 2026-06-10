use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

#[repr(C)]
pub struct DekWfpRule {
    pub direction: u32,
    pub action: u32,
    pub protocol: u8,
    pub remote_port: u16,
    pub remote_ipv4_be: u32,
    pub weight: u8,
}

#[link(name = "dek_wfp_native")]
extern "system" {
    fn dek_wfp_init_provider() -> u32;
    fn dek_wfp_add_rule(rule: *const DekWfpRule) -> u32;
    fn dek_wfp_clear_pollen_filters() -> u32;
}

pub fn init_provider() -> anyhow::Result<()> {
    let status = unsafe { dek_wfp_init_provider() };
    if status == 0 {
        Ok(())
    } else {
        anyhow::bail!("dek_wfp_init_provider failed: 0x{:08x}", status)
    }
}

pub fn add_tcp_block_443() -> anyhow::Result<()> {
    let rule = DekWfpRule {
        direction: 1, // outbound
        action: 2,    // block
        protocol: 6,  // TCP
        remote_port: 443,
        remote_ipv4_be: 0,
        weight: 240,
    };

    let status = unsafe { dek_wfp_add_rule(&rule) };
    if status == 0 {
        Ok(())
    } else {
        anyhow::bail!("dek_wfp_add_rule failed: 0x{:08x}", status)
    }
}

pub fn wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
