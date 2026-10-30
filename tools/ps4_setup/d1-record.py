"""Record the shadPS4 game window with the game's own audio (56578 evidence clips).

    d1-record.py --out clip.mp4 [--seconds 60] [--stop-file PATH] [--tap DIR]

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
    a = ap.parse_args()
    if not a.tap:
        a.tap = str(d1paths.tools() / 'shadPS4' / 'audio-tap')
    os.makedirs(a.tap, exist_ok=True)
    for f in glob.glob(os.path.join(a.tap, 'port*.f32')):
        os.remove(f)
    video = a.out + '.video.mp4'
    flag = os.path.join(a.tap, 'rec.flag')
    open(flag, 'w').close()
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
        os.remove(flag)
        time.sleep(0.5)
    ports = sorted(glob.glob(os.path.join(a.tap, 'port*.f32')))
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
