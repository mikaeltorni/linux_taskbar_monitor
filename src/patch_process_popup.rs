//! Replace the left-click "launch the configured action" behavior with an
//! in-panel popup listing the top process users of each panel metric.
//!
//! Upstream always spawns the `leftclickstatus` command on left-click and
//! offers no GSetting to change that, so this source patch:
//!
//! 1. Adds the `PopupMenu` import next to the existing `PanelMenu` import.
//! 2. Injects a marker-guarded block before `_clickManager` holding the click
//!    handling, a background sampler, and the menu renderer. The sampler keeps
//!    a rolling per-process-name history — CPU and RAM and disk IO from
//!    `/proc`, network from `ss -tnpHi`, GPU load and VRAM from
//!    `nvidia-smi pmon` — and each popup section names the processes that used
//!    the most of one metric across the trailing window (default 60 minutes).
//! 3. Rewires the Enter/Space key activation to open the popup and drops the
//!    left-click `_launchPrimaryAction` call from `_clickManager`.
//! 4. Updates the accessibility tooltip to describe the new behavior.
//! 5. Starts the sampler in `_init` and stops it in `destroy`, so history
//!    exists before the first click and no GLib source outlives the extension.
//!
//! The window is baked in by `--window-minutes` and can be overridden live by
//! the `U2TSSM` environment variable (this repository's initials), so
//! `U2TSSM=60` means "rank by the last 60 minutes".
//!
//! `vfunc_event` is the single toggle owner on purpose. `PanelMenu.Button`
//! toggles `this.menu` for every `BUTTON_PRESS` before the `button-press-event`
//! handler (`_clickManager`) runs, so an earlier revision toggled twice per
//! click: the base class opened the already-populated menu and `_clickManager`
//! immediately closed it again. The popup therefore worked exactly once per
//! Shell process and then silently ignored every click. The override toggles
//! once, only for button 1 / touch, and fills the menu *before* opening it
//! because `PopupMenu.open()` returns early on an empty menu.
//!
//! The edits are idempotent (guarded by marker comments) and fail fast when the
//! expected upstream snippets are absent.

use std::fs;
use std::path::Path;

use crate::logging;

/// Marker comment proving the popup methods are already injected. It doubles
/// as the start of the injected block, so the whole body can be re-rendered in
/// place when the patcher changes shape or the window is reconfigured.
const METHODS_START: &str =
    "    // ── Process popup: top resource users per metric over a rolling window ──";

/// Marker text carried inside [`METHODS_START`]. `lib/lifecycle.sh` greps the
/// installed `extension.js` for exactly this string, so the unit test below
/// pins the two together.
#[cfg(test)]
const MARKER: &str = "Process popup: top resource users per metric over a rolling window";

/// Block header written by the first generation of this patcher, which showed
/// a single instantaneous CPU/RAM list. Located so an installed tree upgrades
/// in place instead of gaining a second, duplicated block.
const LEGACY_METHODS_START: &str =
    "    // ── Process popup: total CPU/RAM aggregated per process name ──";

const PANEL_MENU_IMPORT: &str =
    r#"import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";"#;

const POPUP_MENU_IMPORT: &str =
    r#"import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";"#;

const CLICK_MANAGER_ANCHOR: &str = "    _clickManager(actor, event) {";

/// Default rolling window, in minutes, baked into a fresh patch.
pub const DEFAULT_WINDOW_MINUTES: u32 = 60;

/// Largest window the injected JavaScript accepts, mirrored by the `U2TSSM`
/// bounds check inside [`METHODS_TEMPLATE`] so both reject the same values.
pub const MAX_WINDOW_MINUTES: u32 = 1440;

/// Placeholder replaced by the configured window before injection.
const WINDOW_PLACEHOLDER: &str = "__TOP_WINDOW_MINUTES__";

