import { expect, test } from '@playwright/test';

test('compact timeline keeps controls fixed and scrolls independently', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 380, height: 560 });
  await page.goto('/?surface=compact&platform=macos');
  const header = page.locator('.compact-header');
  const timeline = page.getByTestId('timeline-scroll');
  await expect(page.getByText('Recording', { exact: true })).toBeVisible();
  await expect(page.getByRole('img', { name: '5 sources' })).toBeVisible();
  await page.screenshot({ path: testInfo.outputPath('compact-macos-top.png') });
  const before = await header.boundingBox();
  await timeline.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const after = await header.boundingBox();
  expect(after?.y).toBe(before?.y);
  expect(await timeline.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(380);
  await page.screenshot({ path: testInfo.outputPath('compact-macos-scrolled.png') });
});

test('compact surface remains usable at the minimum supported height', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 380, height: 420 });
  await page.goto('/?surface=compact&platform=windows');
  await expect(page.getByRole('button', { name: 'Pause collection' })).toBeVisible();
  await expect(page.getByRole('button', { name: /private activity/i })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(380);
  await page.screenshot({ path: testInfo.outputPath('compact-windows-low-height.png') });
});

for (const viewport of [
  { width: 760, height: 520, name: 'minimum' },
  { width: 960, height: 680, name: 'default' },
  { width: 1440, height: 900, name: 'expanded' },
]) {
  test(`full history fits the ${viewport.name} window`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.goto('/?surface=history');
    await expect(page.getByRole('navigation', { name: 'Primary navigation' })).toBeVisible();
    await expect(page.getByRole('searchbox', { name: 'Search history' })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(viewport.width);
    expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(viewport.height);
    await page.screenshot({ path: testInfo.outputPath(`history-${viewport.name}.png`) });
  });
}
