import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("bridge", Path(__file__).parents[1] / "tools/codex_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class QuotaTests(unittest.TestCase):
    def test_multi_bucket_codex_takes_precedence(self):
        result = {"rateLimits": {"primary": {"usedPercent": 99}},
                  "rateLimitsByLimitId": {"codex": {"primary": {"usedPercent": 42.5}}}}
        q = bridge.normalize_limits(result, 1792496164)
        self.assertEqual(q["primary"]["remaining_tenths"], 575)

    def test_missing_is_unknown_not_full_or_empty(self):
        for window in (None, {}, {"usedPercent": None}, {"usedPercent": float("nan")},
                       {"usedPercent": float("inf")}, {"usedPercent": True}, {"usedPercent": "42"}):
            self.assertEqual(bridge.normalize_window(window)["remaining_tenths"], -1)

    def test_boundary_percentages(self):
        for used, remaining in ((0, 1000), (100, 0), (120, 0), (-2, 1000)):
            self.assertEqual(bridge.normalize_window({"usedPercent": used})["remaining_tenths"], remaining)

    def test_no_codex_bucket_is_an_error(self):
        with self.assertRaises(bridge.BridgeError):
            bridge.normalize_limits({"rateLimitsByLimitId": {"other": {}}})

    def test_unknown_window_and_reset(self):
        self.assertEqual(bridge.normalize_window({"windowDurationMins": None, "resetsAt": None}),
                         {"remaining_tenths": -1, "window_mins": 0, "reset_at": 0})

    def test_wire_protocol(self):
        q = bridge.normalize_limits({"rateLimits": {
            "primary": {"usedPercent": 56, "windowDurationMins": 300, "resetsAt": 1792496653},
            "secondary": None}}, 1792496164)
        self.assertEqual(bridge.encode_snapshot(q), b"EPAPER1 1792496164 440 300 1792496653 -1 0 0 0\n")

    def test_fragmented_device_refresh(self):
        messages = bridge.DeviceMessages()
        self.assertEqual(messages.feed(b'EPAPER_REF'), [])
        self.assertEqual(messages.feed(b'RESH\r\nEPAPER_ACK 1\n'), ['EPAPER_REFRESH', 'EPAPER_ACK 1'])

    def test_oversized_device_line_is_discarded(self):
        messages = bridge.DeviceMessages()
        self.assertEqual(messages.feed(b'x' * 5000 + b'EPAPER_REFRESH\n'), [])
        self.assertEqual(messages.feed(b'EPAPER_REFRESH\n'), ['EPAPER_REFRESH'])


if __name__ == "__main__":
    unittest.main()
