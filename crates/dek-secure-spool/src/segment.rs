use crc32c::crc32c;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};
use uuid::Uuid;

const MAGIC: &[u8; 4] = b"PDS1";

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub schema_version: String,
    pub event_id: Uuid,
    pub tenant_id: String,
    pub device_id: String,
    pub event_type: String,
    pub timestamp_unix_ms: i64,
    pub body: serde_json::Value,
}

pub struct SegmentWriter {
    file: File,
    tenant_id: String,
    device_id: String,
    segment_id: String,
    seq: u64,
}

impl SegmentWriter {
    pub fn create(
        path: &Path,
        tenant_id: impl Into<String>,
        device_id: impl Into<String>,
        segment_id: impl Into<String>,
    ) -> io::Result<Self> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)?;

        file.write_all(MAGIC)?;
        file.write_all(&1u16.to_le_bytes())?; // segment format version
        file.sync_data()?;

        Ok(Self {
            file,
            tenant_id: tenant_id.into(),
            device_id: device_id.into(),
            segment_id: segment_id.into(),
            seq: 0,
        })
    }

    pub fn append_event(
        &mut self,
        key: &crate::crypto::AeadKey,
        event: &TelemetryEvent,
    ) -> io::Result<()> {
        self.seq += 1;

        let plaintext = serde_json::to_vec(event)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let aad = crate::crypto::RecordAad {
            schema: "pollen.spool.frame.v1".to_string(),
            tenant_id: self.tenant_id.clone(),
            device_id: self.device_id.clone(),
            segment_id: self.segment_id.clone(),
            seq: self.seq,
            key_id: key.key_id().to_string(),
            alg: "AES-256-GCM".to_string(),
        };

        let encrypted = key
            .encrypt_record(aad, &plaintext)
            .map_err(io::Error::other)?;

        let frame = serde_json::to_vec(&encrypted)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if frame.len() > u32::MAX as usize {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }

        let frame_len = frame.len() as u32;
        let checksum = crc32c(&frame);

        self.file.write_all(&frame_len.to_le_bytes())?;
        self.file.write_all(&frame)?;
        self.file.write_all(&checksum.to_le_bytes())?;

        self.file.sync_data()?;
        Ok(())
    }
}

pub fn read_encrypted_records(path: &Path) -> io::Result<Vec<crate::crypto::EncryptedRecord>> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != b"PDS1" {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad spool magic"));
    }

    let mut version = [0u8; 2];
    file.read_exact(&mut version)?;
    if u16::from_le_bytes(version) != 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unsupported spool version"));
    }

    let mut records = Vec::new();
    loop {
        let mut len_buf = [0u8; 4];
        match file.read_exact(&mut len_buf) {
            Ok(()) => {},
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }

        let frame_len = u32::from_le_bytes(len_buf) as usize;
        if frame_len > 16 * 1024 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }

        let mut frame = vec![0u8; frame_len];
        file.read_exact(&mut frame)?;

        let mut crc_buf = [0u8; 4];
        file.read_exact(&mut crc_buf)?;
        let expected = u32::from_le_bytes(crc_buf);
        let actual = crc32c(&frame);
        if expected != actual {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame crc mismatch"));
        }

        let record: crate::crypto::EncryptedRecord = serde_json::from_slice(&frame)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        records.push(record);
    }

    Ok(records)
}
