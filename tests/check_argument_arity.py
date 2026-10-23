"""Reject malformed monitor invocations before build, downloads or desktop writes."""
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as directory:
    home = Path(directory)
    tools = home/'bin'
    tools.mkdir()
    effects = home/'effects'
    for name, body in {
        'getent': 'printf "%s:x:%s:%s::%s:/bin/bash\\n" "$USER" "$(id -u)" "$(id -g)" "$TEST_HOME"',
        'curl': 'echo download >> "$TEST_EFFECTS"; exit 67',
        'gsettings': 'echo desktop >> "$TEST_EFFECTS"; exit 67',
        'gnome-shell': 'printf "GNOME Shell 46.0\\n"',
        'rm-helper': 'echo helper >> "$TEST_EFFECTS"; exit 67',
    }.items():
        path = tools/name
        path.write_text('#!/bin/bash\n'+body+'\n')
        path.chmod(0o755)
    env = dict(os.environ, HOME=str(home), TEST_HOME=str(home), TEST_EFFECTS=str(effects),
               PATH=str(tools)+':'+os.environ['PATH'], RM_MONITOR_BIN=str(tools/'rm-helper'),
               XDG_STATE_HOME=str(home/'state'), XDG_CACHE_HOME=str(home/'cache'))
    invalid = (['--default', '--unknown'], ['--all', 'unexpected'],
               ['--select', 'rm_vram', '--unknown'], ['--select=rm_vram', 'unexpected'],
               ['--reconfigure', 'rm_vram', 'unexpected'], ['--uninstall', 'rm_vram', 'unexpected'],
               ['--list-components', 'unexpected'], ['--configure-component', 'rm_refresh_interval', 'unexpected'],
               ['--select'], ['--reconfigure'], ['--uninstall'], ['--configure-component'])
    for arguments in invalid:
        result = subprocess.run(['bash', str(root/'install.sh'), *arguments], env=env,
                                text=True, capture_output=True, timeout=15)
        assert result.returncode != 0, (arguments, result.stdout, result.stderr)
        assert not effects.exists(), (arguments, effects.read_text(), result.stdout, result.stderr)
        assert 'argument count' in result.stdout+result.stderr, (arguments, result.stdout, result.stderr)
        assert not (home/'.local/share/gnome-shell/extensions').exists(), arguments
print('PASS: malformed monitor action operands fail before helper, network or desktop effects')
