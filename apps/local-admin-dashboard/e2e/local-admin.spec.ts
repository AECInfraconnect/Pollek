import { test, expect } from '@playwright/test';
import { installMockApi } from './mock-api';

test('create and publish policy from dashboard', async ({ page }) => {
  await installMockApi(page);
  await page.goto('/');

  await page.getByRole('link', { name: /policy enforcer/i }).click();
  await page.getByRole('button', { name: /new policy/i }).click();
  
  await page.getByLabel(/name/i).fill('E2E Deny Critical');
  await page.getByLabel(/engine/i).selectOption('cedar');
  await page.getByLabel(/source/i).fill('forbid(principal, action, resource) when { context.risk_level == "critical" };');
  await page.getByRole('button', { name: /^save( draft)?$/i }).click();

  await expect(page.getByText('E2E Deny Critical')).toBeVisible();

  const row = page.locator('tr').filter({ hasText: 'E2E Deny Critical' }).first();
  await row.getByRole('button', { name: /^publish$/i }).click();

  await expect(page.getByText(/^Published .* bundle /i)).toBeVisible({ timeout: 10000 });
});
