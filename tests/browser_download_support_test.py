"""A reserved file or progress event must never count as a finished export."""
import unittest
from unittest.mock import Mock
import json
from browser_download_support import Downloads


class DownloadCompletion(unittest.TestCase):
    def observer(self, *messages):
        observer=Downloads.__new__(Downloads)
        observer.records={}
        observer.socket=Mock()
        observer.socket.recv.side_effect=[json.dumps(value) for value in messages]
        return observer

    def begin(self, guid='new', filename='state.txt'):
        return {'method':'Browser.downloadWillBegin','params':{'guid':guid,'suggestedFilename':filename}}

    def progress(self, state, guid='new'):
        return {'method':'Browser.downloadProgress','params':{'guid':guid,'state':state,'receivedBytes':3}}

    def test_waits_past_begin_and_in_progress_until_completed(self):
        observer=self.observer(self.begin(),self.progress('inProgress'),self.progress('completed'))
        self.assertEqual(observer.completed('state.txt',set())['state'],'completed')
        self.assertEqual(observer.socket.recv.call_count,3)

    def test_canceled_download_fails(self):
        observer=self.observer(self.begin(),self.progress('canceled'))
        with self.assertRaisesRegex(AssertionError,'Canceled export'):
            observer.completed('state.txt',set())

    def test_previous_completed_export_cannot_satisfy_next_export(self):
        observer=self.observer(self.begin(),self.progress('completed'))
        observer.record(self.begin('old'));observer.record(self.progress('completed','old'))
        self.assertEqual(observer.completed('state.txt',{'old'})['guid'],'new')
        self.assertEqual(observer.socket.recv.call_count,2)

    def test_duplicate_export_fails(self):
        observer=self.observer(self.begin('first'),self.begin('second'))
        with self.assertRaisesRegex(AssertionError,'Duplicate export'):
            observer.completed('state.txt',set())


if __name__=='__main__':
    unittest.main()
