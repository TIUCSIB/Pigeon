// Render README SVGs to PNG previews via headless Chromium (dev-only helper)
const { chromium } = require('playwright')
const path = require('path')

const jobs = [
  { svg: 'assets/readme/hero.svg', out: 'assets/readme/preview/hero.png', width: 1200 },
  { svg: 'assets/readme/architecture.svg', out: 'assets/readme/preview/architecture.png', width: 1200 },
]

;(async () => {
  const browser = await chromium.launch()
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, deviceScaleFactor: 2 })
  for (const job of jobs) {
    const url = 'file:///' + path.resolve(job.svg).replace(/\\/g, '/')
    await page.goto(url)
    const el = await page.$('svg')
    await el.screenshot({ path: job.out })
    console.log('rendered', job.out)
  }
  await browser.close()
})().catch((e) => {
  console.error('ERR', e.message)
  process.exit(1)
})