/// Popup methods injected before `_clickManager`. Keep the marker comment in
/// sync with [`MARKER`]; a unit test enforces that.
const METHODS_TEMPLATE: &str = r##"    // ── Process popup: top resource users per metric over a rolling window ──
    // Left-click opens this menu instead of launching the configured action.
    // A background sampler keeps a per-process-name history of CPU, RAM, disk
    // IO, network IO, GPU and VRAM use, and each section lists the processes
    // that consumed the most of one metric across the trailing window.
    //
    // Each row carries two columns. "now" is the live reading, refreshed on
    // the panel's own refresh-time setting for as long as the menu stays
    // open, and answers "what is this using right now?". "avg" is the
    // trailing-window mean the rows are still ranked by, and answers "what
    // has been eating this all hour?". Ranking by the average keeps the row
    // order stable while the live column ticks underneath it.
    //
    // PanelMenu.Button.vfunc_event toggles this.menu on every BUTTON_PRESS,
    // and it runs before the button-press-event handler (_clickManager). With
    // both toggling, the two cancelled out as soon as the menu held rows: the
    // base class opened it and _clickManager closed it again, so the popup
    // stopped responding after its first use. Own the toggle here instead —
    // once, for button 1 / touch only — and populate before opening, because
    // PopupMenu.open() bails out while the menu is still empty.
    vfunc_event(event) {
      const type = event.type();
      const isPrimaryPress =
        type === Clutter.EventType.TOUCH_BEGIN ||
        (type === Clutter.EventType.BUTTON_PRESS && event.get_button() === 1);

      if (this.menu && isPrimaryPress) {
        this._toggleProcessMenu();
      }

      return Clutter.EVENT_PROPAGATE;
    }

    _toggleProcessMenu() {
      if (!this.menu) {
        return;
      }

      if (this.menu.isOpen) {
        this.menu.close();
        this._topLiveStop();
        return;
      }

      this._refreshProcessMenu();
      this.menu.open();
      this._topLiveStart();
    }

    // ── Rolling window configuration ─────────────────────────────────────
    // U2TSSM is this repository's name condensed to its initials (Ubuntu 2404
    // Taskbar System Status Monitor). It is read live, so exporting it into
    // the session changes the window without re-patching; the baked-in default
    // below is written at install time by
    // `rm-monitor patch-process-popup --window-minutes N`.
    _topUsersWindowMinutes() {
      const patchedMinutes = __TOP_WINDOW_MINUTES__;
      const raw = GLib.getenv("U2TSSM");
      const parsed = raw === null ? Number.NaN : Number.parseInt(raw, 10);

      if (Number.isFinite(parsed) && parsed >= 1 && parsed <= 1440) {
        return parsed;
      }

      return patchedMinutes;
    }

    _topSampleSeconds() {
      return 10;
    }

    _topBucketSeconds() {
      // One bucket per window-minute keeps the bucket count near 60 whatever
      // the window is, so memory stays bounded; the floor keeps eviction
      // fine-grained for very short windows.
      return Math.max(10, this._topUsersWindowMinutes());
    }

    // ── Sampler lifecycle ────────────────────────────────────────────────
    _topUsersStart() {
      if (this._topSampleTimer) {
        return;
      }

      this._topBuckets = [];
      this._topCurrentBucket = null;
      this._topProcPrev = new Map();
      this._topProcNames = new Map();
      this._topWalk = null;
      this._topWalkSource = null;
      this._topDecoder = null;
      this._topCpuTotalPrev = 0;
      this._topNetPrev = new Map();
      this._topNetSampledAt = 0;
      this._topLastSampleAt = 0;
      this._topGpuBusy = false;
      this._topNetBusy = false;
      this._topGpuFeedPending = false;
      this._topNetFeedPending = false;
      this._topLiveTimer = null;
      this._topLiveRows = [];
      this._topLiveNames = new Set();
      this._topLiveProc = new Map();
      this._topLiveExtra = new Map();
      this._topLivePrev = new Map();
      this._topLiveCpuPrev = 0;
      this._topLiveAt = 0;
      this._topMemTotalKb = this._topReadMemTotalKb();
      this._topGpuProgram = GLib.find_program_in_path("nvidia-smi");
      this._topNetProgram = GLib.find_program_in_path("ss");

      this._topSampleTimer = GLib.timeout_add_seconds(
        GLib.PRIORITY_LOW,
        this._topSampleSeconds(),
        this._topSampleTick.bind(this)
      );
      this._topSampleTick();
    }

    _topUsersStop() {
      this._topLiveStop();

      if (this._topSampleTimer) {
        GLib.Source.remove(this._topSampleTimer);
        this._topSampleTimer = null;
      }

      // An in-flight chunked /proc walk owns an idle source of its own.
      if (this._topWalkSource) {
        GLib.Source.remove(this._topWalkSource);
        this._topWalkSource = null;
      }
      this._topWalk = null;
    }

    _topSampleTick() {
      if (this._destroyed) {
        this._topSampleTimer = null;
        return GLib.SOURCE_REMOVE;
      }

      // A sampler that throws must not take the timer — and with it every
      // later sample — down with it.
      try {
        this._topRotateBuckets();
        this._topSampleProcesses();
        this._topSampleGpu();
        this._topSampleNetwork();
      } catch (error) {
        this._logger.error(
          `[Resource_Monitor] Top-users sampling failed: ${error}`
        );
      }

      return GLib.SOURCE_CONTINUE;
    }

    _topRotateBuckets() {
      const now = GLib.get_monotonic_time() / 1000000;
      const bucketSeconds = this._topBucketSeconds();

      if (
        !this._topCurrentBucket ||
        now - this._topCurrentBucket.at >= bucketSeconds
      ) {
        this._topCurrentBucket = { at: now, samples: 0, totals: new Map() };
        this._topBuckets.push(this._topCurrentBucket);
      }

      const maxBuckets =
        Math.ceil((this._topUsersWindowMinutes() * 60) / bucketSeconds) + 1;
      while (this._topBuckets.length > maxBuckets) {
        this._topBuckets.shift();
      }
    }

    _topAddSample(name, metric, value) {
      const bucket = this._topCurrentBucket;
      if (!bucket || !name || !(value > 0)) {
        return;
      }

      let entry = bucket.totals.get(name);
      if (!entry) {
        // Bound the per-bucket map: a burst of uniquely named short-lived
        // processes must not grow the Shell's heap without limit.
        if (bucket.totals.size >= 256) {
          return;
        }
        entry = { cpu: 0, ram: 0, disk: 0, net: 0, gpu: 0, vram: 0 };
        bucket.totals.set(name, entry);
      }

      entry[metric] += value;
    }

    // ── /proc sampling (CPU, RAM, disk IO) ───────────────────────────────
    // A full sweep touches two files per process — roughly 1600 reads on a
    // busy desktop, ~25 ms — which is several dropped frames if it runs in one
    // go on the Shell's main loop. The sweep is therefore split into small
    // chunks driven by low-priority idle callbacks, so it only ever runs
    // between frames.
    _topReadTextFile(path) {
      try {
        const [ok, bytes] = GLib.file_get_contents(path);
        if (!ok) {
          return null;
        }
        // One decoder for the whole sweep: allocating one per file cost more
        // than the reads themselves.
        this._topDecoder ??= new TextDecoder();
        return this._topDecoder.decode(bytes);
      } catch (error) {
        // /proc entries routinely vanish between readdir and read.
        return null;
      }
    }

    _topReadMemTotalKb() {
      const meminfo = this._topReadTextFile("/proc/meminfo");
      const match = meminfo === null ? null : /MemTotal:\s+(\d+)/.exec(meminfo);
      return match ? Number.parseInt(match[1], 10) : 0;
    }

    _topReadCpuTotalJiffies() {
      const stat = this._topReadTextFile("/proc/stat");
      if (stat === null) {
        return 0;
      }

      const fields = stat.split("\n", 1)[0].trim().split(/\s+/);
      let total = 0;
      for (let i = 1; i < fields.length; i++) {
        const jiffies = Number.parseInt(fields[i], 10);
        if (Number.isFinite(jiffies)) {
          total += jiffies;
        }
      }

      return total;
    }

    _topReadProcIoBytes(pid) {
      // Unreadable for processes owned by other users. -1 means "unknown",
      // never "idle", so the next delta is skipped instead of counted.
      const io = this._topReadTextFile(`/proc/${pid}/io`);
      if (io === null) {
        return -1;
      }

      let total = 0;
      let found = 0;
      for (const line of io.split("\n")) {
        // "cancelled_write_bytes" deliberately does not match either prefix.
        if (line.startsWith("read_bytes:") || line.startsWith("write_bytes:")) {
          const value = Number.parseInt(line.slice(line.indexOf(":") + 1), 10);
          if (Number.isFinite(value)) {
            total += value;
            found += 1;
          }
        }
      }

      return found === 2 ? total : -1;
    }

    _topListPids() {
      let dir;
      try {
        dir = GLib.Dir.open("/proc", 0);
      } catch (error) {
        return [];
      }

      const pids = [];
      let entry;
      while ((entry = dir.read_name()) !== null) {
        // Every /proc entry starting with a digit is a PID, and a charCode
        // test is markedly cheaper than a regex across ~800 entries.
        const first = entry.charCodeAt(0);
        if (first >= 48 && first <= 57) {
          pids.push(entry);
        }
      }
      dir.close();

      return pids;
    }

    _topSampleProcesses() {
      if (this._topWalk) {
        // The previous sweep has not finished. Skipping keeps one consistent
        // set of deltas instead of interleaving two walks over the same state.
        return;
      }

      const now = GLib.get_monotonic_time() / 1000000;
      const elapsed =
        this._topLastSampleAt > 0 ? now - this._topLastSampleAt : 0;
      this._topLastSampleAt = now;

      const cpuTotal = this._topReadCpuTotalJiffies();
      const cpuDelta = cpuTotal - this._topCpuTotalPrev;
      this._topCpuTotalPrev = cpuTotal;

      const pids = this._topListPids();
      if (pids.length === 0) {
        return;
      }

      this._topWalk = {
        pids,
        index: 0,
        elapsed,
        cpuDelta,
        previous: this._topProcPrev,
        current: new Map(),
        names: new Map(),
      };
      this._topWalkSource = GLib.idle_add(
        GLib.PRIORITY_LOW,
        this._topWalkStep.bind(this)
      );
    }

    _topWalkStep() {
      const walk = this._topWalk;
      if (this._destroyed || !walk) {
        this._topWalk = null;
        this._topWalkSource = null;
        return GLib.SOURCE_REMOVE;
      }

      // ~2 ms of work per callback: small enough to disappear between frames.
      const end = Math.min(walk.pids.length, walk.index + 128);
      try {
        for (; walk.index < end; walk.index++) {
          this._topWalkOne(walk, walk.pids[walk.index]);
        }
      } catch (error) {
        this._logger.error(
          `[Resource_Monitor] Top-users /proc walk failed: ${error}`
        );
        this._topWalk = null;
        this._topWalkSource = null;
        return GLib.SOURCE_REMOVE;
      }

      if (walk.index < walk.pids.length) {
        return GLib.SOURCE_CONTINUE;
      }

      this._topProcPrev = walk.current;
      this._topProcNames = walk.names;

      // Only count a sample once deltas exist, so the very first sweep does
      // not dilute every average by one empty slot.
      if (this._topCurrentBucket && walk.previous.size > 0) {
        this._topCurrentBucket.samples += 1;
      }

      this._topWalk = null;
      this._topWalkSource = null;
      return GLib.SOURCE_REMOVE;
    }

    _topWalkOne(walk, pid) {
      const stat = this._topReadTextFile(`/proc/${pid}/stat`);
      if (stat === null) {
        return;
      }

      // comm is parenthesised and may itself contain spaces or brackets, so
      // the *last* ")" ends it and every numeric field follows from there.
      const opened = stat.indexOf("(");
      const closed = stat.lastIndexOf(")");
      if (opened < 0 || closed < opened) {
        return;
      }

      const name = stat.slice(opened + 1, closed);
      // /proc/pid/stat is single-space separated from the state field on, so
      // fields[i] is field i + 3: utime 14, stime 15, starttime 22, rss 24.
      const fields = stat.slice(closed + 2).split(" ");
      if (fields.length < 22) {
        return;
      }

      const jiffies =
        Number.parseInt(fields[11], 10) + Number.parseInt(fields[12], 10);
      const rssPages = Number.parseInt(fields[21], 10);
      const io = this._topReadProcIoBytes(pid);
      // Key on start time too: PIDs are recycled, and a recycled PID's
      // counters restart from zero.
      const key = `${pid}:${fields[19]}`;

      walk.current.set(key, { jiffies, io });
      walk.names.set(pid, name);

      const before = walk.previous.get(key);
      if (before === undefined) {
        return;
      }

      if (walk.cpuDelta > 0 && Number.isFinite(jiffies)) {
        this._topAddSample(
          name,
          "cpu",
          (100 * (jiffies - before.jiffies)) / walk.cpuDelta
        );
      }

      if (this._topMemTotalKb > 0 && Number.isFinite(rssPages)) {
        // /proc counts RSS in pages; 4 KiB is the page size on every platform
        // this extension supports (x86_64 / aarch64).
        this._topAddSample(
          name,
          "ram",
          (100 * rssPages * 4) / this._topMemTotalKb
        );
      }

      if (walk.elapsed > 0 && io >= 0 && before.io >= 0) {
        this._topAddSample(
          name,
          "disk",
          Math.max(0, io - before.io) / walk.elapsed
        );
      }
    }

    // ── Subprocess sampling (GPU, network) ───────────────────────────────
    _topSpawnAsync(argv, onOutput) {
      let proc;
      try {
        proc = new Gio.Subprocess({
          argv,
          flags:
            Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE,
        });
        proc.init(null);
      } catch (error) {
        onOutput(null);
        return;
      }

      proc.communicate_utf8_async(null, this._ioCancellable, (p, res) => {
        let stdout = "";
        try {
          [, stdout] = p.communicate_utf8_finish(res);
        } catch (error) {
          onOutput(null);
          return;
        }

        if (this._destroyed) {
          return;
        }

        try {
          onOutput(stdout ?? "");
        } catch (error) {
          this._logger.error(
            `[Resource_Monitor] Top-users sampler failed on ${argv[0]}: ${error}`
          );
        }
      });
    }

    // `feed` marks a run whose reading belongs in the rolling buckets. The
    // live column spawns extra runs with feed=false: those refresh the "now"
    // column only, because contributing them would silently weight the
    // averages towards whenever the popup happened to be open. A feeding run
    // that arrives while a live run is still in flight is not dropped — the
    // request is held and handed to the next run to complete, so the bucket
    // cadence survives having the popup open.
    _topSampleGpu(feed = true) {
      if (feed) {
        this._topGpuFeedPending = true;
      }

      if (this._topGpuBusy || !this._topGpuProgram) {
        return;
      }

      const feeding = this._topGpuFeedPending === true;
      this._topGpuFeedPending = false;
      this._topGpuBusy = true;
      this._topSpawnAsync(
        [this._topGpuProgram, "pmon", "-c", "1", "-s", "um"],
        (stdout) => {
          this._topGpuBusy = false;
          if (stdout !== null) {
            this._topRecordGpuSample(stdout, feeding);
          }
        }
      );
    }

    _topRecordGpuSample(stdout, feeding = true) {
      const live = new Map();
      // `nvidia-smi pmon` column sets differ between driver releases, so the
      // header row — not a fixed offset — decides where pid / sm / fb live.
      let columns = null;

      for (const line of stdout.split("\n")) {
        if (line.startsWith("#")) {
          const header = line.slice(1).trim().split(/\s+/);
          const found = {
            pid: header.indexOf("pid"),
            sm: header.indexOf("sm"),
            fb: header.indexOf("fb"),
          };
          if (found.pid >= 0 && found.sm >= 0 && found.fb >= 0) {
            columns = found;
          }
          continue;
        }

        const fields = line.trim().split(/\s+/);
        if (columns === null || fields.length <= columns.fb) {
          continue;
        }

        // Prefer the comm already collected from /proc: pmon truncates its
        // command column and can carry arguments, and every other section
        // keys off comm.
        const name =
          this._topProcNames.get(fields[columns.pid]) ??
          fields[fields.length - 1];
        const sm = Number.parseFloat(fields[columns.sm]);
        const fb = Number.parseFloat(fields[columns.fb]);

        for (const [metric, value] of [
          ["gpu", sm],
          ["vram", fb],
        ]) {
          if (!Number.isFinite(value)) {
            continue;
          }

          if (feeding) {
            this._topAddSample(name, metric, value);
          }

          let seen = live.get(name);
          if (!seen) {
            seen = { gpu: 0, vram: 0 };
            live.set(name, seen);
          }
          seen[metric] += value;
        }
      }

      this._topLiveMerge(live, ["gpu", "vram"]);
    }

    // See _topSampleGpu for what `feed` means.
    _topSampleNetwork(feed = true) {
      if (feed) {
        this._topNetFeedPending = true;
      }

      if (this._topNetBusy || !this._topNetProgram) {
        return;
      }

      const feeding = this._topNetFeedPending === true;
      this._topNetFeedPending = false;
      this._topNetBusy = true;
      this._topSpawnAsync([this._topNetProgram, "-tnpHi"], (stdout) => {
        this._topNetBusy = false;
        if (stdout !== null) {
          this._topRecordNetworkSample(stdout, feeding);
        }
      });
    }

    _topRecordNetworkSample(stdout, feeding = true) {
      // `ss -i` prints one socket line naming its owning process, followed by
      // an indented stats line carrying that socket's cumulative byte
      // counters. Only TCP sockets report bytes, hence the -t query.
      const cumulative = new Map();
      let owner = null;

      for (const line of stdout.split("\n")) {
        if (line === "") {
          continue;
        }

        if (!/^\s/.test(line)) {
          const match = /users:\(\("([^"]+)"/.exec(line);
          owner = match ? match[1] : null;
          continue;
        }

        if (owner === null) {
          continue;
        }

        const sent = /bytes_sent:(\d+)/.exec(line);
        const received = /bytes_received:(\d+)/.exec(line);
        const bytes =
          (sent ? Number.parseInt(sent[1], 10) : 0) +
          (received ? Number.parseInt(received[1], 10) : 0);

        cumulative.set(owner, (cumulative.get(owner) ?? 0) + bytes);
        owner = null;
      }

      const now = GLib.get_monotonic_time() / 1000000;
      const elapsed =
        this._topNetSampledAt > 0 ? now - this._topNetSampledAt : 0;
      this._topNetSampledAt = now;

      const live = new Map();

      if (elapsed > 0) {
        for (const [name, bytes] of cumulative) {
          const before = this._topNetPrev.get(name);
          // Closing sockets shrink the sum; a drop means "nothing measurable
          // this round", not a negative rate.
          if (before !== undefined && bytes > before) {
            const rate = (bytes - before) / elapsed;
            if (feeding) {
              this._topAddSample(name, "net", rate);
            }
            live.set(name, { net: rate });
          }
        }
      }

      this._topLiveMerge(live, ["net"]);
      this._topNetPrev = cumulative;
    }

    // ── Live ("now") sampling, only while the menu is open ───────────────
    // The live column has to tick at the panel's rate, which is far faster
    // than the 10 s bucket sampler. Rather than speed that sampler up — which
    // would distort every rolling average — this keeps its own delta state
    // and writes to its own store, and it only exists between menu open and
    // menu close.
    _topLiveIntervalMs() {
      // The panel's own refresh-time setting, so the two update together.
      const seconds = Number.isFinite(this._refreshTime) ? this._refreshTime : 1;
      return Math.max(100, Math.round(seconds * 1000));
    }

    _topLiveStart() {
      if (this._topLiveTimer) {
        return;
      }

      // Deltas from before the popup was opened would span the whole idle
      // gap, so the first tick after opening only establishes a baseline.
      this._topLivePrev = new Map();
      this._topLiveCpuPrev = this._topReadCpuTotalJiffies();
      this._topLiveAt = 0;

      this._topLiveTimer = GLib.timeout_add(
        GLib.PRIORITY_DEFAULT_IDLE,
        this._topLiveIntervalMs(),
        this._topLiveTick.bind(this)
      );
    }

    _topLiveStop() {
      if (this._topLiveTimer) {
        GLib.Source.remove(this._topLiveTimer);
        this._topLiveTimer = null;
      }
    }

    _topLiveTick() {
      // Closing the menu from anywhere — Escape, a click elsewhere, another
      // panel button — does not route through _toggleProcessMenu, so the
      // timer also retires itself as soon as the menu is gone.
      if (this._destroyed || !this.menu || !this.menu.isOpen) {
        this._topLiveTimer = null;
        return GLib.SOURCE_REMOVE;
      }

      try {
        this._topSampleLive();
        // These cost a subprocess and cannot answer at the panel's rate; the
        // busy guards throttle them to however fast the tool actually
        // returns, and feed=false keeps them out of the buckets.
        this._topSampleGpu(false);
        this._topSampleNetwork(false);
        this._topRenderLiveValues();
      } catch (error) {
        this._logger.error(
          `[Resource_Monitor] Top-users live sampling failed: ${error}`
        );
      }

      return GLib.SOURCE_CONTINUE;
    }

    // Replace one metric group in the live snapshot. Names missing from the
    // new reading have stopped using that resource, so they drop to zero
    // instead of keeping the last value they were seen with.
    _topLiveMerge(values, keys) {
      const store = this._topLiveExtra;
      if (!store) {
        return;
      }

      for (const [name, entry] of store) {
        if (!values.has(name)) {
          for (const key of keys) {
            entry[key] = 0;
          }
        }
      }

      for (const [name, sample] of values) {
        let entry = store.get(name);
        if (!entry) {
          // Same heap bound as the buckets.
          if (store.size >= 256) {
            continue;
          }
          entry = { net: 0, gpu: 0, vram: 0 };
          store.set(name, entry);
        }

        for (const key of keys) {
          entry[key] = sample[key] ?? 0;
        }
      }
    }

    _topSampleLive() {
      const wanted = this._topLiveNames;
      if (!wanted || wanted.size === 0) {
        return;
      }

      const now = GLib.get_monotonic_time() / 1000000;
      const elapsed = this._topLiveAt > 0 ? now - this._topLiveAt : 0;
      this._topLiveAt = now;

      const cpuTotal = this._topReadCpuTotalJiffies();
      const cpuDelta = cpuTotal - this._topLiveCpuPrev;
      this._topLiveCpuPrev = cpuTotal;

      const previous = this._topLivePrev;
      const current = new Map();
      const totals = new Map();

      // Only the handful of processes the popup is actually showing, reached
      // through the pid→name map the last full sweep already built. A fresh
      // ~1600-file /proc walk twice a second would cost far more than these
      // rows are worth, and a process too new to be in that map has no
      // history to be ranked by yet either.
      for (const [pid, name] of this._topProcNames ?? new Map()) {
        if (!wanted.has(name)) {
          continue;
        }

        const stat = this._topReadTextFile(`/proc/${pid}/stat`);
        if (stat === null) {
          continue;
        }

        const closed = stat.lastIndexOf(")");
        if (closed < 0) {
          continue;
        }

        const fields = stat.slice(closed + 2).split(" ");
        if (fields.length < 22) {
          continue;
        }

        const jiffies =
          Number.parseInt(fields[11], 10) + Number.parseInt(fields[12], 10);
        const rssPages = Number.parseInt(fields[21], 10);
        const io = this._topReadProcIoBytes(pid);
        const key = `${pid}:${fields[19]}`;

        current.set(key, { jiffies, io });

        let entry = totals.get(name);
        if (!entry) {
          entry = { cpu: 0, ram: 0, disk: 0 };
          totals.set(name, entry);
        }

        // RAM is a level, not a delta: it reads correctly on the very first
        // tick, while CPU and disk need a previous sample to compare against.
        if (this._topMemTotalKb > 0 && Number.isFinite(rssPages)) {
          entry.ram += (100 * rssPages * 4) / this._topMemTotalKb;
        }

        const before = previous.get(key);
        if (before === undefined) {
          continue;
        }

        if (cpuDelta > 0 && Number.isFinite(jiffies)) {
          entry.cpu += (100 * (jiffies - before.jiffies)) / cpuDelta;
        }

        if (elapsed > 0 && io >= 0 && before.io >= 0) {
          entry.disk += Math.max(0, io - before.io) / elapsed;
        }
      }

      this._topLivePrev = current;
      this._topLiveProc = totals;
    }

    // Current reading for one process name, merging the /proc-derived metrics
    // with the subprocess-derived ones. Absent means "not using it now" — 0,
    // not the trailing average.
    _topLiveValue(name, metric) {
      const proc = this._topLiveProc?.get(name);
      if (proc && metric in proc) {
        return proc[metric];
      }

      const extra = this._topLiveExtra?.get(name);
      if (extra && metric in extra) {
        return extra[metric];
      }

      return 0;
    }

    // ── Aggregation and rendering ────────────────────────────────────────
    _topMetricKeys() {
      return ["cpu", "ram", "disk", "net", "gpu", "vram"];
    }

    _topAggregate() {
      const keys = this._topMetricKeys();
      const totals = new Map();
      let samples = 0;

      for (const bucket of this._topBuckets ?? []) {
        samples += bucket.samples;

        for (const [name, entry] of bucket.totals) {
          let sum = totals.get(name);
          if (!sum) {
            sum = { cpu: 0, ram: 0, disk: 0, net: 0, gpu: 0, vram: 0 };
            totals.set(name, sum);
          }

          for (const key of keys) {
            sum[key] += entry[key];
          }
        }
      }

      // Divide by the window's sample count, not by each process's own
      // lifetime: a process busy for two of sixty minutes should rank below
      // one busy throughout.
      if (samples > 0) {
        for (const sum of totals.values()) {
          for (const key of keys) {
            sum[key] /= samples;
          }
        }
      }

      return { totals, samples };
    }

    _topFormatPercent(value) {
      return `${value.toFixed(1)}%`;
    }

    _topFormatBytesPerSecond(value) {
      if (value >= 1048576) {
        return `${(value / 1048576).toFixed(1)} MB/s`;
      }

      if (value >= 1024) {
        return `${(value / 1024).toFixed(0)} kB/s`;
      }

      return `${value.toFixed(0)} B/s`;
    }

    _topFormatBitsPerSecond(value) {
      // The panel's network columns read in bits, so these rows match them.
      const bits = value * 8;
      if (bits >= 1000000) {
        return `${(bits / 1000000).toFixed(1)} Mb/s`;
      }

      if (bits >= 1000) {
        return `${(bits / 1000).toFixed(0)} kb/s`;
      }

      return `${bits.toFixed(0)} b/s`;
    }

    _topFormatMegabytes(value) {
      if (value >= 1024) {
        return `${(value / 1024).toFixed(1)} GB`;
      }

      return `${value.toFixed(0)} MB`;
    }

    _topSections() {
      return [
        {
          metric: "cpu",
          label: _("CPU"),
          format: (value) => this._topFormatPercent(value),
        },
        {
          metric: "ram",
          label: _("RAM"),
          format: (value) => this._topFormatPercent(value),
        },
        {
          metric: "disk",
          label: _("Disk usage"),
          format: (value) => this._topFormatBytesPerSecond(value),
        },
        {
          metric: "net",
          label: _("Network"),
          format: (value) => this._topFormatBitsPerSecond(value),
        },
        {
          metric: "gpu",
          label: _("GPU usage"),
          format: (value) => this._topFormatPercent(value),
        },
        {
          metric: "vram",
          label: _("GPU VRAM"),
          format: (value) => this._topFormatMegabytes(value),
        },
      ];
    }

    // One place decides the column widths, so the live repaint can never
    // drift out of alignment with the row the initial render produced.
    _topRowText(label, current, average, format) {
      return `${label.padEnd(19)}${format(current).padStart(11)}${format(
        average
      ).padStart(11)}`;
    }

    // Repaint the "now" column in place. Rebuilding the menu at the panel's
    // refresh rate would fight the pointer and reset scroll on every tick, so
    // the rows built at open time are kept and only their text is rewritten.
    _topRenderLiveValues() {
      for (const row of this._topLiveRows ?? []) {
        if (!row.item || !row.item.label) {
          continue;
        }

        row.item.label.set_text(
          this._topRowText(
            row.label,
            this._topLiveValue(row.name, row.metric),
            row.average,
            row.format
          )
        );
      }
    }

    _refreshProcessMenu() {
      this.menu.removeAll();
      this._topLiveRows = [];
      this._topLiveNames = new Set();
      // Values from a previous opening are stale by an unknown amount; the
      // first live ticks refill this within a refresh or two.
      this._topLiveProc = new Map();

      const minutes = this._topUsersWindowMinutes();
      const { totals, samples } = this._topAggregate();
      const collected = Math.min(
        minutes,
        Math.round((samples * this._topSampleSeconds()) / 60)
      );

      const title =
        collected < minutes
          ? `${_("Top process users")} — ${_("last")} ${minutes} ${_(
              "min"
            )} (${collected} ${_("collected")})`
          : `${_("Top process users")} — ${_("last")} ${minutes} ${_("min")}`;
      const header = new PopupMenu.PopupMenuItem(title, { reactive: false });
      header.label.set_style("font-weight: bold;");
      this.menu.addMenuItem(header);

      const columns = new PopupMenu.PopupMenuItem(
        `${"".padEnd(19)}${_("now").padStart(11)}${_("avg").padStart(11)}`,
        { reactive: false }
      );
      columns.label.set_style("font-family: monospace; font-weight: bold;");
      this.menu.addMenuItem(columns);

      for (const section of this._topSections()) {
        this.menu.addMenuItem(
          new PopupMenu.PopupSeparatorMenuItem(section.label)
        );

        const rows = [...totals.entries()]
          .map((pair) => [pair[0], pair[1][section.metric]])
          .filter((pair) => pair[1] > 0)
          .sort((a, b) => b[1] - a[1])
          .slice(0, 5);

        if (rows.length === 0) {
          const empty = new PopupMenu.PopupMenuItem(_("No data yet."), {
            reactive: false,
          });
          empty.label.set_style("font-family: monospace;");
          this.menu.addMenuItem(empty);
          continue;
        }

        for (const [name, value] of rows) {
          // Just the process name — no PID, no command line, no arguments.
          const label = name.length > 18 ? `${name.slice(0, 17)}…` : name;
          const item = new PopupMenu.PopupMenuItem(
            this._topRowText(
              label,
              this._topLiveValue(name, section.metric),
              value,
              section.format
            ),
            { reactive: false }
          );
          item.label.set_style("font-family: monospace;");
          this.menu.addMenuItem(item);

          // Everything the live repaint needs, so a tick never re-derives the
          // ranking: the row order stays put while only "now" moves.
          this._topLiveNames.add(name);
          this._topLiveRows.push({
            item,
            label,
            name,
            metric: section.metric,
            format: section.format,
            average: value,
          });
        }
      }
    }

