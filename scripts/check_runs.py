import urllib.request
import json
try:
    req = urllib.request.Request("https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs?per_page=10", headers={'User-Agent': 'Mozilla/5.0'})
    with urllib.request.urlopen(req) as response:
        data = json.loads(response.read().decode())
        for r in data['workflow_runs']:
            print(f"{r['name']} - {r['status']} - {r['conclusion']} - {r['head_branch']} - {r['event']}")
except Exception as e:
    print(e)
