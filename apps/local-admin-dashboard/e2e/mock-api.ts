import type { Page, Route } from '@playwright/test';

const externalServer = process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === '1';

const json = (route: Route, body: unknown, status = 200) =>
  route.fulfill({
    status,
    contentType: 'application/json',
    body: JSON.stringify(body),
  });

export async function installMockApi(page: Page) {
  if (externalServer) {
    return;
  }

  const policies: any[] = [];

  await page.route('**/.well-known/pollen-contract', (route) =>
    json(route, {
      schema_version: 'contract-discovery.v1',
      preferred: 'pollen.v1',
      supported: ['pollen.v1'],
      capabilities: ['local-admin-dashboard', 'policy-publish'],
    })
  );

  await page.route('**/v1/tenants/local/connectors', (route) => {
    if (route.request().method() === 'GET') {
      return json(route, []);
    }
    return json(route, { id: 'mock-connector', ok: true });
  });

  await page.route('**/v1/tenants/local/registry/**', (route) => {
    if (route.request().method() === 'GET') {
      return json(route, []);
    }
    return json(route, { ok: true });
  });

  await page.route('**/v1/tenants/local/telemetry/decision-logs', (route) =>
    json(route, { count: 0, decisions: [] })
  );

  await page.route('**/v1/tenants/local/policies', (route) => {
    const method = route.request().method();
    if (method === 'GET') {
      return json(route, policies);
    }
    if (method === 'POST') {
      const policy = route.request().postDataJSON();
      policies.push(policy);
      return json(route, policy, 201);
    }
    return json(route, { error: 'unsupported method' }, 405);
  });

  await page.route(/\/v1\/tenants\/local\/policies\/[^/]+\/publish$/, (route) => {
    const policyId = route.request().url().split('/').at(-2) ?? 'policy';
    const policy = policies.find((p) => p.policy_id === policyId);
    if (policy) {
      policy.meta.status = 'published';
    }
    return json(route, {
      published: true,
      bundle_id: 'bundle-local-1',
      build_number: 1,
    });
  });
}
