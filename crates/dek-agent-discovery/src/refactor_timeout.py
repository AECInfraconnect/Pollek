import re

path = "C:/Projects/AntiG_Pollek_DEK/crates/dek-agent-discovery/src/orchestrator.rs"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

# E.g.
# if let Ok(mut x) = tokio::task::spawn_blocking(crate::mcp_scan::scan_mcp_configs)
#     .await
#     .unwrap_or(Ok(vec![]))

# We want to replace it with:
# if let Ok(Ok(mut x)) = tokio::time::timeout(
#     std::time::Duration::from_secs(self.config.source_timeout_secs),
#     tokio::task::spawn_blocking(...)
# ).await.unwrap_or(Ok(Ok(vec![])))

# Actually, the tasks clone `self.config` as `config`.
# Wait, some already clone `config` if they need it (like web_ai), others don't.
# If they don't, we can add `let config = self.config.clone();` at the beginning of `if wants_source(...)` block, before `tasks.push`.
# Wait, all of them have `let tx_cl = ev_tx.clone();`. We can just add `let config = self.config.clone();` after that.

content = re.sub(r'(let tx_cl = ev_tx\.clone\(\);(?!\s*let config = self\.config\.clone\(\);))',
                 r'\1\n            let config = self.config.clone();',
                 content)

# Now, replace the spawn_blocking calls.
# Pattern: tokio::task::spawn_blocking( ... ) .await .unwrap_or(...)
# Or tokio::task::spawn_blocking( move || { ... } ) .await .unwrap_or(...)

# Let's match the `if let Ok(mut x) = tokio::task::spawn_blocking` and similar
# Actually it's easier to replace the specific cases or just use a generic regex for `tokio::task::spawn_blocking( ... ).await.unwrap_or(Ok(vec![]))`

# The generic pattern:
# tokio::task::spawn_blocking(\s*.*?\s*)\s*\.await\s*\.unwrap_or\(Ok\(vec\!\[\]\)\)
# We want to capture the argument to spawn_blocking.

pattern1 = r'tokio::task::spawn_blocking\((.*?)\)\s*\.await\s*\.unwrap_or\(Ok\(vec\!\[\]\)\)'

def replacer1(m):
    inner = m.group(1)
    return f'tokio::time::timeout(std::time::Duration::from_secs(config.source_timeout_secs), tokio::task::spawn_blocking({inner})).await.unwrap_or(Ok(Ok(vec![]))).unwrap_or(Ok(vec![]))'

content = re.sub(pattern1, replacer1, content, flags=re.DOTALL)

# For probe_local_models().await
pattern2 = r'crate::local_model_probe::probe_local_models\(\)\.await'
def replacer2(m):
    return f'tokio::time::timeout(std::time::Duration::from_secs(config.source_timeout_secs), crate::local_model_probe::probe_local_models()).await.unwrap_or(Ok(vec![]))'
content = re.sub(pattern2, replacer2, content)

# For python_framework_scan, it didn't have unwrap_or(Ok(vec![])):
# tokio::task::spawn_blocking(crate::python_framework_scan::scan_python_frameworks).await
pattern3 = r'tokio::task::spawn_blocking\(\s*crate::python_framework_scan::scan_python_frameworks\s*,\s*\)\s*\.await'
def replacer3(m):
    return f'tokio::time::timeout(std::time::Duration::from_secs(config.source_timeout_secs), tokio::task::spawn_blocking(crate::python_framework_scan::scan_python_frameworks)).await.unwrap_or(Ok(Ok(vec![]))).unwrap_or(Ok(vec![]))'
content = re.sub(pattern3, replacer3, content)

with open(path, "w", encoding="utf-8") as f:
    f.write(content)

print("Done")
