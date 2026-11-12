"""Record the shadPS4 game window with the game's own audio (56578 evidence clips).

    d1-record.py --out clip.mp4 [--seconds 60] [--stop-file PATH] [--tap DIR]
                 [--wait-log shad_log.txt [--wait-lines 200] [--wait-since EPOCH] [--wait-timeout 600]]

With --wait-log the recorder first waits for the emulator to have booted (the log exists with
at least --wait-lines lines, last written at or after --wait-since, and a shadPS4 window exists); it exits 3 if that does not happen
within --wait-timeout seconds or if --stop-file appears first. A stale port*.f32 held open by
a running emulator is skipped, not fatal, and left out of the mux; rec.flag is removed on
every exit path.

Video: Windows Graphics Capture of the emulator window's client area (no title bar, no cursor,
no desktop), encoded once with NVENC at the window's own size; nothing is upscaled. Audio: the
emulator's D1_AUDIO_TAP (sdl_audio_out.cpp in the D1 shadPS4 patch) writes each audio port's
float samples to DIR while DIR/rec.flag exists, so the clip carries game audio only (no
microphone, no other desktop sound). Each port file is named with the wall-clock time of its
first sample; the ports are delayed or trimmed against the first video frame and mixed to
stereo. Recording ends after --seconds, or earlier when --stop-file appears.
"""
import argparse, glob, json, os, re, shutil, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import d1paths  # sibling: local paths from D1_ROOT or paths.json

FFMPEG = os.environ.get('D1_FFMPEG') or shutil.which('ffmpeg') or os.path.expandvars(
    r'%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe'
    r'\ffmpeg-9.0.2-full_build\bin\ffmpeg.exe')
FFPROBE = os.path.join(os.path.dirname(FFMPEG), 'ffprobe.exe' if FFMPEG.endswith('.exe') else 'ffprobe')
LAYOUT = {1: 'mono', 2: 'stereo', 6: '5.1', 8: '7.1'}


def cleanup_stale_taps(tap):
    """Delete old port*.f32 dumps. Returns (removed, skipped); a file in use is skipped."""
    removed, skipped = [], []
    for f in sorted(glob.glob(os.path.join(tap, 'port*.f32'))):
        try:
            os.remove(f)
            removed.append(f)
        except OSError:
            skipped.append(f)
    return removed, skipped


_warned = set()


def count_lines(path, stop_at=None):
    """Newline count of a file (stops reading once stop_at is reached); 0 if missing/unreadable."""
    n = 0
    try:
        with open(path, 'rb') as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b''):
                n += chunk.count(b'\n')
                if stop_at is not None and n >= stop_at:
                    break
    except FileNotFoundError:
        return 0
    except OSError as e:
        if path not in _warned:
            _warned.add(path)
            print(f'cannot read {path}: {e}')
        return 0
    return n


def log_lines_since(path, since, stop_at=None):
    """Lines of a log that belongs to this run: 0 while its mtime is older than `since`."""
    try:
        if os.path.getmtime(path) < since:
            return 0
    except OSError:
        return 0
    return count_lines(path, stop_at)


def boot_ready(line_count, wanted, window_present=True):
    """The emulator has booted: enough log lines and a window to capture."""
    return line_count >= wanted and bool(window_present)


def shadps4_window_present():
    """True when a visible top-level window of shadPS4.exe exists (Windows only)."""
    if os.name != 'nt':
        return False
    import ctypes
    from ctypes import wintypes
    u, k = ctypes.windll.user32, ctypes.windll.kernel32
    k.OpenProcess.restype = wintypes.HANDLE
    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def each(hwnd, _):
        if not u.IsWindowVisible(hwnd) or u.GetWindowTextLengthW(hwnd) == 0:
            return True
        pid = wintypes.DWORD()
        u.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        h = k.OpenProcess(0x1000, False, pid.value)  # PROCESS_QUERY_LIMITED_INFORMATION
        if h:
            buf = ctypes.create_unicode_buffer(1024)
            n = wintypes.DWORD(1024)
            if k.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n)) and \
                    os.path.basename(buf.value).lower() == 'shadps4.exe':
                found.append(hwnd)
            k.CloseHandle(h)
        return not found
    u.EnumWindows(each, 0)
    return bool(found)


def wait_for_boot(line_count_fn, window_fn, wanted, timeout, stop_file=None,
                  sleep=time.sleep, clock=time.time, poll=1.0):
    """Wait until boot_ready. Returns 'ready', 'timeout' or 'stop' (stop file wins over ready)."""
    deadline = clock() + timeout
    while True:
        if stop_file and os.path.exists(stop_file):
            return 'stop'
        n = line_count_fn()
        if boot_ready(n, wanted, window_fn() if n >= wanted else False):
            return 'ready'
        if clock() >= deadline:
            return 'timeout'
        sleep(poll)


