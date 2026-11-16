"""Offline checks for Git upload boundaries; never touches the real index or devices."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('check_git_contents', ROOT / 'software/tools/check_git_contents.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class GitContentsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='homecraft-git-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git('init', '-q')
        (self.root / '.gitignore').write_bytes((ROOT / '.gitignore').read_bytes())
        self.write('reTerminalE1002/.gitignore', (ROOT / 'reTerminalE1002/.gitignore').read_bytes())

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True).stdout

    def write(self, name, value=b'synthetic fixture\n'):
        file = self.root / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(value)
        return file

    def test_generated_private_and_downloaded_files_are_ignored(self):
        for name in [
            'software/mac-app/target/debug/app', '3Inch7EPaper/build/firmware.bin',
            'new-board/build-debug/compile_commands.json', 'software/dist/HomeCraft.app/Contents/Info.plist',
            'new-board/.local/.ble-pairing.json', '3Inch7EPaper/backups/recovery.bin',
            'software/artifacts/preview/image.png', 'reTerminalE1002/.arduino/core/file',
            'reTerminalE1002/reference/Seeed_GxEPD2/source.cpp', 'reTerminalE1002/reference/schematic.pdf',
            'software/.env', '3Inch7EPaper/.vscode/settings.json',
            'software/auth.json', 'software/credentials.json', 'software/private.pem',
            'software/.ssh/id_ed25519', 'software/.aws/credentials',
            'gitleaks-report.json',
        ]:
            self.write(name)
            self.assertTrue(self.git('check-ignore', name).strip(), name)

    def test_required_sources_locks_and_directory_readmes_remain_candidates(self):
        names = ['software/mac-app/Cargo.lock', '3Inch7EPaper/dependencies.lock',
                 '3Inch7EPaper/components/network_provisioning/src/manager.c',
                 'reTerminalE1002/main/fonts.c', 'reTerminalE1002/sdkconfig.homecraft',
                 '3Inch7EPaper/sdkconfig.defaults', 'software/artifacts/README.md',
                 'reTerminalE1002/reference/README.md', 'reTerminalE1002/reference/sources.lock.json',
                 'software/assets/brands/openai.png', 'software/tools/ble-settings.example.json']
        for name in names:
            self.write(name)
        files = guard.paths(self.git('ls-files', '--others', '--exclude-standard', '-z'))
        self.assertTrue(set(names).issubset(files))
        self.assertFalse(guard.check(self.root)[2])

    def test_force_added_private_file_is_reported(self):
        self.write('3Inch7EPaper/.local/.ble-pairing.json', b'{}\n')
        self.git('add', '-f', '3Inch7EPaper/.local/.ble-pairing.json')
        self.assertTrue(guard.check(self.root, staged=True)[2])

    def test_checks_index_bytes_even_when_working_copy_is_clean(self):
        secret = b'ghp_' + b'1' * 36
        file = self.write('config.txt', secret)
        self.git('add', 'config.txt')
        file.write_text('placeholder\n')
        for staged in (False, True):
            issues = guard.check(self.root, staged)[2]
            self.assertTrue(any('GitHub token' in reason for _, _, reason in issues))
            self.assertNotIn(secret.decode(), str(issues))

    def test_external_symlink_and_oversized_file_are_reported(self):
        (self.root / 'external').symlink_to('/private/tmp')
        with (self.root / 'large.txt').open('wb') as file:
            file.truncate(guard.MAX_FILE + 1)
        reasons = [reason for _, _, reason in guard.check(self.root)[2]]
        self.assertTrue(any('符号链接' in reason for reason in reasons))
        self.assertTrue(any('20 MiB' in reason for reason in reasons))

    def test_task_history_path_is_not_mistaken_for_an_api_key(self):
        self.write('notes.md', b'3Inch7EPaper/docs/task-history-before-split-20261026.md\n')
        self.assertFalse(guard.check(self.root)[2])

    def test_parent_symlink_cannot_read_outside_the_repository(self):
        with tempfile.TemporaryDirectory(prefix='homecraft-private-fixture-') as directory:
            private = Path(directory)
            (private / 'credential.txt').write_text('synthetic fixture\n')
            (self.root / 'linked').symlink_to(private, target_is_directory=True)
            # Simulate a tracked path whose parent has since become an external link.
            self.git('update-index', '--add', '--cacheinfo', '100644',
                     self.git('hash-object', '-w', '--stdin').decode().strip(), 'linked/credential.txt')
            self.assertTrue(any('符号链接' in reason for _, _, reason in guard.check(self.root)[2]))

    def test_symlink_cannot_expose_ignored_private_data(self):
        self.write('new-board/.local/pairing.json', b'synthetic fixture\n')
        (self.root / 'public.txt').symlink_to('new-board/.local/pairing.json')
        self.assertTrue(any('禁止上传的本机路径' in reason for _, _, reason in guard.check(self.root)[2]))


if __name__ == '__main__':
    unittest.main()
