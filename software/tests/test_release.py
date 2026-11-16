"""Offline release contract tests; Git repositories and assets are synthetic temporary fixtures."""
import importlib.util
import json
from pathlib import Path
import plistlib
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('homecraft_release', ROOT / 'software/tools/release.py')
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='homecraft-release-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.meta = release.metadata()
        self.tag = 'v' + self.meta['version']

    def archive(self, extra=None):
        path = self.root / (release.stem(self.meta) + '.zip')
        files = {
            'HomeCraft.app/Contents/Info.plist': plistlib.dumps(release.plist(self.meta)),
            'HomeCraft.app/Contents/MacOS/HomeCraft': b'synthetic executable',
            'HomeCraft.app/Contents/Resources/HomeCraft.icns': b'synthetic icon',
            'HomeCraft.app/Contents/Resources/poems.json': b'[]',
            'HomeCraft.app/Contents/_CodeSignature/CodeResources': b'synthetic signature',
        }
        files.update(extra or {})
        with zipfile.ZipFile(path, 'w') as z:
            for name, data in files.items():
                z.writestr(name, data)
        (self.root / (release.stem(self.meta) + '.dmg')).write_bytes(b'synthetic dmg')
        return path

    def test_manifest_uses_exact_version_repository_assets_and_hashes(self):
        self.archive()
        m = release.write_manifest(self.meta, self.tag, self.meta['repository'], self.root, '更新说明')
        self.assertEqual(m['build'], self.meta['build'])
        self.assertEqual(m['notes'], '更新说明')
        self.assertEqual(set(m['platforms']), {'macos-arm64'})
        a = m['platforms']['macos-arm64']
        self.assertEqual(a['url'], f"https://github.com/{self.meta['repository']}/releases/download/{self.tag}/{release.stem(self.meta)}.zip")
        self.assertEqual(a['sha256'], release.hashlib.sha256((self.root / (release.stem(self.meta) + '.zip')).read_bytes()).hexdigest())
        self.assertEqual(len((self.root / 'SHA256SUMS.txt').read_text().splitlines()), 2)

    def test_tag_repository_mismatch_and_extra_zip_paths_are_rejected(self):
        with self.assertRaises(ValueError): release.check_tag(self.meta, 'v9.9.9')
        with self.assertRaises(ValueError): release.check_tag(self.meta, self.tag, 'other/repo')
        path = self.archive({'HomeCraft.app/.local/.ble-pairing.json': b'{}'})
        with self.assertRaises(ValueError): release.validate_archive(path, self.meta)
        path = self.archive({'../outside/': b''})
        with self.assertRaises(ValueError): release.validate_archive(path, self.meta)

    def test_wrong_bundle_version_cannot_be_published(self):
        info = release.plist(self.meta); info['CFBundleVersion'] = '1'
        path = self.archive({'HomeCraft.app/Contents/Info.plist': plistlib.dumps(info)})
        with self.assertRaises(ValueError): release.validate_archive(path, self.meta)

    def test_annotated_tag_notes_and_checked_in_fallback(self):
        def git(*args): return subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True)
        folder = self.root / 'software/release-notes'; folder.mkdir(parents=True)
        (folder / f'{self.tag}.md').write_text('fallback notes')
        git('init', '-q')
        self.assertEqual(release.notes(self.meta, self.tag, self.root), 'fallback notes')
        git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '--allow-empty', '-qm', 'fixture')
        git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'tag', '-a', self.tag, '-m', 'annotated notes')
        self.assertEqual(release.notes(self.meta, self.tag, self.root), 'annotated notes')

    def test_draft_uploads_finish_before_publication_and_old_versions_do_not_become_latest(self):
        calls = []
        def run(args, **kwargs):
            calls.append(args)
            if args[1:3] == ['release', 'view']: return SimpleNamespace(returncode=1, stdout='')
            if args[1] == 'api': return SimpleNamespace(returncode=0, stdout='{"tag_name":"v99.0.0"}')
            return SimpleNamespace(returncode=0, stdout='')
        with patch.object(release.subprocess, 'run', side_effect=run), patch.object(release.subprocess, 'check_output', return_value='public\n'):
            release.publish(self.meta, self.tag, self.meta['repository'], self.root)
        create = next(i for i,c in enumerate(calls) if c[1:3] == ['release','create'])
        upload = next(i for i,c in enumerate(calls) if c[1:3] == ['release','upload'])
        publish = next(i for i,c in enumerate(calls) if c[1:3] == ['release','edit'])
        self.assertLess(create, upload); self.assertLess(upload, publish)
        self.assertIn('--draft', calls[create]); self.assertIn('--draft=false', calls[publish]); self.assertIn('--latest=false', calls[publish])

    def test_published_release_is_never_overwritten(self):
        with patch.object(release.subprocess, 'run', return_value=SimpleNamespace(returncode=0, stdout='{"isDraft":false}')), patch.object(release.subprocess, 'check_output', return_value='public\n'):
            with self.assertRaises(ValueError): release.publish(self.meta, self.tag, self.meta['repository'], self.root)

    def test_first_release_can_be_latest_but_network_failure_keeps_draft(self):
        for error, succeeds in [('gh: Not Found (HTTP 404)', True), ('connection timed out', False)]:
            calls = []
            def run(args, **kwargs):
                calls.append(args)
                if args[1:3] == ['release', 'view']:
                    return SimpleNamespace(returncode=0, stdout='{"isDraft":true}')
                if args[1] == 'api':
                    return SimpleNamespace(returncode=1, stderr=error)
                return SimpleNamespace(returncode=0, stdout='')
            with patch.object(release.subprocess, 'run', side_effect=run), patch.object(release.subprocess, 'check_output', return_value='public\n'):
                if succeeds:
                    release.publish(self.meta, self.tag, self.meta['repository'], self.root)
                    self.assertIn('--latest=true', calls[-1])
                else:
                    with self.assertRaises(ValueError): release.publish(self.meta, self.tag, self.meta['repository'], self.root)
                    self.assertFalse(any(c[1:3] == ['release', 'edit'] for c in calls))

    def test_private_distribution_feed_is_rejected_before_publishing(self):
        with patch.object(release.subprocess, 'check_output', return_value='private\n'), patch.object(release.subprocess, 'run') as run:
            with self.assertRaises(ValueError): release.publish(self.meta, self.tag, self.meta['repository'], self.root)
            run.assert_not_called()


if __name__ == '__main__': unittest.main()
