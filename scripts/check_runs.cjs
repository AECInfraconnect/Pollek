const https = require('https');

https.get('https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs?per_page=10', {
  headers: { 'User-Agent': 'Node.js' }
}, (res) => {
  let data = '';
  res.on('data', (chunk) => { data += chunk; });
  res.on('end', () => {
    try {
      const runs = JSON.parse(data).workflow_runs;
      for (const r of runs) {
        console.log(`${r.name} - ${r.status} - ${r.conclusion} - ${r.head_branch} - ${r.event}`);
      }
    } catch (e) {
      console.error(e.message);
    }
  });
}).on('error', (e) => {
  console.error(e.message);
});