"##;

/// Render [`METHODS_TEMPLATE`] with the rolling window baked in.
///
/// # Parameters
/// - `window_minutes`: Trailing window the popup ranks processes over.
fn methods(window_minutes: u32) -> String {
    METHODS_TEMPLATE.replace(WINDOW_PLACEHOLDER, &window_minutes.to_string())
}

const OLD_CLICK: &str = r#"        case 1: // Left-click
          this._launchPrimaryAction();"#;

const NEW_CLICK: &str = r#"        case 1: // Left-click
          // vfunc_event above already toggled the process popup for this very
          // event. Toggling again here would cancel it out and leave the panel
          // button looking dead, so only swallow the click (which also keeps
          // the upstream _launchPrimaryAction from running)."#;

/// First-generation rewrite that toggled the popup a second time per click.
/// Re-running the patcher over an already-installed tree must upgrade it.
const LEGACY_CLICK: &str = r#"        case 1: // Left-click
          // Show per-process CPU/RAM data instead of launching the task manager.
          this._toggleProcessMenu();"#;

const OLD_KEY: &str = r#"        case Clutter.KEY_space:
          this._launchPrimaryAction();"#;

const NEW_KEY: &str = r#"        case Clutter.KEY_space:
          // Keyboard activation mirrors left-click: show the process popup.
          this._toggleProcessMenu();"#;

