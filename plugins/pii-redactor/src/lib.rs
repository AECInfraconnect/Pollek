// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![allow(clippy::unwrap_used)]
#![allow(unsafe_code)]
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct DeterministicDetector;

impl DeterministicDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect(&self, text: &str) -> Vec<DetectedEntity> {
        let mut entities = Vec::new();

        // 1. Email
        static EMAIL_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}").unwrap());
        for cap in EMAIL_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "EMAIL".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 1.0,
                source: "regex".to_string(),
            });
        }

        // 2. Phone Number (Thai + Int)
        static PHONE_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(\+66|0)\s?[689]\s?\d{3}\s?\d{4}").unwrap());
        for cap in PHONE_RE.find_iter(text) {
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
        static CC_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b").unwrap());
        for cap in CC_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "CREDIT_CARD".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.95,
                source: "regex".to_string(),
            });
        }

        // 4. Thai National ID / Tax ID
        static THAI_ID_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b[1-8]-?\d{4}-?\d{5}-?\d{2}-?\d{1}\b").unwrap());
        for cap in THAI_ID_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "THAI_NATIONAL_ID".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.99,
                source: "regex".to_string(),
            });
        }

        // 5. Passport (General alphanumeric 8-9 chars)
        static PASSPORT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b[A-Z0-9]{8,9}\b").unwrap());
        for cap in PASSPORT_RE.find_iter(text) {
            // Because this is generic, confidence is lower. We might want additional context checks.
            entities.push(DetectedEntity {
                entity_type: "PASSPORT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.5,
                source: "regex".to_string(),
            });
        }

        // 6. Bank Account (Thai formats, generally 10-12 digits with hyphens)
        static BANK_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b\d{3}-?\d{1}-?\d{5}-?\d{1}\b").unwrap());
        for cap in BANK_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "BANK_ACCOUNT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.85,
                source: "regex".to_string(),
            });
        }

        // 7. IP Address (v4 for now)
        static IP_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap());
        for cap in IP_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "IP_ADDRESS".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 0.9,
                source: "regex".to_string(),
            });
        }

        // 8. UUID / Device ID
        static UUID_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(
                r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
            )
            .unwrap()
        });
        for cap in UUID_RE.find_iter(text) {
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
        static JWT_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"eyJ[a-zA-Z0-9_-]+\.eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap()
        });
        for cap in JWT_RE.find_iter(text) {
            entities.push(DetectedEntity {
                entity_type: "JWT".to_string(),
                start: cap.start(),
                end: cap.end(),
                value: cap.as_str().to_string(),
                confidence: 1.0,
                source: "regex".to_string(),
            });
        }

        // 10. Generic API Keys / Access Tokens (starts with generic prefixes or long base64 strings)
        static API_KEY_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?i)(?:api_key|access_token|secret)[=:\s]+['\x22]?([a-zA-Z0-9_\-\.]{16,})['\x22]?").unwrap()
        });
        for cap in API_KEY_RE.captures_iter(text) {
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
        Self::new()
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
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    let mut data: Value = match serde_json::from_str(&input) {
        Ok(v) => v,
        Err(_) => return, // Fail silently or log error via WASI stderr
    };

    let detector = DeterministicDetector::new();
    process_json(&mut data, &detector);

    let output = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    let _ = io::stdout().write_all(output.as_bytes());
}
