"""Observe completed downloads in the test's own Chrome session."""
import json
import time
from urllib.request import urlopen
import websocket


class Downloads:
    def __init__(self, browser, directory):
        address = browser.capabilities['goog:chromeOptions']['debuggerAddress']
        with urlopen('http://'+address+'/json/version', timeout=5) as response:
            endpoint = json.load(response)['webSocketDebuggerUrl']
        self.socket = websocket.create_connection(endpoint, timeout=5, suppress_origin=True)
        self.records = {}
        self.socket.send(json.dumps({'id': 1, 'method': 'Browser.setDownloadBehavior',
            'params': {'behavior': 'allow', 'downloadPath': str(directory), 'eventsEnabled': True}}))
        while True:
            message = json.loads(self.socket.recv())
            if message.get('id') == 1:
                if 'error' in message:
                    raise RuntimeError(message['error'])
                break
            self.record(message)

    def record(self, message):
        method = message.get('method')
        if method in ('Browser.downloadWillBegin', 'Browser.downloadProgress'):
            values = message['params']
            self.records.setdefault(values['guid'], {}).update(values)
            assert len(self.records) <= 64, 'Unexpected unbounded fixture downloads'

    def completed(self, filename, previous, timeout=20):
        deadline = time.monotonic()+timeout
        while time.monotonic() < deadline:
            matches = [value for guid, value in self.records.items()
                       if guid not in previous and value.get('suggestedFilename') == filename]
            assert len(matches) <= 1, f'Duplicate export downloads: {self.records}'
            if matches:
                assert matches[0].get('state') != 'canceled', f'Canceled export: {self.records}'
                if matches[0].get('state') == 'completed':
                    return matches[0]
            self.socket.settimeout(max(.001, deadline-time.monotonic()))
            try:
                self.record(json.loads(self.socket.recv()))
            except websocket.WebSocketTimeoutException:
                break
        raise AssertionError(f'Export did not complete: {self.records}')

    def close(self):
        self.socket.close()
