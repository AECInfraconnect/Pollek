import urllib.request, json
urls = [
    'https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs/27326069539/jobs',
    'https://api.github.com/repos/AECInfraconnect/AntiG_Pollen_DEK/actions/runs/27326069549/jobs'
]
for url in urls:
    print(f"--- URL: {url} ---")
    req = urllib.request.Request(url)
    response = urllib.request.urlopen(req)
    data = json.loads(response.read())
    for job in data.get('jobs', []):
        if job.get('conclusion') == 'failure':
            print(f"Failed Job: {job['name']}")
            for step in job.get('steps', []):
                if step.get('conclusion') == 'failure':
                    print(f"  Failed Step: {step['name']}")
