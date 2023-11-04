import { expect, test } from '@playwright/test';

test('index page has expected content', async ({ page }) => {
	await page.goto('/audio');
	await expect(page.getByText('hello world')).toBeVisible();
});
