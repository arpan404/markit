#!/usr/bin/env python3
"""Run the opt-in release demo harness after building/installing the app."""
import argparse
import json
import os
from pathlib import Path
import statistics
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('platform', choices=['ios', 'android'])
parser.add_argument('--device', required=True, help='simctl UUID or adb serial')
parser.add_argument('--out', required=True)
parser.add_argument('--fixtures', type=Path, help='Private exported corpus, copied into app data only')
args = parser.parse_args()

def command(*parts):
    return subprocess.check_output(parts, text=True).strip()

if args.platform == 'ios':
    container = Path(command('xcrun', 'simctl', 'get_app_container', args.device, 'dev.markit.demo', 'data'))
    result = container / 'Documents/benchmark.json'
    result.unlink(missing_ok=True)
    launch = ['xcrun', 'simctl', 'launch', '--terminate-running-process', args.device, 'dev.markit.demo', '--benchmark']
    if args.fixtures:
        shutil.copyfile(args.fixtures, container / 'Documents/benchmark-fixtures.json')
        launch.append('--private-benchmark')
    command(*launch)
    deadline = time.monotonic() + 60
    while not result.exists():
        if time.monotonic() > deadline:
            raise RuntimeError('iOS benchmark did not finish; inspect app logs/crash reports')
        time.sleep(0.25)
    data = json.loads(result.read_text())
else:
    adb = str(Path(os.environ.get('ANDROID_HOME', str(Path.home() / 'Library/Android/sdk'))) / 'platform-tools/adb')
    def android(*parts):
        return command(adb, '-s', args.device, *parts)
    android('shell', 'am', 'force-stop', 'dev.markit.demo')
    launch = ['shell', 'am', 'start', '-n', 'dev.markit.demo/.MainActivity', '--ez', 'benchmark', 'true']
    if args.fixtures:
        destination = '/sdcard/Android/data/dev.markit.demo/files'
        android('shell', 'mkdir', '-p', destination)
        android('push', str(args.fixtures), destination + '/benchmark-fixtures.json')
        launch += ['--ez', 'privateBenchmark', 'true']
    android(*launch)
    launch_deadline = time.monotonic() + 10
    while True:
        try:
            pid = android('shell', 'pidof', 'dev.markit.demo')
            break
        except subprocess.CalledProcessError:
            if time.monotonic() > launch_deadline:
                raise RuntimeError('Android app did not start')
            time.sleep(0.1)
    deadline = time.monotonic() + 60
    while True:
        logs = android('logcat', '-d', '--pid=' + pid, '-v', 'raw', '-s', 'MARKIT_BENCH:I')
        if 'DONE' in logs:
            break
        if time.monotonic() > deadline:
            raise RuntimeError('Android benchmark did not finish; inspect AndroidRuntime logs')
        time.sleep(0.25)
    cases = {}
    for line in logs.splitlines():
        if line.startswith('{'):
            item = json.loads(line)
            cases[item['case']] = item['result']
    expected = set(json.loads(args.fixtures.read_text())) if args.fixtures else {'code1000','code5000','message','transcript50'}
    if set(cases) != expected:
        raise RuntimeError('Missing benchmark fixtures in logcat')
    data = {'platform': 'Android emulator', 'os': android('shell', 'getprop', 'ro.build.version.release'), 'screen': android('shell', 'wm', 'size'), 'density': android('shell', 'wm', 'density'), 'cases': cases}

data['device'] = args.device
data['commit'] = command('git', 'rev-parse', 'HEAD')
data['workingTreeDirty'] = bool(command('git', 'status', '--porcelain'))
data['build'] = 'Release; Rust release; simulator/emulator on host CPU'
data['method'] = 'First observation retained separately; median of remaining 9; synchronous bridge and viewport work, excludes GPU presentation'
Path(args.out).write_text(json.dumps(data, indent=2) + '\n')
for name, values in data['cases'].items():
    metrics = ['bridgeMs', 'viewportLayoutMs', 'scrollToEndMs', 'codeScrollRoundTripsMs', 'tableScrollRoundTripsMs']
    print(name, {metric: round(statistics.median(values[metric][1:]), 3) for metric in metrics if values.get(metric)})
