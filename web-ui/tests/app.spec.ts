import { test, expect } from '@playwright/test';

test.describe('KendraCLI Web UI E2E Tests', () => {
  test('should load the chat page and display the header', async ({ page }) => {
    // Navigate to root, should redirect to /chat
    await page.goto('/');

    // Wait for the URL to redirect to /chat
    await page.waitForURL('**/chat');

    // Assert that the page title is correct (or contains our app name)
    const brand = page.locator('header');
    await expect(brand).toContainText('KENDRA');
  });

  test('should show connection status', async ({ page }) => {
    await page.goto('/chat');
    
    // Check for the connection status pill (it should say either Connected or Offline)
    const statusPill = page.locator('span:has-text("Offline"), span:has-text("Connected")');
    await expect(statusPill).toBeVisible();
  });
});
