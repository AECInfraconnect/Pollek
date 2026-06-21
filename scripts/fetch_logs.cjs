const https = require('https');
const fs = require('fs');

const runId = '27389005581';

https.get(`https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs/${runId}/jobs`, {
  headers: { 'User-Agent': 'Node.js' }
}, (res) => {
  let data = '';
  res.on('data', (chunk) => { data += chunk; });
  res.on('end', () => {
    const jobs = JSON.parse(data).jobs;
    const failedJob = jobs.find(j => j.name === 'Build eBPF Bytecode' && j.conclusion === 'failure');
    if (failedJob) {
        console.log(`Failed job URL: ${failedJob.url}`);
        console.log(`To get logs, use: curl -L -s -H "User-Agent: Node.js" ${failedJob.url}/logs`);
    }
  });
});
