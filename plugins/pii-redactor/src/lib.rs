// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub enum PluginError {
    RegexCompile(regex::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::RegexCompile(e) => write!(f, "Regex compilation failed: {}", e),
            PluginError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for PluginError {}

impl From<regex::Error> for PluginError {
    fn from(err: regex::Error) -> Self {
        PluginError::RegexCompile(err)
    }
}

/// Define the standard entity type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEntity {
    pub entity_type: String,
    pub start: usize,
    pub end: usize,
    pub value: String,
    pub confidence: f32,
    pub source: String,
}

/// Interface for integrating remote NER models
pub trait NerProvider {
    fn detect_entities(&self, text: &str) -> Result<Vec<DetectedEntity>, String>;
}

/// Provider for flexible custom entity types (Zero-shot NER)
pub struct GlinerProvider {
    pub endpoint: String,
}

impl NerProvider for GlinerProvider {
    fn detect_entities(&self, _text: &str) -> Result<Vec<DetectedEntity>, String> {
        // TODO: Implement HTTP call to GLiNER service
        Ok(vec![])
    }
}

/// Provider for Thai / multilingual PII
pub struct MultilingualNerProvider {
    pub endpoint: String,
}

impl NerProvider for MultilingualNerProvider {
    fn detect_entities(&self, _text: &str) -> Result<Vec<DetectedEntity>, String> {
        // TODO: Implement HTTP call to Multilingual NER model
        Ok(vec![])
    }
}

/// Provider for Healthcare / legal / government (Domain-specific NER)
pub struct DomainNerProvider {
    pub endpoint: String,
}

impl NerProvider for DomainNerProvider {
    fn detect_entities(&self, _text: &str) -> Result<Vec<DetectedEntity>, String> {
        // TODO: Implement HTTP call to Domain-specific NER model
        Ok(vec![])
    }
}

/// Provider for High-accuracy enterprise mode (NER + rules + LLM verification)
pub struct HighAccuracyEnterpriseProvider {
    pub ner_endpoint: String,
    pub llm_endpoint: String,
}

impl NerProvider for HighAccuracyEnterpriseProvider {
    fn detect_entities(&self, _text: &str) -> Result<Vec<DetectedEntity>, String> {
        // TODO: Implement multi-step NER and LLM verification workflow
        Ok(vec![])
    }
}

/// A deterministic PII detector based on Regex and rules
pub struct DeterministicDetector {
    email_re: Regex,
    phone_re: Regex,
    cc_re: Regex,
    thai_id_re: Regex,
    passport_re: Regex,
    bank_re: Regex,
    ip_re: Regex,
    uuid_re: Regex,
    jwt_re: Regex,
    api_key_re: Regex,
}

impl DeterministicDetector {
    pub fn new() -> Result<Self, PluginError> {
        Ok(Self {
            email_re: Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}")?,
            phone_re: Regex::new(r"(\+66|0)\s?[689]\s?\d{3}\s?\d{4}")?,
            cc_re: Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b")?,
            thai_id_re: Regex::new(r"\b[1-8]-?\d{4}-?\d{5}-?\d{2}-?\d{1}\b")?,
            passport_re: Regex::new(r"\b[A-Z0-9]{8,9}\b")?,
            bank_re: Regex::new(r"\b\d{3}-?\d{1}-?\d{5}-?\d{1}\b")?,
            ip_re: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")?,
            uuid_re: Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")?,
            jwt_re: Regex::new(r"eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+")?,
            api_key_re: Regex::new(r"(?i)(?:api_key|access_token|secret)[=:\s]+['\x22]?([a-zA-Z0-9_\-\.]{16,})['\x22]?")?,
        })
    }

    pub fn detect(&self, text: &str) -> Vec<DetectedEntity> {
        let mut entities = Vec::new();

        // 1. Email
        for cap in self.email_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "EMAIL".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 1.0,
                source: "regex".to_string(),
            });
        }

        // 2. Phone Number
        for cap in self.phone_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "PHONE".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.9,
                source: "regex".to_string(),
            });
        }

        // 3. Credit Card
        for cap in self.cc_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "CREDIT_CARD".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.95,
                source: "regex".to_string(),
            });
        }

        // 4. Thai National ID
        for cap in self.thai_id_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "THAI_NATIONAL_ID".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.99,
                source: "regex".to_string(),
            });
        }

        // 5. Passport
        for cap in self.passport_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "PASSPORT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.5,
                source: "regex".to_string(),
            });
        }

        // 6. Bank Account
        for cap in self.bank_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "BANK_ACCOUNT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.85,
                source: "regex".to_string(),
            });
        }

        // 7. IP Address
        for cap in self.ip_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "IP_ADDRESS".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.9,
                source: "regex".to_string(),
            });
        }

        // 8. UUID
        for cap in self.uuid_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "UUID".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 1.0,
                source: "regex".to_string(),
            });
        }

        // 9. JWT
        for cap in self.jwt_re.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "JWT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 1.0,
                source: "regex".to_string(),
            });
        }

        // 10. API Keys
        for cap in self.api_key_re.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                entities.push(DetectedEntity {
                    entity_type: "API_KEY".to_string(),
                    start: m.start(),
                    end: m.end(),
                    value: m.as_str().to_string(),
                    confidence: 0.9,
                    source: "regex".to_string(),
                });
            }
        }

        entities
    }

    pub fn redact(&self, text: &str) -> String {
        let mut result = text.to_string();
        let mut entities = self.detect(text);

        // Sort in reverse order to replace without disrupting earlier indices
        entities.sort_by_key(|b| std::cmp::Reverse(b.start));

        for entity in entities {
            let replacement = format!("[REDACTED_{}]", entity.entity_type);
            result.replace_range(entity.start..entity.end, &replacement);
        }

        result
    }
}

impl Default for DeterministicDetector {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| panic!("Failed to compile regexes")) // Unused in this context, just satisfying Default for now.
    }
}

pub fn process_json(value: &mut Value, detector: &DeterministicDetector) {
    match value {
        Value::String(s) => {
            *s = detector.redact(s);
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                process_json(item, detector);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj.iter_mut() {
                process_json(val, detector);
            }
        }
        _ => {}
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {
    use std::io::{self, Read, Write};
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    let mut data: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return, // Fail silently or log error via WASI stderr
    };

    let detector = match DeterministicDetector::new() {
        Ok(d) => d,
        Err(_) => return,
    };
    process_json(&mut data, &detector);

    let output = match serde_json::to_string(&data) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = io::stdout().write_all(output.as_bytes());
}
