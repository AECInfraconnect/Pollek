#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use regex::Regex;
use serde_json::Value;
use std::io::{self, Read, Write};

#[link(wasm_import_module = "pollen_env")]
extern "C" {
    fn ner_predict(ptr: *const u8, len: usize, out_ptr: *mut u8, max_out_len: usize) -> usize;
}

fn do_ner_redaction(text: &str) -> String {
    let mut out_buf = vec![0u8; text.len() * 2 + 1024]; // ample space
    let written = unsafe {
        ner_predict(
            text.as_ptr(),
            text.len(),
            out_buf.as_mut_ptr(),
            out_buf.len(),
        )
    };
    if written > 0 && written <= out_buf.len() {
        String::from_utf8_lossy(&out_buf[..written]).into_owned()
    } else {
        text.to_string()
    }
}

fn redact_value(v: &mut Value, email_re: &Regex, ssn_re: &Regex) {
    match v {
        Value::String(s) => {
            let mut result = s.clone();
            result = email_re
                .replace_all(&result, "[REDACTED_EMAIL]")
                .to_string();
            result = ssn_re.replace_all(&result, "[REDACTED_SSN]").to_string();

            // Try NER Redaction via host function
            // We use a small heuristic to avoid calling NER on short/empty strings
            if result.len() > 3 {
                result = do_ner_redaction(&result);
            }

            *s = result;
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_value(item, email_re, ssn_re);
            }
        }
        Value::Object(obj) => {
            for (_, val) in obj.iter_mut() {
                redact_value(val, email_re, ssn_re);
            }
        }
        _ => {}
    }
}

fn main() {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return;
    }

    if let Ok(mut json) = serde_json::from_str::<Value>(&input) {
        let email_re = Regex::new(r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}")
            .unwrap_or_else(|e| panic!("Invalid regex: {}", e));
        let ssn_re =
            Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap_or_else(|e| panic!("Invalid regex: {}", e));

        redact_value(&mut json, &email_re, &ssn_re);

        if let Ok(out) = serde_json::to_string(&json) {
            let _ = io::stdout().write_all(out.as_bytes());
        }
    } else {
        let _ = io::stdout().write_all(input.as_bytes());
    }
}
