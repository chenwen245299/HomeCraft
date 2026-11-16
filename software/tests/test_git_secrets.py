"""Real Gitleaks and Git hook checks in disposable repos; no accounts or network."""
from pathlib import Path
import os
import random
import shutil
import string
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(shutil.which('gitleaks'), 'Gitleaks is required for integration checks')
class GitSecretTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='homecraft-git-security-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.env = os.environ.copy()
        # Do not inherit another checkout's index, refs, hooks or user identity.
        for key in list(self.env):
            if key.startswith(('GIT_', 'GITLEAKS_')):
                del self.env[key]
        self.env.update({
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull,
            'GIT_AUTHOR_NAME': 'Offline fixture', 'GIT_COMMITTER_NAME': 'Offline fixture',
            'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
            'GIT_COMMITTER_EMAIL': 'fixture@example.invalid',
        })
        self.git('init', '-q')
        for name in (
            '.gitleaks.toml', '.gitignore',
            'software/tools/check_git_contents.py', 'software/tools/check_secrets.py',
            'software/tools/install_git_hooks.py',
            'software/tools/git-hooks/pre-commit', 'software/tools/git-hooks/pre-push',
        ):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, target)
        self.git('add', '.')
        self.git('commit', '-qm', 'Clean fixture')
        rng = random.Random(42)
        self.secret = ''.join(rng.choices(string.ascii_letters + string.digits, k=40))

    def run_tool(self, *args, input=None):
        return subprocess.run(args, cwd=self.root, env=self.env, input=input,
                              capture_output=True, text=True)

    def git(self, *args):
        result = self.run_tool('git', *args)
        self.assertEqual(result.returncode, 0, 'fixture Git operation failed')
        return result.stdout

    def write(self, name, value):
        file = self.root / name
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(value)
        return file

    def sensitive(self, name='settings.txt'):
        return self.write(name, f'api_key = "{self.secret}"\n')

    def scan(self, *args):
        return self.run_tool('python3', 'software/tools/check_secrets.py', *args)

    def assert_blocked_without_value(self, result):
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(self.secret, result.stdout + result.stderr)

    def install(self):
        return self.run_tool('python3', 'software/tools/install_git_hooks.py')

    def test_clean_repository_and_initial_staging_pass(self):
        self.assertEqual(self.scan().returncode, 0)
        # Also support a brand-new repository's first commit.
        fresh = self.root / 'first-commit'
        fresh.mkdir()
        subprocess.run(['git', 'init', '-q', str(fresh)], check=True, env=self.env)
        shutil.copy2(ROOT / '.gitleaks.toml', fresh / '.gitleaks.toml')
        (fresh / 'readme.txt').write_text('safe fixture\n')
        subprocess.run(['git', '-C', str(fresh), 'add', '.'], check=True, env=self.env)
        result = self.run_tool('gitleaks', 'git', '--pre-commit', '--staged',
                               '--config', str(fresh / '.gitleaks.toml'), '--redact=100',
                               '--no-banner', str(fresh))
        self.assertEqual(result.returncode, 0)

    def test_untracked_source_and_staged_bytes_are_both_scanned(self):
        file = self.sensitive()
        self.assert_blocked_without_value(self.scan())
        self.git('add', 'settings.txt')
        file.write_text('safe working copy\n')
        for args in ((), ('--staged',)):
            result = self.scan(*args)
            self.assert_blocked_without_value(result)
            self.assertIn('generic-api-key', result.stdout)

    def test_deleted_secret_is_still_found_in_history(self):
        file = self.sensitive()
        self.git('add', 'settings.txt')
        self.git('commit', '-qm', 'Synthetic secret fixture')
        file.unlink()
        self.git('add', '-u')
        self.git('commit', '-qm', 'Remove synthetic fixture')
        self.assert_blocked_without_value(self.scan())

    def test_deleted_private_path_is_still_blocked_in_history(self):
        file = self.write('new-board/.local/pairing.json', '{}\n')
        self.git('add', '-f', 'new-board/.local/pairing.json')
        self.git('commit', '-qm', 'Private path fixture without a key')
        file.unlink()
        self.git('add', '-u')
        self.git('commit', '-qm', 'Remove private fixture')
        result = self.scan()
        self.assert_blocked_without_value(result)
        self.assertIn('提交历史', result.stdout)

    def test_force_added_credentials_are_blocked(self):
        self.write('software/auth.json', '{}\n')
        self.git('add', '-f', 'software/auth.json')
        self.assert_blocked_without_value(self.scan('--staged'))

    def test_exception_does_not_exempt_other_values_in_the_same_source(self):
        self.sensitive('software/mac-app/src/registry.rs')
        self.git('add', '.')
        result = self.scan('--staged')
        self.assert_blocked_without_value(result)
        self.assertIn('generic-api-key', result.stdout)

    def test_actual_commit_and_pre_push_hooks_block(self):
        self.assertEqual(self.install().returncode, 0)
        self.sensitive()
        self.git('add', 'settings.txt')
        self.assert_blocked_without_value(self.run_tool('git', 'commit', '-qm', 'Must be blocked'))
        oid = self.git('rev-parse', 'HEAD').strip()
        ref_line = f'refs/heads/main {oid} refs/heads/main {"0" * 40}\n'
        self.assert_blocked_without_value(self.run_tool(
            'software/tools/git-hooks/pre-push', input=ref_line,
        ))
        deletion = f'(delete) {"0" * 40} refs/heads/main {oid}\n'
        self.assertEqual(self.run_tool('software/tools/git-hooks/pre-push', input=deletion).returncode, 0)

    def test_installer_preserves_custom_hook_configuration(self):
        self.git('config', '--local', 'core.hooksPath', 'existing-hooks')
        self.assertNotEqual(self.install().returncode, 0)
        self.assertEqual(self.git('config', '--get', 'core.hooksPath').strip(), 'existing-hooks')

    def test_installer_preserves_existing_default_hook(self):
        file = self.write('.git/hooks/pre-commit', '#!/bin/sh\nexit 0\n')
        self.assertNotEqual(self.install().returncode, 0)
        self.assertEqual(file.read_text(), '#!/bin/sh\nexit 0\n')

    def test_bad_configuration_fails_closed_without_echoing_its_contents(self):
        self.write('.gitleaks.toml', f'invalid = {self.secret}\n')
        self.assert_blocked_without_value(self.scan('--staged'))

    def test_missing_gitleaks_fails_closed(self):
        env = dict(self.env, PATH='')
        result = subprocess.run(
            [sys.executable, 'software/tools/check_secrets.py', '--staged'],
            cwd=self.root, env=env, capture_output=True, text=True,
        )
        self.assert_blocked_without_value(result)
        self.assertIn('缺少 Gitleaks', result.stdout)

    def test_old_gitleaks_cannot_silently_ignore_new_configuration(self):
        binary = self.write('fixture-bin/gitleaks', '#!/bin/sh\necho 8.24.0\n')
        binary.chmod(0o700)
        env = dict(self.env, PATH=str(binary.parent))
        result = subprocess.run(
            [sys.executable, 'software/tools/check_secrets.py', '--staged'],
            cwd=self.root, env=env, capture_output=True, text=True,
        )
        self.assert_blocked_without_value(result)
        self.assertIn('需要 Gitleaks 8.30.1', result.stdout)


if __name__ == '__main__':
    unittest.main()
