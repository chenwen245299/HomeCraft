import sys
from pathlib import Path
import unittest
import hashlib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
try:
    from ble_bridge import codex_packet,encode_packet
except ImportError:
    codex_packet=None

@unittest.skipIf(codex_packet is None,'BLE dependencies require .ble-venv')
class BlePacketTests(unittest.TestCase):
    def test_srp_preserves_fixed_width_digest_and_salt(self):
        from security.srp6a import calculate_x
        username,password='wifiprov','fixture-144'
        salt=b'\x00'+bytes(range(1,16))
        inner=hashlib.sha512((username+':'+password).encode()).digest()
        self.assertEqual(inner[0],0)
        expected=int.from_bytes(hashlib.sha512(salt+inner).digest(),'big')
        self.assertEqual(calculate_x(hashlib.sha512,salt,username,password),expected)
    def test_credentials_never_enter_snapshot(self):
        packet=codex_packet({'access_token':'do-not-send','account_id':'private','rateLimits':{'primary':{'usedPercent':12,'windowDurationMins':300,'resetsAt':1792498000},'secondary':{'usedPercent':43,'windowDurationMins':10080,'resetsAt':1792890000}}})
        wire=encode_packet(packet)
        self.assertNotIn(b'do-not-send',wire);self.assertNotIn(b'private',wire)
        self.assertEqual(packet['w'][0][0],880);self.assertEqual(packet['w'][1][0],570)
        self.assertEqual(packet['w'][2],[-1,0,0]);self.assertLess(len(wire),480)
    def test_sensitive_transport_field_rejected(self):
        with self.assertRaises(ValueError):encode_packet({'v':1,'access_token':'do-not-send'})
    def test_oversized_packet_rejected(self):
        with self.assertRaises(ValueError):encode_packet({'w':['x'*500]})

if __name__=='__main__':unittest.main()
