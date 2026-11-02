"""Maintainer check: the selected ChromeDriver must launch the selected browser."""
import json
import os
from pathlib import Path
import shutil

from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service

# Match the GUI fixtures' discovery order, including when run through sudo.
chrome = (shutil.which('google-chrome') or shutil.which('google-chrome-stable')
          or shutil.which('chromium'))
driver = (shutil.which('chromedriver')
          or str(Path(os.environ.get('CHROMEWEBDRIVER', '/nonexistent')) / 'chromedriver'))
assert chrome and Path(driver).is_file(), (chrome, driver)
print(json.dumps({'browser': chrome, 'driver': driver,
                  'browser_resolved': str(Path(chrome).resolve()),
                  'driver_resolved': str(Path(driver).resolve())}), flush=True)
options = Options()
options.binary_location = chrome
for flag in ['--headless=new', '--no-sandbox', '--disable-dev-shm-usage', '--mute-audio']:
    options.add_argument(flag)
with webdriver.Chrome(service=Service(driver), options=options) as browser:
    browser.get('data:text/html,<title>GamePKG browser check</title>')
    assert browser.title == 'GamePKG browser check', browser.title
    assert browser.execute_script('return 6 * 7') == 42
    print(json.dumps({'browser_version': browser.capabilities['browserVersion'],
                      'driver_version': browser.capabilities['chrome']['chromedriverVersion']}),
          flush=True)
