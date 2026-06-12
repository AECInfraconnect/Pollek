import { test, expect } from '@playwright/test';

test('create and publish policy from dashboard', async ({ page }) => {
  await page.goto('http://127.0.0.1:3000');

  await page.getByRole('link', { name: /policy enforcer/i }).click();
  await page.getByRole('button', { name: /new policy/i }).click();
  
  await page.getByLabel(/name/i).fill('E2E Deny Critical');
  await page.getByLabel(/engine/i).selectOption('cedar');
  await page.getByLabel(/source/i).fill('forbid(principal, action, resource) when { context.risk_level == "critical" };');
  await page.getByRole('button', { name: /save draft/i }).click();

  await expect(page.getByText('E2E Deny Critical')).toBeVisible();

  const row = page.locator('tr').filter({ hasText: 'E2E Deny Critical' }).first();
  await row.getByRole('button', { name: /^publish$/i }).click();

  // Wait for the publishing to complete (button text might say Publishing... then toast appears)
  // We can just rely on the test finishing without errors for now or check toast
  await expect(page.getByText(/published/i)).toBeVisible({ timeout: 10000 });
});
