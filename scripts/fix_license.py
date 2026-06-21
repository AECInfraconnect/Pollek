import os

for root, dirs, files in os.walk('.'):
    if '.git' in root or 'target' in root:
        continue
    for f in files:
        if f == 'Cargo.toml':
            path = os.path.join(root, f)
            with open(path, 'r', encoding='utf-8') as file:
                content = file.read()
            if 'license = "Apache-2.0"' in content and 'workspace.package' not in content:
                content = content.replace('license = "Apache-2.0"', '')
                with open(path, 'w', encoding='utf-8') as file:
                    file.write(content)