const OLD_TOOLTIP: &str = r#"_("Left-click launches the configured action.")"#;

const NEW_TOOLTIP: &str = r#"_("Left-click shows the top process users of each metric.")"#;

/// Tooltip written by the first generation of this patcher.
const LEGACY_TOOLTIP: &str = r#"_("Left-click shows per-process CPU and RAM usage.")"#;

// ── Sampler lifecycle hooks ──────────────────────────────────────────────────
// The popup needs history from the moment the extension loads, so the sampler
// starts in `_init` rather than on first click. `_refreshHandler();` also
// appears in `_refreshTimeChanged`, so the `_setupAccessibility` tail is what
// pins these snippets to the constructor.

const OLD_INIT_TAIL: &str = r#"      this._refreshHandler();
    }

    _setupAccessibility() {"#;

const NEW_INIT_TAIL: &str = r#"      this._refreshHandler();

      // Collect top-process history from load, not from the first click.
      this._topUsersStart();
    }

    _setupAccessibility() {"#;

const OLD_DESTROY_HEAD: &str = r#"    destroy() {
      this._destroyed = true;

      if (this._mainTimer) {"#;

const NEW_DESTROY_HEAD: &str = r#"    destroy() {
      this._destroyed = true;

      this._topUsersStop();

      if (this._mainTimer) {"#;

/// Failure while applying the process-popup patch.
#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)] // Missing* names mirror the absent snippet
pub enum ProcessPopupError {
    /// The `PanelMenu` import anchor is missing.
    #[error("ERROR: PanelMenu import not found; upstream layout changed — aborting.")]
    MissingPanelMenuImport,
    /// The `_clickManager` anchor is missing.
    #[error("ERROR: _clickManager not found; upstream layout changed — aborting.")]
    MissingClickManager,
    /// Neither the upstream nor the patched left-click case is present.
    #[error("ERROR: left-click _launchPrimaryAction case not found — aborting.")]
    MissingLeftClickCase,
    /// Neither the upstream nor the patched keyboard case is present.
    #[error("ERROR: keyboard _launchPrimaryAction case not found — aborting.")]
    MissingKeyboardCase,
    /// Neither the upstream nor the patched tooltip string is present.
    #[error("ERROR: process-popup tooltip string not found — aborting.")]
    MissingTooltip,
    /// The `_init` tail that starts the background sampler is missing.
    #[error("ERROR: _init refresh-handler tail not found; upstream layout changed — aborting.")]
    MissingInitHook,
    /// The `destroy` head that stops the background sampler is missing.
    #[error("ERROR: destroy() head not found; upstream layout changed — aborting.")]
    MissingDestroyHook,
    /// The requested rolling window is outside the range the popup accepts.
    #[error("ERROR: --window-minutes must be between 1 and {MAX_WINDOW_MINUTES}.")]
    WindowOutOfRange,
}

/// Apply every process-popup edit to `extension.js` content.
///
/// # Parameters
/// - `content`: Current `extension.js` content.
/// - `window_minutes`: Trailing window baked into the injected sampler.
///
/// Returns the patched content and whether anything changed.
pub fn patch_extension_js(
    content: &str,
    window_minutes: u32,
) -> Result<(String, bool), ProcessPopupError> {
    if window_minutes == 0 || window_minutes > MAX_WINDOW_MINUTES {
        return Err(ProcessPopupError::WindowOutOfRange);
    }

    let mut content = content.to_string();
    let mut changed = false;

    // ── 1. PopupMenu import ──────────────────────────────────────────────
    if !content.contains(POPUP_MENU_IMPORT) {
        if !content.contains(PANEL_MENU_IMPORT) {
            return Err(ProcessPopupError::MissingPanelMenuImport);
        }
        content = content.replacen(
            PANEL_MENU_IMPORT,
            &format!("{PANEL_MENU_IMPORT}\n{POPUP_MENU_IMPORT}"),
            1,
        );
        changed = true;
    }

    // ── 2. Popup-menu methods, inserted (or upgraded) before _clickManager ─
    // The block is always re-rendered rather than merely presence-checked, so
    // a changed body (or a changed --window-minutes) converges on one re-run
    // instead of leaving a stale mixture behind.
    if !content.contains(CLICK_MANAGER_ANCHOR) {
        return Err(ProcessPopupError::MissingClickManager);
    }

    let rendered = methods(window_minutes);
    let existing = content
        .find(METHODS_START)
        .or_else(|| content.find(LEGACY_METHODS_START));
    match existing {
        Some(start) => {
            let Some(rel_end) = content[start..].find(CLICK_MANAGER_ANCHOR) else {
                return Err(ProcessPopupError::MissingClickManager);
            };
            let end = start + rel_end;
            if content[start..end] != rendered {
                content = format!("{}{}{}", &content[..start], rendered, &content[end..]);
                changed = true;
            }
        }
        None => {
            content = content.replacen(
                CLICK_MANAGER_ANCHOR,
                &format!("{rendered}{CLICK_MANAGER_ANCHOR}"),
                1,
            );
            changed = true;
        }
    }

    // ── 3. Rewire left-click and keyboard activation ─────────────────────
    if content.contains(OLD_CLICK) {
        content = content.replacen(OLD_CLICK, NEW_CLICK, 1);
        changed = true;
    } else if content.contains(LEGACY_CLICK) {
        // Upgrade an in-place install that still double-toggles the popup.
        content = content.replacen(LEGACY_CLICK, NEW_CLICK, 1);
        changed = true;
    } else if !content.contains(NEW_CLICK) {
        return Err(ProcessPopupError::MissingLeftClickCase);
    }

    if content.contains(OLD_KEY) {
        content = content.replacen(OLD_KEY, NEW_KEY, 1);
        changed = true;
    } else if !content.contains(NEW_KEY) {
        return Err(ProcessPopupError::MissingKeyboardCase);
    }

    // ── 4. Tooltip describes the new behavior ────────────────────────────
    if content.contains(OLD_TOOLTIP) {
        content = content.replacen(OLD_TOOLTIP, NEW_TOOLTIP, 1);
        changed = true;
    } else if content.contains(LEGACY_TOOLTIP) {
        content = content.replacen(LEGACY_TOOLTIP, NEW_TOOLTIP, 1);
        changed = true;
    } else if !content.contains(NEW_TOOLTIP) {
        return Err(ProcessPopupError::MissingTooltip);
    }

    // ── 5. Start and stop the background sampler ─────────────────────────
    if !content.contains(NEW_INIT_TAIL) {
        if !content.contains(OLD_INIT_TAIL) {
            return Err(ProcessPopupError::MissingInitHook);
        }
        content = content.replacen(OLD_INIT_TAIL, NEW_INIT_TAIL, 1);
        changed = true;
    }

    if !content.contains(NEW_DESTROY_HEAD) {
        if !content.contains(OLD_DESTROY_HEAD) {
            return Err(ProcessPopupError::MissingDestroyHook);
        }
        content = content.replacen(OLD_DESTROY_HEAD, NEW_DESTROY_HEAD, 1);
        changed = true;
    }

    Ok((content, changed))
}

/// CLI entry point for the process-popup patcher.
///
/// # Parameters
/// - `extension_path`: Path to the extension's `extension.js`.
/// - `window_minutes`: Trailing window the popup ranks processes over.
///
/// Returns `0` on success (including the already-applied case) and `1` when an
/// upstream anchor is missing or the file cannot be read/written.
pub fn run(extension_path: &Path, window_minutes: u32) -> i32 {
    logging::info(format!(
        "patch-process-popup path={} window_minutes={window_minutes}",
        extension_path.display()
    ));

    let content = match fs::read_to_string(extension_path) {
        Ok(content) => content,
        Err(err) => {
            logging::error(format!(
                "Could not read {}: {err}",
                extension_path.display()
            ));
            eprintln!("Could not read {}: {err}", extension_path.display());
            return 1;
        }
    };

    let (patched, changed) = match patch_extension_js(&content, window_minutes) {
        Ok(result) => result,
        Err(err) => {
            logging::error(err.to_string());
            eprintln!("{err}");
            return 1;
        }
    };

    if !changed {
        logging::info("Process popup already applied; nothing to do.");
        println!("Process popup already applied; nothing to do.");
        return 0;
    }

    if let Err(err) = crate::patch_text::write_atomic(extension_path, &patched) {
        logging::error(format!(
            "Could not write {}: {err}",
            extension_path.display()
        ));
        eprintln!("Could not write {}: {err}", extension_path.display());
        return 1;
    }

    logging::info(format!(
        "Patched {}: left-click now shows top process users over {window_minutes} min.",
        extension_path.display()
    ));
    println!(
        "Patched {}: left-click now shows top process users over {window_minutes} min.",
        extension_path.display()
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upstream() -> String {
        format!(
            "{PANEL_MENU_IMPORT}\n\n\
             {OLD_INIT_TAIL}\n\
             {OLD_DESTROY_HEAD}\n\
             {CLICK_MANAGER_ANCHOR}\n\
             {OLD_CLICK}\n\
             {OLD_KEY}\n\
             const tooltip = {OLD_TOOLTIP};\n"
        )
    }

    fn patched_upstream() -> String {
        let (patched, changed) =
            patch_extension_js(&upstream(), DEFAULT_WINDOW_MINUTES).expect("patch");
        assert!(changed);
        patched
    }

    #[test]
    fn injected_methods_carry_the_idempotence_marker() {
        assert!(METHODS_START.contains(MARKER));
        assert!(METHODS_TEMPLATE.starts_with(METHODS_START));
        assert!(METHODS_TEMPLATE.ends_with("    }\n\n"));
    }

    #[test]
    fn injected_methods_keep_javascript_escapes_literal() {
        // These must reach extension.js as backslash escapes for GJS to parse.
        assert!(METHODS_TEMPLATE.contains(r#"stdout.split("\n")"#));
        assert!(METHODS_TEMPLATE.contains(r"line.trim().split(/\s+/)"));
        assert!(METHODS_TEMPLATE.contains(r"/^\s/.test(line)"));
        assert!(METHODS_TEMPLATE.contains(r"/MemTotal:\s+(\d+)/"));
        assert!(METHODS_TEMPLATE.contains(r"/bytes_sent:(\d+)/"));
        assert!(
            METHODS_TEMPLATE.contains("`[Resource_Monitor] Top-users sampling failed: ${error}`")
        );
    }

    #[test]
    fn every_requested_metric_has_its_own_section() {
        // CPU, RAM, disk *usage* (IO, never free space), network, GPU usage and
        // GPU VRAM each get their own top-users list.
        for metric in ["cpu", "ram", "disk", "net", "gpu", "vram"] {
            assert!(
                METHODS_TEMPLATE.contains(&format!("metric: \"{metric}\",")),
                "{metric} section missing"
            );
        }
        assert!(METHODS_TEMPLATE.contains(r#"_("Disk usage")"#));
        assert!(METHODS_TEMPLATE.contains(r#"_("GPU usage")"#));
        assert!(METHODS_TEMPLATE.contains(r#"_("GPU VRAM")"#));
        assert!(METHODS_TEMPLATE.contains(r#"_("Network")"#));
        // Disk rows are IO rate, not capacity: no free/total-space maths here.
        assert!(!METHODS_TEMPLATE.contains("availableBytes"));
    }

    #[test]
    fn every_row_carries_a_live_column_beside_the_average() {
        // One formatter owns both columns, so an in-place live repaint cannot
        // drift out of alignment with the row the initial render produced.
        assert!(METHODS_TEMPLATE.contains("_topRowText(label, current, average, format) {"));
        assert!(METHODS_TEMPLATE
            .contains(r#"${label.padEnd(19)}${format(current).padStart(11)}${format("#));
        // The header names the two columns.
        assert!(METHODS_TEMPLATE.contains(r#"_("now").padStart(11)}${_("avg").padStart(11)}"#));
        // Rows stay ranked by the average, so their order holds still while
        // the live column ticks underneath.
        assert!(METHODS_TEMPLATE.contains(".sort((a, b) => b[1] - a[1])"));
    }

    #[test]
    fn live_column_follows_the_panel_refresh_time() {
        // "Same rate as the bar" means the panel's own refresh-time setting.
        assert!(
            METHODS_TEMPLATE.contains("Number.isFinite(this._refreshTime) ? this._refreshTime : 1")
        );
        assert!(METHODS_TEMPLATE.contains("this._topLiveIntervalMs(),"));
        // Started on open and stopped on close, on destroy, and by the tick
        // itself when the menu closed without going through the toggle.
        assert!(METHODS_TEMPLATE.contains("this._topLiveStart();"));
        assert!(METHODS_TEMPLATE.contains("this._topLiveStop();"));
        assert!(
            METHODS_TEMPLATE.contains("if (this._destroyed || !this.menu || !this.menu.isOpen) {")
        );
        // Repaint the existing rows; never rebuild the menu under the pointer.
        assert!(METHODS_TEMPLATE.contains("row.item.label.set_text("));
    }

    #[test]
    fn live_sampling_never_feeds_the_rolling_averages() {
        // Contributing the extra live runs would weight every average towards
        // whenever the popup happened to be open.
        assert!(METHODS_TEMPLATE.contains("this._topSampleGpu(false);"));
        assert!(METHODS_TEMPLATE.contains("this._topSampleNetwork(false);"));
        // The 10 s bucket tick keeps the feeding default.
        assert!(METHODS_TEMPLATE
            .contains("        this._topSampleGpu();\n        this._topSampleNetwork();"));
        // A bucket sample requested while a live run is in flight is handed to
        // the next run to complete instead of being dropped.
        for pending in ["_topGpuFeedPending", "_topNetFeedPending"] {
            assert!(
                METHODS_TEMPLATE.contains(&format!("this.{pending} = true;")),
                "{pending} is never requested"
            );
            assert!(
                METHODS_TEMPLATE.contains(&format!("this.{pending} = false;")),
                "{pending} is never consumed"
            );
        }
    }

    #[test]
    fn live_sampling_only_reads_the_processes_on_show() {
        // A full ~1600-file /proc walk at the panel's refresh rate would cost
        // far more than these rows are worth, so the live pass filters the
        // last sweep's pid→name map down to the names actually displayed.
        assert!(METHODS_TEMPLATE
            .contains("for (const [pid, name] of this._topProcNames ?? new Map()) {"));
        assert!(METHODS_TEMPLATE.contains("if (!wanted.has(name)) {"));
        assert!(METHODS_TEMPLATE.contains("this._topLiveNames.add(name);"));
        // It keeps delta state of its own rather than disturbing the sampler's.
        assert!(METHODS_TEMPLATE.contains("this._topLivePrev = current;"));
        assert!(METHODS_TEMPLATE.contains("this._topLiveProc = totals;"));
    }

    #[test]
    fn live_readings_decay_to_zero_instead_of_sticking() {
        // A process that stopped using the GPU or network must not keep
        // showing the last value it was seen with.
        assert!(METHODS_TEMPLATE.contains("_topLiveMerge(values, keys) {"));
        assert!(METHODS_TEMPLATE.contains("if (!values.has(name)) {"));
        assert!(METHODS_TEMPLATE.contains(r#"this._topLiveMerge(live, ["gpu", "vram"]);"#));
        assert!(METHODS_TEMPLATE.contains(r#"this._topLiveMerge(live, ["net"]);"#));
        // The live store is bounded exactly like the buckets are.
        assert!(METHODS_TEMPLATE.contains("if (store.size >= 256) {"));
    }

    #[test]
    fn applies_every_edit_to_upstream_source() {
        let patched = patched_upstream();
        assert!(patched.contains(POPUP_MENU_IMPORT));
        assert!(patched.contains(MARKER));
        assert!(patched.contains(NEW_CLICK));
        assert!(patched.contains(NEW_KEY));
        assert!(patched.contains(NEW_TOOLTIP));
        assert!(patched.contains(NEW_INIT_TAIL));
        assert!(patched.contains(NEW_DESTROY_HEAD));
        assert!(!patched.contains("this._launchPrimaryAction();"));
        // The methods must land immediately before _clickManager.
        let methods_at = patched.find(MARKER).expect("marker");
        let anchor_at = patched.find(CLICK_MANAGER_ANCHOR).expect("anchor");
        assert!(methods_at < anchor_at);
    }

    #[test]
    fn sampler_is_started_in_init_and_stopped_in_destroy() {
        // Without both hooks the popup would either have no history to show or
        // would keep a GLib timeout alive across an extension disable.
        let patched = patched_upstream();
        assert_eq!(patched.matches("this._topUsersStart();").count(), 1);
        assert_eq!(patched.matches("this._topUsersStop();").count(), 1);
        assert!(patched.contains("    _topUsersStart() {"));
        assert!(patched.contains("    _topUsersStop() {"));
    }

    #[test]
    fn only_one_toggle_owner_survives_the_patch() {
        let patched = patched_upstream();
        // vfunc_event is the sole click-driven toggle: _clickManager's
        // left-click case must no longer call _toggleProcessMenu, otherwise the
        // two toggles cancel out and the popup stops opening (see module docs).
        assert!(patched.contains("    vfunc_event(event) {"));
        assert_eq!(patched.matches("this._toggleProcessMenu();").count(), 2);
        let click_at = patched.find(NEW_CLICK).expect("left-click case");
        let key_at = patched.find(NEW_KEY).expect("key case");
        assert!(!patched[click_at..key_at].contains("_toggleProcessMenu"));
    }

    #[test]
    fn vfunc_event_toggles_only_for_primary_press() {
        // Right-click must reach _clickManager's preferences case without the
        // base-class toggle leaving a stray popup open behind the dialog.
        assert!(METHODS_TEMPLATE.contains("type === Clutter.EventType.TOUCH_BEGIN"));
        assert!(METHODS_TEMPLATE
            .contains("(type === Clutter.EventType.BUTTON_PRESS && event.get_button() === 1)"));
        assert!(METHODS_TEMPLATE.contains("return Clutter.EVENT_PROPAGATE;"));
    }

    #[test]
    fn window_minutes_is_baked_into_the_injected_source() {
        let (patched, _) = patch_extension_js(&upstream(), 90).expect("patch");
        assert!(patched.contains("const patchedMinutes = 90;"));
        assert!(!patched.contains(WINDOW_PLACEHOLDER));
        // The live override keeps the documented name and bounds.
        assert!(patched.contains(r#"GLib.getenv("U2TSSM")"#));
        assert!(patched.contains(&format!("parsed <= {MAX_WINDOW_MINUTES}")));
    }

    #[test]
    fn reconfiguring_the_window_rewrites_an_installed_body() {
        let (sixty, _) = patch_extension_js(&upstream(), 60).expect("patch");
        let (ninety, changed) = patch_extension_js(&sixty, 90).expect("reconfigure");
        assert!(changed);
        assert!(ninety.contains("const patchedMinutes = 90;"));
        assert!(!ninety.contains("const patchedMinutes = 60;"));
        // One block, not two: the rewrite replaces rather than appends.
        assert_eq!(ninety.matches(METHODS_START).count(), 1);
    }

    #[test]
    fn out_of_range_windows_are_rejected() {
        assert!(matches!(
            patch_extension_js(&upstream(), 0),
            Err(ProcessPopupError::WindowOutOfRange)
        ));
        assert!(matches!(
            patch_extension_js(&upstream(), MAX_WINDOW_MINUTES + 1),
            Err(ProcessPopupError::WindowOutOfRange)
        ));
    }

    #[test]
    fn re_running_the_patch_changes_nothing() {
        let once = patched_upstream();
        let (twice, changed) = patch_extension_js(&once, DEFAULT_WINDOW_MINUTES).expect("second");
        assert!(!changed);
        assert_eq!(once, twice);
    }

    #[test]
    fn first_generation_install_is_upgraded_in_place() {
        // An installed tree carrying the old header, the double-toggling click
        // case and the old tooltip must converge on the current shape without
        // gaining a duplicated method block.
        let legacy = format!(
            "{PANEL_MENU_IMPORT}\n{POPUP_MENU_IMPORT}\n\n\
             {NEW_INIT_TAIL}\n\
             {NEW_DESTROY_HEAD}\n\
             {LEGACY_METHODS_START}\n    // stale body\n\
             {CLICK_MANAGER_ANCHOR}\n\
             {LEGACY_CLICK}\n\
             {NEW_KEY}\n\
             const tooltip = {LEGACY_TOOLTIP};\n"
        );
        let (upgraded, changed) =
            patch_extension_js(&legacy, DEFAULT_WINDOW_MINUTES).expect("upgrade");
        assert!(changed);
        assert!(upgraded.contains(NEW_CLICK));
        assert!(upgraded.contains(NEW_TOOLTIP));
        assert!(!upgraded.contains(LEGACY_CLICK));
        assert!(!upgraded.contains(LEGACY_TOOLTIP));
        assert!(!upgraded.contains(LEGACY_METHODS_START));
        assert!(!upgraded.contains("// stale body"));
        assert_eq!(upgraded.matches(METHODS_START).count(), 1);

        let (again, changed_again) =
            patch_extension_js(&upgraded, DEFAULT_WINDOW_MINUTES).expect("idempotent");
        assert!(!changed_again);
        assert_eq!(upgraded, again);
    }

    #[test]
    fn missing_panel_menu_import_fails_fast() {
        assert!(matches!(
            patch_extension_js("// unrelated", DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingPanelMenuImport)
        ));
    }

    #[test]
    fn missing_click_manager_fails_fast() {
        assert!(matches!(
            patch_extension_js(PANEL_MENU_IMPORT, DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingClickManager)
        ));
    }

    #[test]
    fn missing_click_and_key_cases_fail_fast() {
        let no_click = format!("{PANEL_MENU_IMPORT}\n{CLICK_MANAGER_ANCHOR}\n{OLD_KEY}\n");
        assert!(matches!(
            patch_extension_js(&no_click, DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingLeftClickCase)
        ));

        let no_key = format!("{PANEL_MENU_IMPORT}\n{CLICK_MANAGER_ANCHOR}\n{OLD_CLICK}\n");
        assert!(matches!(
            patch_extension_js(&no_key, DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingKeyboardCase)
        ));
    }

    #[test]
    fn missing_sampler_hooks_fail_fast() {
        let no_init = format!(
            "{PANEL_MENU_IMPORT}\n{CLICK_MANAGER_ANCHOR}\n{OLD_CLICK}\n{OLD_KEY}\n\
             const tooltip = {OLD_TOOLTIP};\n"
        );
        assert!(matches!(
            patch_extension_js(&no_init, DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingInitHook)
        ));

        let no_destroy = format!(
            "{PANEL_MENU_IMPORT}\n{OLD_INIT_TAIL}\n{CLICK_MANAGER_ANCHOR}\n\
             {OLD_CLICK}\n{OLD_KEY}\nconst tooltip = {OLD_TOOLTIP};\n"
        );
        assert!(matches!(
            patch_extension_js(&no_destroy, DEFAULT_WINDOW_MINUTES),
            Err(ProcessPopupError::MissingDestroyHook)
        ));
    }

    #[test]
    fn run_is_idempotent_on_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("extension.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path, DEFAULT_WINDOW_MINUTES), 0);
        let first = fs::read_to_string(&path).expect("read");
        assert_eq!(run(&path, DEFAULT_WINDOW_MINUTES), 0);
        assert_eq!(first, fs::read_to_string(&path).expect("read"));
    }

    #[test]
    fn run_rejects_an_out_of_range_window() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("extension.js");
        fs::write(&path, upstream()).expect("write");
        assert_eq!(run(&path, MAX_WINDOW_MINUTES + 1), 1);
        assert_eq!(fs::read_to_string(&path).expect("read"), upstream());
    }
}
