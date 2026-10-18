"""Local UI regression: no database, OTA uploads, or external requests."""
from pathlib import Path
from playwright.sync_api import sync_playwright

DASHBOARD = Path(__file__).resolve().parents[1]

def test_theme_toggle():
    with sync_playwright() as p:
        browser = p.chromium.launch(channel='msedge')
        page = browser.new_page(viewport={"width": 1440, "height": 900}, color_scheme="dark")
        errors = []
        page.on("pageerror", lambda e: errors.append(str(e)))
        page.goto((DASHBOARD / 'index.html').as_uri())
        page.evaluate("localStorage.removeItem('coffee-lab-theme')")
        page.reload()
        button = page.locator('#themeToggle')
        assert button.evaluate('(e) => e.tagName') == 'BUTTON'
        assert button.locator('svg').count() == 1
        assert button.locator('input').count() == 0
        assert not page.locator('html').evaluate("e => e.classList.contains('light')")
        assert button.locator('circle').count() == 0
        evidence = DASHBOARD / 'tests' / 'evidence'
        evidence.mkdir(exist_ok=True)
        page.locator('.app-header').screenshot(path=str(evidence / 'theme-dark.png'))
        button.click()
        assert page.locator('html').evaluate("e => e.classList.contains('light')")
        assert button.locator('circle').count() == 1
        page.locator('.app-header').screenshot(path=str(evidence / 'theme-light.png'))
        page.reload()
        assert page.locator('html').evaluate("e => e.classList.contains('light')")
        button.focus()
        page.keyboard.press('Space')
        assert not page.locator('html').evaluate("e => e.classList.contains('light')")
        page.keyboard.press('Enter')
        assert page.locator('html').evaluate("e => e.classList.contains('light')")
        for width in (320, 768, 1024, 1440):
            page.set_viewport_size({"width": width, "height": 900})
            assert button.locator('svg').count() == 1
            box = button.bounding_box()
            assert box['width'] == 36 and box['height'] == 36
        assert not errors, errors
        browser.close()
    print('PASS: one icon, dark/light click, Space/Enter, reload persistence, fixed control size, no JS errors')

if __name__ == '__main__':
    test_theme_toggle()
