import { test, expect } from '@playwright/test';
import { installMockApi } from './mock-api';

test('load contract discovery in settings', async ({ page }) => {
  await installMockApi(page);
  await page.goto('/');
  
  // Navigate to Settings
  await page.getByRole('link', { name: /settings/i }).click();

  // Wait for Contract Discovery to load and verify "Preferred Contract" exists
  await expect(page.getByText('Contract Discovery')).toBeVisible();
  
  // Check that the schema version appears
  await expect(page.getByText('contract-discovery.v1')).toBeVisible({ timeout: 10000 });
});
