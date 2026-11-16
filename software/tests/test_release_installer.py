"""Exercise real macOS replacement/rollback using tiny signed fixture apps in a temp directory."""
from pathlib import Path
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(sys.platform == 'darwin', 'macOS helper')
class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.TemporaryDirectory(prefix='homecraft-installer-fixture-')
        cls.base = Path(cls.workspace.name)
        source = cls.base / 'fixture.c'; source.write_text('int main(void) { return 0; }\n')
        cls.exe = cls.base / 'fixture'
        subprocess.run(['/usr/bin/clang', '-arch', 'arm64', str(source), '-o', str(cls.exe)], check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls): cls.workspace.cleanup()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='case-', dir=self.base)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.target = self.root / 'Applications/HomeCraft.app'
        self.stage = self.root / 'homecraft-update-fixture'
        self.source = self.stage / 'unpacked/HomeCraft.app'
        self.app(self.target, '0.3.0', '48')
        self.app(self.source, '0.3.1', '49')
        self.private = self.root / '.local/fixture-binding'
        self.private.parent.mkdir(); self.private.write_text('synthetic binding')

    def app(self, app, version, build):
        (app / 'Contents/MacOS').mkdir(parents=True)
        shutil.copy2(self.exe, app / 'Contents/MacOS/HomeCraft')
        (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'local.epaper.bluetooth','CFBundleExecutable':'HomeCraft','CFBundlePackageType':'APPL','CFBundleVersion':build,'CFBundleShortVersionString':version}))
        subprocess.run(['/usr/bin/codesign','--force','--deep','--sign','-',str(app)],check=True,capture_output=True)

    def install(self, env=None):
        # A child that has exited is a known-safe PID fixture; no real app is stopped.
        child = subprocess.Popen(['/usr/bin/true']); child.wait()
        return subprocess.run(['/bin/sh',str(ROOT/'software/tools/install_update.sh'),str(child.pid),str(self.target),str(self.source),'0.3.1','49','0'],capture_output=True,env=env)

    def test_replaces_only_app_after_exit_and_keeps_private_data(self):
        result = self.install(); self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(plistlib.loads((self.target/'Contents/Info.plist').read_bytes())['CFBundleVersion'],'49')
        self.assertEqual(self.private.read_text(),'synthetic binding'); self.assertFalse(self.stage.exists())

    def test_invalid_signature_keeps_original_app(self):
        with (self.source/'Contents/MacOS/HomeCraft').open('ab') as file: file.write(b'broken fixture')
        self.assertNotEqual(self.install().returncode,0)
        self.assertEqual(plistlib.loads((self.target/'Contents/Info.plist').read_bytes())['CFBundleVersion'],'48')
        self.assertEqual(self.private.read_text(),'synthetic binding')

    def test_failed_replacement_rename_rolls_back_original(self):
        commands=self.root/'fixture-commands'; commands.mkdir()
        mv=commands/'mv'
        mv.write_text('#!/bin/sh\ncase "$1" in */.homecraft-update.*/HomeCraft.app) exit 1;; esac\nexec /bin/mv "$@"\n')
        mv.chmod(0o700)
        result=self.install({**os.environ,'PATH':str(commands)+':'+os.environ.get('PATH','/usr/bin:/bin')})
        self.assertNotEqual(result.returncode,0)
        self.assertEqual(plistlib.loads((self.target/'Contents/Info.plist').read_bytes())['CFBundleVersion'],'48')
        self.assertEqual(self.private.read_text(),'synthetic binding')


if __name__ == '__main__': unittest.main()
