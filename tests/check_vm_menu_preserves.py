"""Accept an unchanged live monitor menu while preserving its installed patches."""
import os
from pathlib import Path
import pty
import select
import signal
import subprocess
import time

assert os.environ.get('MT_STACK_VM_TEST') == '1', 'Run only inside the disposable test VM'
root = Path(__file__).resolve().parents[1]
installed = Path.home()/'.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n'
before = {str(path.relative_to(installed)): path.read_bytes() for path in installed.rglob('*') if path.is_file()}
assert b'Process popup: top resource users per metric' in before['extension.js'], 'Install default monitor components first'
master, slave = pty.openpty()
process = subprocess.Popen(['bash', str(root/'install.sh')], stdin=slave, stdout=slave,
                           stderr=slave, env=dict(os.environ, TERM='xterm-256color'), start_new_session=True)
os.close(slave)
data = b''
accepted = discarded = False
deadline = time.monotonic()+45
try:
    while process.poll() is None and time.monotonic()<deadline:
        if select.select([master], [], [], 0.2)[0]:
            try:
                data += os.read(master, 65536)
            except OSError:
                break
        if not accepted and b'[enter] apply' in data:
            os.write(master, b'\n')
            accepted = True
        if not discarded and b'Choice [1/2/3]' in data:
            os.write(master, b'3\n')
            discarded = True
    process.wait(timeout=5)
    assert accepted and process.returncode == 0, (process.returncode, data.decode(errors='replace'))
    after = {str(path.relative_to(installed)): path.read_bytes() for path in installed.rglob('*') if path.is_file()}
    assert after == before, 'Accepting unchanged installed components replaced their patched source'
finally:
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        process.wait(timeout=10)
    os.close(master)
print('PASS: accepting unchanged monitor menu preserves every installed extension file')
