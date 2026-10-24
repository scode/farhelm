"""Validate window identity, native-image calibration and the complete gesture proof.

Fictional measurements isolate bad evidence from bad product behavior. The
separate guest Swift probe establishes that actual native measurements run.
"""

import base64
import copy
import unittest

from test_guest_tester import PNG
from window_geometry import calibrate, image_metadata, require_drag_delivery, require_window_trace, select_frame


def observation():
    """Supply one owned window while leaving unrelated windows in the raw native inventory."""
    return {'format': 1, 'display_count': 1, 'display': {}, 'windows': [
        {'pid': 101, 'window_id': 7, 'layer': 0, 'bounds': {'X': 112, 'Y': 51, 'Width': 800, 'Height': 600}},
        {'pid': 202, 'window_id': 8, 'layer': 0, 'bounds': {'X': 0, 'Y': 0, 'Width': 1024, 'Height': 768}}]}


def trace():
    """Model the recipe's moves, two zoom/restores and final stationary control states."""
    rectangles = [(112, 51, 800, 600), (187, 101, 800, 600), (137, 76, 800, 600),
                  (0, 24, 1024, 744), (137, 76, 800, 600), (0, 24, 1024, 744),
                  (137, 76, 800, 600), (137, 76, 800, 600), (137, 76, 800, 600), (137, 76, 800, 600)]
    return [{'target': 'farhelm', 'observation': {
        'frame': dict(window_id=7, pid=101, **dict(zip(('x', 'y', 'width', 'height'), rectangle))),
        'image': {'sequence': number, 'sha256': 'fixture-image'}, 'image_scale': {'x': 2, 'y': 2}}}
        for number, rectangle in enumerate(rectangles, 1)]


class WindowGeometryTests(unittest.TestCase):
    """A plausible image or rectangle is insufficient without the whole same-window trace."""

    def test_native_image_dimensions_use_bytes_not_a_display_scale_guess(self):
        """The native provider's image can have a different scale from the virtual display's reported pixels."""
        metadata = image_metadata({'data': base64.b64encode(PNG).decode()}, 1)
        self.assertEqual((metadata['width'], metadata['height']), (1, 1))
        jpeg = bytes.fromhex('ffd8ffc0000b0804b0064001011100ffd9')
        metadata = image_metadata({'data': base64.b64encode(jpeg).decode()}, 2)
        self.assertEqual((metadata['width'], metadata['height']), (1600, 1200))
        frame = select_frame(observation(), {101})
        self.assertEqual(calibrate(frame, metadata), {'x': 2, 'y': 2})

    def test_corrupt_image_and_incompatible_crop_cannot_calibrate_motion(self):
        """Incomplete headers or a different-shaped crop cannot establish the coordinate mapping for a drag."""
        with self.assertRaisesRegex(RuntimeError, 'dimensions'):
            image_metadata({'data': base64.b64encode(b'not a native image').decode()}, 1)
        frame = select_frame(observation(), {101})
        with self.assertRaisesRegex(RuntimeError, 'aspect ratios'):
            calibrate(frame, {'width': 1600, 'height': 600, 'sequence': 1})
        self.assertIsNone(calibrate(frame, None))

    def test_window_is_pinned_to_both_process_and_window_id(self):
        """A replacement app window must not satisfy a later observation merely because its size matches."""
        value = observation()
        pinned = select_frame(value, {101})
        value['windows'][0]['window_id'] = 99
        with self.assertRaisesRegex(RuntimeError, 'absent or ambiguous'):
            select_frame(value, {101}, pinned)
        value = observation()
        with self.assertRaisesRegex(RuntimeError, 'ambiguous'):
            select_frame(value, {101, 202})
        value['windows'][0]['bounds']['Width'] = 0
        with self.assertRaisesRegex(RuntimeError, 'geometry'):
            select_frame(value, {101})

    def test_complete_trace_and_each_distinct_failure_boundary(self):
        """Moves, size preservation, both restores and stationary controls are independent acceptance obligations."""
        require_window_trace(trace())
        failures = [('x', 1, 112), ('width', 2, 801), ('width', 3, 100),
                    ('x', 4, 150), ('height', 5, 100), ('y', 6, 80), ('x', 8, 140), ('pid', 9, 999)]
        for key, index, value in failures:
            with self.subTest(key=key, index=index):
                rows = trace()
                rows[index]['observation']['frame'][key] = value
                if index == 1 and key == 'x':
                    rows[index]['observation']['frame']['y'] = 51
                with self.assertRaises(RuntimeError):
                    require_window_trace(rows)
        with self.assertRaisesRegex(RuntimeError, 'ten native'):
            require_window_trace(trace()[:-1])

    def test_control_motion_cannot_be_hidden_by_a_later_restoration(self):
        """Every frame-only control observation matters, even when the next screenshot shows the prior frame again."""
        rows = trace()
        moved = copy.deepcopy(rows[6])
        moved['observation']['frame']['x'] += 10
        rows.insert(7, moved)
        with self.assertRaisesRegex(RuntimeError, 'control interaction'):
            require_window_trace(rows)
        moved['observation']['image'] = None
        with self.assertRaisesRegex(RuntimeError, 'control interaction'):
            require_window_trace(rows)

    def test_bonus_readiness_requires_an_independent_same_window_move(self):
        """Working clicks do not imply that held input can move a window; reject stationary and replaced oracles."""
        rows = trace()[:2]
        for row in rows:
            row['target'] = 'finder'
        require_drag_delivery(rows)
        rows[1]['observation']['frame']['window_id'] += 1
        with self.assertRaisesRegex(RuntimeError, 'actual window-drag'):
            require_drag_delivery(rows)
        rows = [dict(row, target='finder') for row in trace()[7:]]
        with self.assertRaisesRegex(RuntimeError, 'actual window-drag'):
            require_drag_delivery(rows)


if __name__ == '__main__':
    unittest.main()
