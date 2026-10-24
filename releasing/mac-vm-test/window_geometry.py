"""Bind native window measurements to actual CUA images, rather than guessing motion.

The provider crops images to the app and may use a different pixel scale from
the window server. Frame identity and image calibration therefore accompany
every observation; display pixel counts alone cannot supply that calibration.
"""

import base64
import hashlib
import math
import struct


def image_metadata(value, sequence):
    """Read dimensions from genuine native PNG/JPEG bytes without rendering or editing them."""
    if not isinstance(value.get('data'), str) or len(value['data']) > 24 * 1024 * 1024:
        raise RuntimeError('native image exceeds the evidence bound')
    data = base64.b64decode(value['data'], validate=True)
    width = height = None
    if data.startswith(b'\x89PNG\r\n\x1a\n') and len(data) >= 24 and data[12:16] == b'IHDR':
        width, height = struct.unpack('>II', data[16:24])
    elif data.startswith(b'\xff\xd8'):
        offset = 2
        while offset + 3 < len(data):
            if data[offset] != 0xff:
                break
            while offset < len(data) and data[offset] == 0xff:
                offset += 1
            if offset >= len(data):
                break
            marker = data[offset]
            offset += 1
            if marker in (0xd9, 0xda):
                break
            if marker == 0x01 or 0xd0 <= marker <= 0xd7:
                continue
            length = int.from_bytes(data[offset:offset + 2], 'big')
            if length < 2 or offset + length > len(data):
                break
            if marker in (0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf) and length >= 7:
                height, width = struct.unpack('>HH', data[offset + 3:offset + 7])
                break
            offset += length
    if width is None or not 0 < width <= 20000 or not 0 < height <= 20000:
        raise RuntimeError('native image has no supported bounded PNG/JPEG dimensions')
    return {'width': width, 'height': height, 'sha256': hashlib.sha256(data).hexdigest(), 'sequence': sequence}


def select_frame(value, process_ids, pinned=None):
    """Refuse ambiguous or replaced windows instead of measuring whichever one happens to appear first."""
    if value.get('format') != 1 or value.get('display_count') != 1:
        raise RuntimeError('window measurements require the single-display guest fixture')
    windows = [row for row in value['windows'] if row['pid'] in process_ids and row['layer'] == 0]
    if pinned is not None:
        windows = [row for row in windows if (row['window_id'], row['pid']) == (pinned['window_id'], pinned['pid'])]
    if len(windows) != 1:
        raise RuntimeError('the original owned native window is absent or ambiguous')
    window = windows[0]
    if type(window['window_id']) is not int or window['window_id'] <= 0:
        raise RuntimeError('native window identity is invalid')
    bounds = window['bounds']
    coordinates = [bounds[key] for key in ('X', 'Y', 'Width', 'Height')]
    if (any(type(number) not in (int, float) or not math.isfinite(number) for number in coordinates)
            or coordinates[2] <= 0 or coordinates[3] <= 0):
        raise RuntimeError('native window bounds are not finite positive geometry')
    return {'window_id': window['window_id'], 'pid': window['pid'],
            **dict(zip(('x', 'y', 'width', 'height'), coordinates))}


def calibrate(frame, image, previous=None):
    """Use the actual app image scale; a virtual display's reported pixels can describe a different scale.

A new image is paired with the current measured frame. Before the first image,
uncalibrated bounds still establish room to move without taking an extra image.
Between images, retain
that mapping: a frame query after a gesture is still useful before the next
screenshot. Neither a resize nor a drag is inferred from image dimensions.
"""
    if image is None:
        return None
    if previous is not None and previous['image'] is not None and previous['image']['sequence'] == image['sequence']:
        return previous['image_scale']
    scale = {'x': image['width'] / frame['width'], 'y': image['height'] / frame['height']}
    if not math.isclose(scale['x'], scale['y'], rel_tol=0.01):
        raise RuntimeError('native image and measured window have inconsistent aspect ratios')
    return scale


def require_window_trace(history):
    """A passing title-bar test needs measured moves, zoom/restores and stationary controls.

Ten actual image observations define the recipe's S1..S10 states. Additional
frame-only observations after control gestures must also remain stationary.
The native event stream separately proves the gestures and image provenance.
"""
    owned = [row['observation'] for row in history if row['target'] == 'farhelm']
    rows = [row for row in owned if row['image'] is not None]
    images = {}
    for row in rows:
        images.setdefault(row['image']['sequence'], row)
    if len(images) != 10:
        raise RuntimeError('window trace needs all ten native acceptance image states')
    states = [row['frame'] for row in images.values()]
    identity = {(row['frame']['window_id'], row['frame']['pid']) for row in owned}
    if len(identity) != 1:
        raise RuntimeError('window trace changed its owned process/window identity')
    def same(left, right, keys):
        """Compare window-server coordinates with only floating-point representation tolerance."""
        return all(math.isclose(left[key], right[key], abs_tol=0.000001, rel_tol=0) for key in keys)
    rectangle = ('x', 'y', 'width', 'height')
    for before, after in ((states[0], states[1]), (states[1], states[2])):
        if same(before, after, ('x', 'y')) or not same(before, after, ('width', 'height')):
            raise RuntimeError('title-bar drag did not move the same-sized window')
    for before, zoomed, restored in ((states[2], states[3], states[4]), (states[4], states[5], states[6])):
        if zoomed['width'] * zoomed['height'] <= before['width'] * before['height'] or not same(before, restored, rectangle):
            raise RuntimeError('title-bar zoom did not enlarge and restore the prior frame')
    baseline = states[6]
    control_sequence = list(images)[6]
    control_start = next(index for index, row in enumerate(owned)
                         if row['image'] is not None and row['image']['sequence'] == control_sequence)
    if any(not same(row['frame'], baseline, rectangle) for row in owned[control_start:]):
        raise RuntimeError('a control interaction moved or resized the window')


def require_drag_delivery(history):
    """Prove the bonus fixture can move a native window before investing in product setup.

Finder's ordinary window is the independent input oracle. Successful native
tool calls can leave it stationary, so readiness needs an observed same-size
move on one process/window, with genuine image-associated measurements.
"""
    rows = [row['observation'] for row in history if row['target'] == 'finder'
            and row['observation']['image'] is not None]
    for before, after in zip(rows, rows[1:]):
        first, last = before['frame'], after['frame']
        if ((first['window_id'], first['pid']) == (last['window_id'], last['pid'])
                and (first['width'], first['height']) == (last['width'], last['height'])
                and (first['x'], first['y']) != (last['x'], last['y'])
                and before['image']['sequence'] != after['image']['sequence']):
            return
    raise RuntimeError('bonus native readiness did not establish actual window-drag delivery')
