"""Cancel the public bundled component menu without errors or installation effects."""
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory() as directory:
    home = Path(directory)
    tools = home/'bin'
    tools.mkdir()
    getent = tools/'getent'
    getent.write_text('#!/bin/bash\nprintf "%s:x:%s:%s::%s:/bin/bash\\n" "$USER" "$(id -u)" "$(id -g)" "$TEST_HOME"\n')
    getent.chmod(0o755)
    environment = dict(os.environ, HOME=str(home), TEST_HOME=str(home), TERM='xterm-256color',
                       PATH=str(tools)+':'+os.environ['PATH'],
                       XDG_STATE_HOME=str(home/'state'), XDG_CACHE_HOME=str(home/'cache'))
    for key in (b'q', b'\x1b'):
        master, slave = pty.openpty()
        process = subprocess.Popen(['bash', str(root/'install.sh')], stdin=slave,
                                   stdout=slave, stderr=slave, env=environment, start_new_session=True)
        os.close(slave)
        data = b''
        deadline = time.monotonic()+15
        try:
            while b'Configure components' not in data and process.poll() is None and time.monotonic()<deadline:
                if select.select([master], [], [], 0.2)[0]:
                    try:
                        data += os.read(master, 65536)
                    except OSError:
                        break
            assert b'Configure components' in data, data.decode(errors='replace')
            os.write(master, key)
            process.wait(timeout=10)
            while select.select([master], [], [], 0)[0]:
                try:
                    data += os.read(master, 65536)
                except OSError:
                    break
            assert process.returncode == 0, (key, process.returncode, data.decode(errors='replace'))
            assert not (home/'.local/share/gnome-shell/extensions').exists(), key
            assert not (home/'.agents/skills').exists(), key
            assert not (home/'.claude/skills').exists(), key
            assert not (home/'state/isc/receipts').exists(), key
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
            os.close(master)
print('PASS: q and Escape cancel the public installer successfully without installation')
