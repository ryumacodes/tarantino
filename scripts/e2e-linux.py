#!/usr/bin/env python3
"""Real WebKit/Tauri editor E2E tests. Requires a running tauri-driver and Pillow."""
import argparse
import base64
import io
import json
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', required=True)
    parser.add_argument('--fixture', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--webdriver', default='http://127.0.0.1:4444')
    args = parser.parse_args()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    session = None

    def request(method, path, payload=None):
        prefix = f'/session/{session}' if session else ''
        data = None if payload is None else json.dumps(payload).encode()
        req = urllib.request.Request(args.webdriver + prefix + path, data=data,
                                     method=method, headers={'Content-Type': 'application/json'})
        try:
            with urllib.request.urlopen(req, timeout=65) as response:
                result = json.load(response)['value']
        except urllib.error.HTTPError as error:
            raise RuntimeError(error.read().decode()) from error
        if isinstance(result, dict) and isinstance(result.get('error'), str):
            raise RuntimeError(result)
        return result

    def js(source):
        return request('POST', '/execute/sync', {'script': source, 'args': []})

    def wait(check, label, timeout=30):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            result = check()
            if result:
                return result
            time.sleep(0.15)
        raise AssertionError(f'Timed out: {label}')

    def click(selector):
        element = request('POST', '/element', {'using': 'css selector', 'value': selector})
        identifier = element['element-6066-11e4-a52e-4f735466cecf']
        request('POST', f'/element/{identifier}/click', {})

    def screenshot(name):
        data = base64.b64decode(request('GET', '/screenshot'))
        (output / f'{name}.png').write_bytes(data)
        img = Image.open(io.BytesIO(data)).convert('RGB')
        rect = js('const r=document.querySelector(".video-canvas-frame").getBoundingClientRect();'
                  'return {x:r.x,y:r.y,width:r.width,height:r.height,viewport:innerWidth};')
        scale = img.width / rect['viewport']
        return img.crop(tuple(round(v * scale) for v in (
            rect['x'], rect['y'], rect['x'] + rect['width'], rect['y'] + rect['height'])))

    def bars(image):
        # The native fixture uses SMPTE color bars. Zooming visibly reduces the
        # number of bars across its top row; a frozen black canvas cannot pass.
        strip = image.resize((100, 100))
        sequence = []
        for x in range(3, 97):
            pixel = strip.getpixel((x, 20))
            high = max(pixel)
            color = sum(1 << i for i, channel in enumerate(pixel)
                        if high > 40 and channel > high * 0.6)
            if not sequence or sequence[-1] != color:
                sequence.append(color)
        return sequence

    try:
        created = request('POST', '/session', {'capabilities': {'alwaysMatch': {
            'tauri:options': {'application': str(Path(args.app).resolve())}}}})
        session = created['sessionId']
        request('POST', '/window/rect', {'width': 1280, 'height': 840, 'x': 30, 'y': 30})
        origin = urllib.parse.urlsplit(request('GET', '/url'))
        editor_url = f'{origin.scheme}://{origin.netloc}/editor.html?media=' + urllib.parse.quote(
            str(Path(args.fixture).resolve()), safe='')
        request('POST', '/url', {'url': editor_url})
        wait(lambda: js('return document.querySelectorAll(".video-thumbnail").length >= 5 '
                        '&& [...document.querySelectorAll(".video-thumbnail")].every(i=>i.naturalWidth>0) '
                        '&& document.querySelectorAll(".zoom-block-container").length>0;'),
             'decoded timeline thumbnails and generated zoom blocks')
        initial = wait(lambda: visible_frame(screenshot('initial')), 'visible preview pixels')
        initial_bars = bars(initial)
        assert len(initial_bars) >= 5, f'Fixture color bars missing: {initial_bars}'

        click('button[title="Play"]')
        wait(lambda: js('return document.querySelector(".time-display .current-time").textContent==="0:02";'),
             'playback reaches the automatic zoom')
        click('button[title="Pause"]')
        zoomed = screenshot('zoomed')
        zoomed_bars = bars(zoomed)
        assert len(zoomed_bars) < len(initial_bars), f'Zoom not visible: {initial_bars} -> {zoomed_bars}'

        # Remove the zoom through the UI before measuring frame movement, so a
        # changing zoom transform cannot hide a frozen source frame.
        click('.zoom-block-container')
        request('POST', '/actions', {'actions': [{'type': 'key', 'id': 'keyboard', 'actions': [
            {'type': 'keyDown', 'value': '\ue017'}, {'type': 'keyUp', 'value': '\ue017'}]}]})
        wait(lambda: js('return document.querySelectorAll(".zoom-block-container").length===0;'),
             'delete selected zoom block')
        click('button[title="Go to Start"]')
        time.sleep(0.8)  # Let the spring settle and the first frame arrive.
        unzoomed = screenshot('unzoomed-start')
        click('button[title="Play"]')
        wait(lambda: js('return document.querySelector(".time-display .current-time").textContent==="0:02";'),
             'unzoomed playback advances')
        advanced = screenshot('unzoomed-advanced')
        click('button[title="Pause"]')
        # SMPTE's changing noise is in the lower-right area, away from cursor effects.
        crop = (int(advanced.width * .72), int(advanced.height * .82),
                int(advanced.width * .94), int(advanced.height * .96))
        difference = ImageStat.Stat(ImageChops.difference(unzoomed.crop(crop), advanced.crop(crop)))
        movement = sum(difference.mean) / 3
        assert movement > 2, f'Playback clock advanced but source frames froze: pixel difference={movement}'

        paused_frame = screenshot('before-end-seek')
        click('button[title="Go to End"]')
        wait(lambda: js('return document.querySelector(".time-display .current-time").textContent==="0:06";'),
             'seek to recording end')
        def final_frame_changed():
            end = screenshot('last-frame')
            diff = ImageStat.Stat(ImageChops.difference(paused_frame.crop(crop), end.crop(crop)))
            return sum(diff.mean) / 3 > 2
        wait(final_frame_changed, 'last decodable frame after seeking to end')

        # Loading the same finalized recording again must restore the saved zoom
        # and decoded thumbnails, without relying on stale editor state.
        request('POST', '/url', {'url': editor_url})
        wait(lambda: js('return document.querySelectorAll(".zoom-block-container").length>0 '
                        '&& document.querySelectorAll(".video-thumbnail").length>=5 '
                        '&& [...document.querySelectorAll(".video-thumbnail")].every(i=>i.naturalWidth>0);'),
             'reopen finalized recording')
        wait(lambda: visible_frame(screenshot('reopened')), 'preview after reopening')
        click('button[title="Cursor - Cursor style & behavior"]')
        wait(lambda: js('const panel=document.querySelector(".cursor-settings-panel"); '
                        'const row=document.querySelector(".toggle-row"); '
                        'return panel && row && getComputedStyle(panel).display==="flex" '
                        '&& getComputedStyle(row).display==="flex";'),
             'packaged cursor panel styles loaded')
        screenshot('cursor-panel')
        report = {'passed': True, 'native_webview': created['capabilities']['browserName'],
                  'initial_bars': initial_bars, 'zoomed_bars': zoomed_bars,
                  'frame_pixel_difference': movement,
                  'checks': ['native IPC and generated sidecars', 'decoded timeline thumbnails',
                             'non-black preview pixels', 'visible automatic zoom',
                             'moving source frames during playback', 'last frame after end seek', 'recording reopened',
                             'packaged cursor panel styles loaded']}
        (output / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    except Exception:
        if session:
            try:
                (output / 'failure.png').write_bytes(base64.b64decode(request('GET', '/screenshot')))
                (output / 'failure.html').write_text(request('GET', '/source'))
            except Exception:
                pass
        raise
    finally:
        if session:
            request('DELETE', '')


def visible_frame(image):
    sample = image.resize((64, 64))
    colorful = sum(max(pixel) - min(pixel) > 40 and max(pixel) > 70 for pixel in sample.getdata())
    return image if colorful > 800 else None


if __name__ == '__main__':
    main()
