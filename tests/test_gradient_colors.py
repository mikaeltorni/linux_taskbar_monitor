#!/usr/bin/env python3
"""Tests for scripts/lib/gradient_colors.js — pure gradient color math.

This module is a tested, canonical mirror of the gradient logic that
patch_resource_monitor_colors.js injects into extension.js as a string block.
The helpers are pure, so they are exercised directly from a small Node harness.
"""

import json
import subprocess
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[1]
MODULE = ROOT_DIR / "scripts" / "lib" / "gradient_colors.js"


def _eval(expr: str) -> str:
    """Evaluate a JS expression with the module bound as `g`; return stdout."""
    script = (
        f"const g = require({json.dumps(str(MODULE))});\n"
        f"process.stdout.write(String({expr}));"
    )
    result = subprocess.run(
        ["node", "-e", script],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def test_rgb_to_style_formats_clean_values():
    assert _eval("g.rgbToStyle(0, 255, 0)") == "color: rgb(0, 255, 0);"


def test_rgb_to_style_clamps_and_rounds():
    assert _eval("g.rgbToStyle(-5, 300, 12.6)") == "color: rgb(0, 255, 13);"


def test_gradient_color_non_finite_returns_empty():
    assert _eval("g.getGradientColor(NaN, 0, 100, [0,255,0], [255,0,0])") == ""


def test_gradient_color_at_min_is_start():
    assert _eval("g.getGradientColor(0, 0, 100, [0,255,0], [255,0,0])") == "color: rgb(0, 255, 0);"


def test_gradient_color_at_max_is_end():
    assert _eval("g.getGradientColor(100, 0, 100, [0,255,0], [255,0,0])") == "color: rgb(255, 0, 0);"


def test_gradient_color_clamps_above_max():
    assert _eval("g.getGradientColor(150, 0, 100, [0,255,0], [255,0,0])") == "color: rgb(255, 0, 0);"


def test_green_yellow_red_hits_bright_yellow_at_midpoint():
    assert (
        _eval("g.getGreenYellowRedGradientColor(50, 0, 100, [0,255,0], [255,0,0])")
        == "color: rgb(255, 255, 0);"
    )


def test_green_yellow_red_below_midpoint_interpolates_to_yellow():
    # 25% -> halfway between green and yellow -> red channel ~127.
    assert (
        _eval("g.getGreenYellowRedGradientColor(25, 0, 100, [0,255,0], [255,0,0])")
        == "color: rgb(128, 255, 0);"
    )


def test_gradient_configs_define_all_indicator_types():
    keys = _eval("Object.keys(g.GRADIENT_CONFIGS).sort().join(',')")
    assert keys == "cpu,diskSpace,eth,gpu,gpuMemory,ram,wlan"


def test_usage_color_non_finite_returns_empty():
    assert _eval("g.gradientGetUsageColor({}, NaN, [])") == ""


def test_usage_color_array_takes_max():
    # eth config maxVal 2000; value max(0,0)=0 -> green.
    assert _eval("g.gradientGetUsageColor({}, [0, 0], ['__eth'])") == "color: rgb(0, 255, 0);"


def test_usage_color_selects_disk_by_identity():
    expr = (
        "(() => { const ind = {}; ind._diskSpaceColors = ['x']; "
        "return g.gradientGetUsageColor(ind, 100, ind._diskSpaceColors); })()"
    )
    # diskSpace maxVal 100 -> 100% -> red.
    assert _eval(expr) == "color: rgb(255, 0, 0);"


def test_usage_color_falls_back_to_cpu():
    assert _eval("g.gradientGetUsageColor({}, 0, [])") == "color: rgb(0, 255, 0);"


def test_disk_usage_percent_style_non_finite_returns_empty():
    assert _eval("g.getDiskUsagePercentStyle(NaN)") == ""


def test_disk_usage_percent_style_zero_is_green():
    assert _eval("g.getDiskUsagePercentStyle(0)") == "color: rgb(0, 255, 0);"


def test_disk_usage_percent_style_full_is_red():
    assert _eval("g.getDiskUsagePercentStyle(100)") == "color: rgb(255, 0, 0);"


def test_disk_usage_percent_style_midpoint_is_yellow():
    assert _eval("g.getDiskUsagePercentStyle(50)") == "color: rgb(255, 255, 0);"


def test_disk_usage_percent_style_quarter_interpolates():
    assert _eval("g.getDiskUsagePercentStyle(25)") == "color: rgb(128, 255, 0);"


if __name__ == "__main__":
    import pytest

    raise SystemExit(pytest.main([__file__, "-v"]))