def probe(path):
    out = subprocess.run([FFPROBE, '-v', 'error', '-show_entries',
                          'stream=codec_type,width,height,avg_frame_rate,nb_frames:format=duration',
                          '-of', 'json', path], capture_output=True, text=True).stdout
    return json.loads(out or '{}')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--seconds', type=float, default=60)
    ap.add_argument('--stop-file')
    ap.add_argument('--tap', default=os.environ.get('D1_AUDIO_TAP'),
                    help='audio tap folder (default: D1_AUDIO_TAP, else D1_TOOLS/shadPS4/audio-tap)')
    ap.add_argument('--fps', default='60')
    ap.add_argument('--wait-log', help='emulator log; wait for it before recording (exit 3 on give-up)')
    # 200 is a heuristic, not a measured boot marker: a booted emulator passes it within seconds,
    # a hung one stays under about 60 lines (seen 12 November 2026).
    ap.add_argument('--wait-lines', type=int, default=200, help='lines the log must have (default 200)')
    ap.add_argument('--wait-since', type=float, default=None,
                    help='epoch seconds; a log last written before this is a previous run (default: now)')
    ap.add_argument('--wait-timeout', type=float, default=600, help='seconds to wait (default 600)')
    a = ap.parse_args()
    if a.wait_since is None:
        a.wait_since = time.time()
    if a.wait_log:
        why = wait_for_boot(lambda: log_lines_since(a.wait_log, a.wait_since, a.wait_lines), shadps4_window_present, a.wait_lines,
                            a.wait_timeout, a.stop_file)
        if why != 'ready':
            print(f'emulator not booted ({why}): {a.wait_log} needs {a.wait_lines} lines and a shadPS4 window')
            sys.exit(3)
    if not a.tap:
        a.tap = str(d1paths.tools() / 'shadPS4' / 'audio-tap')
    os.makedirs(a.tap, exist_ok=True)
    _, stale = cleanup_stale_taps(a.tap)
    if stale:
        print(f'{len(stale)} stale tap file(s) in use, skipped and not muxed')
    video = a.out + '.video.mp4'
    flag = os.path.join(a.tap, 'rec.flag')
    open(flag, 'w').close()
    try:
        record(a, video, set(stale))
    finally:
        try:
            os.remove(flag)
        except OSError:
            pass


def record(a, video, stale):
    cmd = [FFMPEG, '-hide_banner', '-loglevel', 'error', '-y', '-filter_complex',
           f'gfxcapture=window_exe=(?i)shadPS4\\.exe:capture_cursor=0:max_framerate={a.fps},'
           'hwdownload,format=bgra,format=yuv420p',
           '-c:v', 'h264_nvenc', '-preset', 'p5', '-cq', '18', '-g', '120',
           '-progress', 'pipe:1', '-stats_period', '0.05', video]
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    t_first = None
    frame = 0
    t_end = time.time() + a.seconds
    try:
        for line in p.stdout:
            if line.startswith('frame='):
                frame = int(line.split('=')[1])
                if t_first is None and frame > 0:
                    # Wall time of frame 0: the progress line's arrival minus the frames already
                    # encoded (one frame-time each at the requested rate).
                    t_first = time.time() - frame / float(a.fps)
            if time.time() > t_end or (a.stop_file and os.path.exists(a.stop_file)):
                break
    finally:
        p.stdin.write('q')
        p.stdin.flush()
        p.wait(timeout=30)
        try:  # stop the tap before muxing; main() removes it again on any other exit path
            os.remove(os.path.join(a.tap, 'rec.flag'))
        except OSError:
            pass
        time.sleep(0.5)
    ports = [f for f in sorted(glob.glob(os.path.join(a.tap, 'port*.f32'))) if f not in stale]
    inputs, chains = ['-i', video], []
    for f in ports:
        m = re.search(r'port(\d+)-(\d+)-(\d+)-(\d+)\.f32$', f)
        rate, ch, ms = int(m.group(2)), int(m.group(3)), int(m.group(4))
        if os.path.getsize(f) == 0:
            continue
        inputs += ['-f', 'f32le', '-ar', str(rate), '-ch_layout', LAYOUT.get(ch, f'{ch}c'), '-i', f]
        off = ms / 1000.0 - (t_first or 0)
        shift = f'adelay={int(off * 1000)}:all=1' if off > 0 else f'atrim=start={-off:.3f},asetpts=PTS-STARTPTS'
        n = len(chains) + 1
        chains.append(f'[{n}:a]{shift},aformat=sample_rates=48000:channel_layouts=stereo[a{n}]')
    if chains:
        mix = ''.join(f'[a{k}]' for k in range(1, len(chains) + 1))
        fc = ';'.join(chains) + f';{mix}amix=inputs={len(chains)}:normalize=0:duration=longest[aout]'
        subprocess.run([FFMPEG, '-hide_banner', '-loglevel', 'error', '-y', *inputs, '-filter_complex', fc,
                        '-map', '0:v', '-map', '[aout]', '-c:v', 'copy', '-c:a', 'aac', '-b:a', '192k',
                        '-shortest', '-movflags', '+faststart', a.out], check=True)
        os.remove(video)
    else:
        print('no game audio captured (is D1_AUDIO_TAP set for the emulator?): video only')
        os.replace(video, a.out)
    info = probe(a.out)
    print(json.dumps({'out': a.out, 'audio_ports': len(chains), 'probe': info}))


if __name__ == '__main__':
    main()
