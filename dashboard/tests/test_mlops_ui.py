"""Offline UI tracer; full HTTP acceptance lives in run_mlops_local.py."""
from pathlib import Path
from playwright.sync_api import sync_playwright,expect
root=Path(__file__).resolve().parents[1]
with sync_playwright() as p:
 b=p.chromium.launch(executable_path='C:/Program Files/Google/Chrome/Application/chrome.exe',headless=True)
 page=b.new_page();page.goto((root/'index.html').as_uri())
 expect(page.locator('#mlTab')).to_have_text('Dataset / Experiments')
 page.click('#mlTab');expect(page.locator('#mlPanel')).to_be_visible();expect(page.locator('#monitorPanel')).to_be_hidden()
 expect(page.locator('#mlNotice')).to_contain_text('pred_level')
 expect(page.locator('#mlToken')).to_have_attribute('type','password')
 page.click('#otaTab');expect(page.locator('#mlPanel')).to_be_hidden();expect(page.locator('#otaPanel')).to_be_visible()
 page.click('#monitorTab');page.select_option('#dataMode','demo');expect(page.locator('#logTableBody tr')).to_have_count(61)
 b.close()
print('PASS ML tab isolation, token input, honest warning, preserved OTA and Demo')
