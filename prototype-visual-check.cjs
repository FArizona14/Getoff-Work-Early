const { chromium } = require('C:/Users/10728/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const assert = require('node:assert/strict');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const directory = path.join(__dirname,'原型','v0.1');

(async () => {
  const browser = await chromium.launch({channel:'msedge', headless:true});
  try {
    const page = await browser.newPage({viewport:{width:1024,height:1000},colorScheme:'light'});
    const errors = [];
    page.on('pageerror',error=>errors.push(error.message));
    await page.goto(pathToFileURL(path.join(directory,'index.html')).href);
    const ui = page.frameLocator('iframe');
    await ui.locator('#oa-main-time').waitFor();
    await ui.locator('#early-office-prototype').evaluate(()=>{
      const nativeNow = Date.now.bind(Date);
      window.__clockOffset = 0;
      Date.now = () => nativeNow()+window.__clockOffset;
    });
    assert.equal(await ui.locator('#oa-main-time').innerText(),'06 时 08 分');
    await page.screenshot({path:path.join(directory,'redesign-desktop.png'),fullPage:true});
    await ui.locator('#oa-show-float').click();
    assert.match(await ui.locator('#oa-float-time').innerText(),/06:08:\d{2} \/ 9h/);
    assert.equal(await ui.locator('#oa-float-time').evaluate(el=>getComputedStyle(el).fontSize),'12px');
    assert.equal(await ui.locator('#oa-float-time-fill').evaluate(el=>getComputedStyle(el,'::after').animationName),'oa-light-trail');
    const timeBounds = await ui.locator('#oa-float-time').boundingBox();
    const trackBounds = await ui.locator('#oa-float-progress').boundingBox();
    assert.ok(timeBounds.y+timeBounds.height <= trackBounds.y);
    assert.ok(Math.abs(timeBounds.x+timeBounds.width-trackBounds.x-trackBounds.width)<2);
    await ui.locator('#oa-float-card').screenshot({path:path.join(directory,'float-working.png')});
    await ui.locator('#early-office-prototype').evaluate(root=>{
      const workday = new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date());
      window.dispatchEvent(new CustomEvent('openai:set_globals',{detail:{globals:{widgetState:{modelContent:{clockVersion:3,workday,startedAt:Date.now()-(9*3600000-1000),endedAt:null}}}}}));
    });
    await ui.locator('#early-office-prototype').evaluate(()=>{window.__clockOffset+=2000;});
    await page.waitForTimeout(1200);
    assert.match(await ui.locator('#oa-float-status').innerText(),/加班中/);
    assert.equal(await ui.locator('#oa-float-income-block').isVisible(),false);
    assert.equal(await ui.locator('#oa-float-hourly-value').innerText(),'¥62.22 / 小时');
    await ui.locator('#early-office-prototype').evaluate(()=>{window.__clockOffset+=3600000;});
    await page.waitForTimeout(1200);
    const rateAfter10h = parseFloat((await ui.locator('#oa-float-hourly-value').innerText()).replace('¥',''));
    assert.ok(rateAfter10h>=55.98 && rateAfter10h<=56);
    await ui.locator('#oa-float-card').screenshot({path:path.join(directory,'float-overtime.png')});
    await ui.locator('#oa-close-float').click();
    await ui.locator('.oa-nav-item[data-view-target="settings"]').click();
    await ui.locator('#oa-fun-toggle').check();
    await ui.locator('#oa-money-toggle').uncheck();
    assert.equal(await ui.locator('#oa-main-money').innerText(),'金额已隐藏');
    assert.equal(await ui.locator('#oa-float-hourly-value').innerText(),'金额已隐藏');
    await ui.locator('#oa-show-float').click();
    await page.screenshot({path:path.join(directory,'redesign-float.png'),fullPage:true});
    await ui.locator('#oa-close-float').click();
    await ui.locator('#oa-money-toggle').check();
    await ui.locator('#oa-open-add').click();
    await ui.locator('#oa-add-input').fill('下周三提交季度客户分析');
    assert.match(await ui.locator('#oa-parsed-date').innerText(),/11 月 11 日/);
    await ui.locator('#oa-date-override').fill('2026-11-12');
    await ui.locator('#oa-date-override').dispatchEvent('change');
    await ui.locator('#oa-save-add').click();
    assert.equal(await ui.locator('[data-date-group="2026-11-12"] .oa-task-title').innerText(),'提交季度客户分析');
    await ui.locator('.oa-nav-item[data-view-target="report"]').click();
    assert.equal(await ui.locator('.oa-report-block').count(),5);
    for (const width of [1024,736,390,320]) {
      await page.setViewportSize({width,height:1100});
      for (const view of ['today','future','report','settings']) {
        await ui.locator(`.oa-nav-item[data-view-target="${view}"]`).click();
        const sizes = await ui.locator('#early-office-prototype').evaluate(root => ({width:root.clientWidth,scroll:root.scrollWidth}));
        assert.ok(sizes.scroll<=sizes.width+1,`${view} overflows at ${width}: ${JSON.stringify(sizes)}`);
      }
    }
    await ui.locator('.oa-nav-item[data-view-target="today"]').click();
    await ui.locator('#oa-show-float').click();
    await ui.locator('#early-office-prototype').evaluate(()=>{
      window.openai.setWidgetState = async value=>{window.__capturedState=value;};
    });
    await ui.locator('#oa-float-clock-out').click();
    const frozenTime = await ui.locator('#oa-float-time').innerText();
    const frozenRate = await ui.locator('#oa-float-hourly-value').innerText();
    await ui.locator('#early-office-prototype').evaluate(()=>{window.__clockOffset+=60000;});
    await page.waitForTimeout(1200);
    assert.equal(await ui.locator('#oa-float-time').innerText(),frozenTime);
    assert.equal(await ui.locator('#oa-float-hourly-value').innerText(),frozenRate);
    assert.equal(await ui.locator('#oa-demo-forward').isDisabled(),true);
    assert.equal(await ui.locator('#oa-float-clock-out').isDisabled(),true);
    await ui.locator('#early-office-prototype').evaluate(()=>{
      const workday = new Intl.DateTimeFormat('en-CA',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date());
      window.dispatchEvent(new CustomEvent('openai:set_globals',{detail:{globals:{widgetState:{modelContent:{clock:'working',clockVersion:3,workday,startedAt:Date.now()-3600000,endedAt:null}}}}}));
    });
    assert.equal(await ui.locator('#oa-float-status').innerText(),'已下班');
    assert.equal(await ui.locator('#oa-float-time-fill').evaluate(el=>getComputedStyle(el,'::after').display),'none');
    const snapshot = await ui.locator('#early-office-prototype').evaluate(()=>window.__capturedState);
    const fragment = require('node:fs').readFileSync(path.join(directory,'early-off-work-prototype.html'),'utf8');
    const reopened = await browser.newPage();
    await reopened.setContent(`<script>window.openai={widgetState:${JSON.stringify(snapshot)},setWidgetState:async()=>{}};</script>${fragment}`);
    assert.equal(await reopened.locator('#oa-float-status').innerText(),'已下班');
    assert.equal(await reopened.locator('#oa-demo-forward').isDisabled(),true);
    await reopened.close();
    await page.screenshot({path:path.join(directory,'redesign-mobile.png'),fullPage:true});
    assert.deepEqual(errors,[]);
    console.log('PASS: compact timing alignment, animated trailing light, automatic 9h threshold, hourly fee decline, privacy, irreversible clock-out, frozen values, stale snapshot guard, date override, weekly report, responsive widths.');
  } finally {await browser.close();}
})().catch(error=>{console.error(error); process.exitCode=1;});
