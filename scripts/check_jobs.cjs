const https = require('https');
const fs = require('fs');

https.get('https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs?per_page=10', {
  headers: { 'User-Agent': 'Node.js' }
}, (res) => {
  let data = '';
  res.on('data', (chunk) => { data += chunk; });
  res.on('end', () => {
    const runs = JSON.parse(data).workflow_runs;
    const activeRuns = runs.filter(r => r.status === 'in_progress' || r.status === 'queued');
    
    activeRuns.forEach(run => {
      https.get(run.jobs_url, { headers: { 'User-Agent': 'Node.js' } }, (res2) => {
        let jdata = '';
        res2.on('data', (c) => jdata += c);
        res2.on('end', () => {
          const jobs = JSON.parse(jdata).jobs;
          if (jobs) {
              for (const j of jobs) {
                 if (j.conclusion === 'failure') {
                     console.log(`Failed job: ${j.name} in run ${run.name}`);
                     console.log(`Log URL: ${j.url}/logs`);
                 }
              }
          }
        });
      });
    });
  });
});
