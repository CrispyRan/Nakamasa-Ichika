const { chromium } = require('playwright');

async function screenshot() {
  try {
    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ ignoreHTTPSErrors: true });
    await page.setViewportSize({ width: 1400, height: 900 });
    
    await page.goto('https://localhost:8889/', { waitUntil: 'networkidle', timeout: 30000 });
    await page.waitForTimeout(3000);
    await page.screenshot({ path: 'user-list.png', fullPage: true });
    
    await browser.close();
    console.log('Screenshot saved');
  } catch (e) {
    console.error('Error:', e.message);
  }
}

screenshot();
