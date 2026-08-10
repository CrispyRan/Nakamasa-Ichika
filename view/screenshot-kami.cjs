const { chromium } = require('playwright');

async function screenshot() {
  try {
    console.log('Launching browser...');
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext({ ignoreHTTPSErrors: true });
    const page = await context.newPage();
    await page.setViewportSize({ width: 1400, height: 900 });
    
    // 访问登录页面
    console.log('Navigating to login page...');
    await page.goto('https://localhost:8889/', { waitUntil: 'networkidle', timeout: 30000 });
    await page.waitForTimeout(2000);
    
    // 检查是否在登录页面
    const url = page.url();
    console.log('Current URL:', url);
    
    // 尝试登录 (使用 mock 模式应该有默认登录)
    // 检查是否有登录表单
    const loginForm = await page.$('input[type="password"]');
    if (loginForm) {
      console.log('Found login form, attempting login...');
      
      // 输入用户名
      const usernameInput = await page.$('input[type="text"]') || await page.$('input:first-of-type');
      if (usernameInput) {
        await usernameInput.fill('admin');
        await page.waitForTimeout(500);
      }
      
      // 输入密码
      await loginForm.fill('admin123');
      await page.waitForTimeout(500);
      
      // 点击登录按钮
      const loginBtn = await page.$('button[type="submit"]') || await page.$('button:has-text("登录")');
      if (loginBtn) {
        await loginBtn.click();
        await page.waitForTimeout(3000);
        console.log('Login attempted');
      }
    }
    
    // 导航到卡密列表页面
    console.log('Navigating to kami list page...');
    await page.goto('https://localhost:8889/kami/list', { waitUntil: 'networkidle', timeout: 30000 });
    await page.waitForTimeout(3000);
    
    // 截图
    console.log('Taking screenshot...');
    await page.screenshot({ path: 'kami-list.png', fullPage: true });
    
    // 获取页面标题
    const title = await page.title();
    console.log('Page title:', title);
    
    // 检查控制台错误
    page.on('console', msg => {
      if (msg.type() === 'error') {
        console.log('Console error:', msg.text());
      }
    });
    
    await browser.close();
    console.log('Screenshot saved to kami-list.png');
  } catch (e) {
    console.error('Error:', e.message);
    process.exit(1);
  }
}

screenshot();
