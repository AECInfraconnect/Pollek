import re
import sys

def main():
    path = "crates/local-control-plane/src/policy_first_api.rs"
    with open(path, "r", encoding="utf-8") as f:
        content = f.read()

    # 1. Change `fn generate_real_snapshot` to async
    content = content.replace(
        "fn generate_real_snapshot() -> LocalCapabilitySnapshot {",
        "async fn generate_real_snapshot(st: &AppState) -> LocalCapabilitySnapshot {"
    )

    # 2. Replace the hardcoded `let agents = vec![ ... ];` with DB fetch
    # This regex is a bit complex, let's just do a manual string replace by finding the boundaries.
    start_str = "// Generate agents based on typical dev environment for demo/UX testing\n    let agents = vec!["
    end_str = "    ];\n\n    LocalCapabilitySnapshot {"
    
    start_idx = content.find(start_str)
    if start_idx != -1:
        end_idx = content.find(end_str, start_idx)
        if end_idx != -1:
            db_fetch_code = """// Fetch registered agents from the registry store
    let agents = st.registry_store.list_agent_inventories("local").await.unwrap_or_default();\n\n    LocalCapabilitySnapshot {"""
            content = content[:start_idx] + db_fetch_code + content[end_idx + len(end_str):]
        else:
            print("Could not find end_str")
            sys.exit(1)
    else:
        print("Could not find start_str")
        sys.exit(1)

    # 3. Fix callers
    content = content.replace("let snapshot = generate_real_snapshot();", "let snapshot = generate_real_snapshot(&st).await;")
    content = content.replace("let fresh = generate_real_snapshot();", "let fresh = generate_real_snapshot(&st).await;")
    content = content.replace("None => generate_real_snapshot(),", "None => generate_real_snapshot(&st).await,")

    with open(path, "w", encoding="utf-8") as f:
        f.write(content)
    
    print("Done refactoring policy_first_api.rs")

if __name__ == "__main__":
    main()
