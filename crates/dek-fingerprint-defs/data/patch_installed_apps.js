const fs = require('fs');
const path = 'C:/Projects/AntiG_Pollek_DEK/crates/dek-fingerprint-defs/data/baseline.v3.json';
const data = JSON.parse(fs.readFileSync(path, 'utf8'));

for (let app of data.installed_app_signatures) {
    if (app.id === 'chatgpt_desktop') {
        app.process_names = ["ChatGPT.exe", "ChatGPT", "chatgpt"];
    } else if (app.id === 'claude_desktop' || app.id === 'claude_desktop_app') {
        app.process_names = ["Claude.exe", "Claude", "claude"];
    }
}

fs.writeFileSync(path, JSON.stringify(data, null, 2));
console.log('patched');
