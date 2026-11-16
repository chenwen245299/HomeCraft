import base64
import importlib.util
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).parents[1]
sys.path.insert(0, str(ROOT/'tools'))
import authorize_device
import configure_device


class ProvisioningTests(unittest.TestCase):
    def auth(self):
        claims = base64.urlsafe_b64encode(json.dumps({'client_id':'fixture-public-client'}).encode()).decode().rstrip('=')
        return {'tokens':{'access_token':'header.'+claims+'.signature','refresh_token':'fixture-refresh',
                          'account_id':'fixture-account','id_token':'must-not-export'}}

    def test_independent_authorization_exports_only_required_fields(self):
        result = authorize_device.export_codex(self.auth())['codex']
        self.assertEqual(set(result), {'access_token','refresh_token','account_id','client_id'})
        self.assertEqual(result['client_id'],'fixture-public-client')
        self.assertNotIn('id_token', result)

    def test_desktop_import_never_copies_rotating_refresh_token(self):
        result = configure_device.current_codex_access(self.auth())
        self.assertNotIn('refresh_token', result)
        self.assertNotIn('id_token', result)
        self.assertEqual(result['account_id'],'fixture-account')

    def test_incomplete_login_fails(self):
        with self.assertRaises(ValueError):
            authorize_device.export_codex({'tokens':{}})


if __name__ == '__main__':
    unittest.main()
