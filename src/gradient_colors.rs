//! Pure gradient color math for the Resource Monitor patches.
//!
//! Canonical, unit-tested mirror of the JavaScript injected by `patch_colors`
//! and `patch_disk`. GNOME
//! Shell (GJS) cannot import this crate, so `patch_colors` and `patch_disk`
//! embed inline JavaScript copies of the same math; keeping this tested source
//! alongside them documents and verifies the intended behavior, and owns the
//! range constants interpolated into the injected support block.
//!
//! Gradient scheme: each metric maps a value within `[min_val, max_val]` onto a
//! green -> yellow -> red gradient, where green is healthy and red is critical.

// The math mirrors an injected JavaScript block; several helpers exist so the
// behavior stays covered by tests even when no Rust caller uses them directly.
#![allow(dead_code)]

/// Maximum Ethernet/WLAN throughput for color gradient, in displayed Mbps.
pub const ETHERNET_MAX_MBPS: i64 = 2000;

/// Maximum RAM for color gradient, in GB.
pub const RAM_MAX_GB: i64 = 64;

/// Maximum disk usage percentage for color gradient.
pub const DISK_USAGE_MAX_PERCENT: i64 = 100;

/// Maximum GPU memory (VRAM) for color gradient, in GB.
pub const GPU_MEMORY_MAX_GB: i64 = 24;

/// Gradient configuration for a single indicator type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientConfig {
    /// Minimum threshold value (mapped to `start_rgb`).
    pub min_val: f64,
    /// Maximum threshold value (mapped to `end_rgb`).
    pub max_val: f64,
    /// Healthy end of the gradient.
    pub start_rgb: [f64; 3],
    /// Critical end of the gradient.
    pub end_rgb: [f64; 3],
    /// When true the value is mirrored inside the range before interpolation.
    pub inverted: bool,
}

const GREEN: [f64; 3] = [0.0, 255.0, 0.0];
const RED: [f64; 3] = [255.0, 0.0, 0.0];

/// Gradient configuration per indicator type, mirroring `GRADIENT_CONFIGS`.
pub const CPU_CONFIG: GradientConfig = GradientConfig {
    min_val: 0.0,
    max_val: 100.0,
    start_rgb: GREEN,
    end_rgb: RED,
    inverted: false,
};

/// RAM gradient: 0 GB green through `RAM_MAX_GB` red.
pub const RAM_CONFIG: GradientConfig = GradientConfig {
    min_val: 0.0,
    max_val: RAM_MAX_GB as f64,
    start_rgb: GREEN,
    end_rgb: RED,
    inverted: false,
};

/// Disk-space gradient: 0% used green through `DISK_USAGE_MAX_PERCENT` red.
pub const DISK_SPACE_CONFIG: GradientConfig = GradientConfig {
    min_val: 0.0,
    max_val: DISK_USAGE_MAX_PERCENT as f64,
    start_rgb: GREEN,
    end_rgb: RED,
    inverted: false,
};

/// Ethernet gradient: 0 Mbps green through `ETHERNET_MAX_MBPS` red.
pub const ETH_CONFIG: GradientConfig = GradientConfig {
    min_val: 0.0,
    max_val: ETHERNET_MAX_MBPS as f64,
    start_rgb: GREEN,
    end_rgb: RED,
    inverted: false,
};

/// WLAN gradient, identical to the Ethernet gradient.
pub const WLAN_CONFIG: GradientConfig = ETH_CONFIG;

/// GPU usage gradient: 0% green through 100% red.
pub const GPU_CONFIG: GradientConfig = CPU_CONFIG;

/// GPU memory gradient: 0 GB green through `GPU_MEMORY_MAX_GB` red.
pub const GPU_MEMORY_CONFIG: GradientConfig = GradientConfig {
    min_val: 0.0,
    max_val: GPU_MEMORY_MAX_GB as f64,
    start_rgb: GREEN,
    end_rgb: RED,
    inverted: false,
};

/// JavaScript `Math.round` semantics: halves round toward positive infinity.
fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// JavaScript `Math.max(0, Math.min(limit, value))`, propagating NaN like JS.
fn js_clamp(value: f64, limit: f64) -> f64 {
    if value.is_nan() {
        return f64::NAN;
    }
    if value < 0.0 {
        0.0
    } else if value > limit {
        limit
    } else {
        value
    }
}

/// Convert RGB components to a CSS color string, clamping to 0-255 and rounding.
///
/// # Parameters
/// - `r`, `g`, `b`: Channel intensities; may be fractional or out of range.
///
/// Returns a style string such as `color: rgb(0, 255, 0);`.
pub fn rgb_to_style(r: f64, g: f64, b: f64) -> String {
    let rr = js_clamp(js_round(r), 255.0) as i64;
    let gg = js_clamp(js_round(g), 255.0) as i64;
    let bb = js_clamp(js_round(b), 255.0) as i64;
    format!("color: rgb({rr}, {gg}, {bb});")
}

/// Linearly interpolate a gradient color between two RGB endpoints.
///
/// # Parameters
/// - `value`: Current value; non-finite input yields an empty string.
/// - `min_val`, `max_val`: Range the value is normalized against.
/// - `start_rgb`, `end_rgb`: Gradient endpoints.
///
/// Returns a CSS color style string, or `""` when `value` is non-finite.
pub fn get_gradient_color(
    value: f64,
    min_val: f64,
    max_val: f64,
    start_rgb: [f64; 3],
    end_rgb: [f64; 3],
) -> String {
    if !value.is_finite() {
        return String::new();
    }

    let ratio = (value - min_val) / (max_val - min_val);
    let clamped_ratio = js_clamp(ratio, 1.0);
    let r = start_rgb[0] + (end_rgb[0] - start_rgb[0]) * clamped_ratio;
    let g = start_rgb[1] + (end_rgb[1] - start_rgb[1]) * clamped_ratio;
    let b = start_rgb[2] + (end_rgb[2] - start_rgb[2]) * clamped_ratio;

    rgb_to_style(r, g, b)
}

/// Compute a green -> yellow -> red gradient passing through bright yellow at
/// the midpoint instead of a dark olive.
///
/// # Parameters
/// - `value`: Current value within `min_val..max_val`.
/// - `min_val`, `max_val`: Range the value is normalized against.
/// - `start_rgb`, `end_rgb`: Healthy and critical endpoints.
///
/// Returns a CSS color style string.
pub fn get_green_yellow_red_gradient_color(
    value: f64,
    min_val: f64,
    max_val: f64,
    start_rgb: [f64; 3],
    end_rgb: [f64; 3],
) -> String {
    let midpoint = min_val + (max_val - min_val) / 2.0;
    let midpoint_rgb = [255.0, 255.0, 0.0];

    if value <= midpoint {
        return get_gradient_color(value, min_val, midpoint, start_rgb, midpoint_rgb);
    }

    get_gradient_color(value, midpoint, max_val, midpoint_rgb, end_rgb)
}

/// Select the gradient config for a joined colors string containing type markers.
///
/// The injected JavaScript first compares the `colors` argument against the
/// indicator's own color arrays by object identity; that check has no Rust
/// equivalent, so this port implements the marker fallback chain only, in the
/// same priority order.
///
/// # Parameters
/// - `color_str`: Colors joined with spaces, e.g. `"#00ff00 __cpu"`.
pub fn select_gradient_config(color_str: &str) -> GradientConfig {
    if color_str.contains("__eth") {
        ETH_CONFIG
    } else if color_str.contains("__wlan") {
        WLAN_CONFIG
    } else if color_str.contains("__gpuMem") {
        GPU_MEMORY_CONFIG
    } else if color_str.contains("__diskSpace") {
        DISK_SPACE_CONFIG
    } else if color_str.contains("__gpu") {
        GPU_CONFIG
    } else if color_str.contains("__ram") {
        RAM_CONFIG
    } else {
        CPU_CONFIG
    }
}

/// Gradient replacement for the extension's `_getUsageColor` method.
///
/// # Parameters
/// - `values`: Metric samples; the maximum finite sample is colored. Pass a
///   single-element slice for scalar metrics.
/// - `colors`: Color thresholds, possibly carrying type markers such as `__cpu`.
///
/// Returns a CSS style string, or `""` when no finite sample is present.
pub fn gradient_get_usage_color(values: &[f64], colors: &[&str]) -> String {
    let numeric_value = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !numeric_value.is_finite() {
        return String::new();
    }

    let config = select_gradient_config(&colors.join(" "));
    let effective_value = if config.inverted {
        config.max_val - numeric_value
    } else {
        numeric_value
    };

    get_green_yellow_red_gradient_color(
        effective_value,
        config.min_val,
        config.max_val,
        config.start_rgb,
        config.end_rgb,
    )
}

/// Build a green -> yellow -> red CSS style for a disk usage percentage.
///
/// Unlike [`get_green_yellow_red_gradient_color`] this is a fixed 0-100 scale
/// emitting a `color: rgb(r, g, 0);` form. Mirrors the helper injected into
/// `refreshers.js` by the disk patcher.
///
/// # Parameters
/// - `value`: Usage percentage (0-100). Non-finite values yield `""`.
pub fn get_disk_usage_percent_style(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }

    let ratio = js_clamp(value / 100.0, 1.0);
    let red = if ratio <= 0.5 {
        js_round(510.0 * ratio) as i64
    } else {
        255
    };
    let green = if ratio <= 0.5 {
        255
    } else {
        js_round(510.0 * (1.0 - ratio)) as i64
    };

    format!("color: rgb({red}, {green}, 0);")
}

/// Build a red -> yellow -> green CSS style for available filesystem space.
///
/// The filesystem's current total capacity defines the upper endpoint, so the
/// scale remains correct for disks of any size: no free space is red, half of
/// the capacity free is yellow, and the entire capacity free is green.
///
/// # Parameters
/// - `available`: Available space in any byte/unit scale.
/// - `total`: Total capacity in the same scale. Non-positive or non-finite
///   totals yield `""`.
pub fn get_disk_free_space_style(available: f64, total: f64) -> String {
    if !available.is_finite() || !total.is_finite() || total <= 0.0 {
        return String::new();
    }

    let ratio = js_clamp(available / total, 1.0);
    let red = if ratio <= 0.5 {
        255
    } else {
        js_round(510.0 * (1.0 - ratio)) as i64
    };
    let green = if ratio <= 0.5 {
        js_round(510.0 * ratio) as i64
    } else {
        255
    };

    format!("color: rgb({red}, {green}, 0);")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_to_style_rounds_and_clamps() {
        assert_eq!(rgb_to_style(0.0, 255.0, 0.0), "color: rgb(0, 255, 0);");
        assert_eq!(rgb_to_style(-10.0, 300.0, 12.4), "color: rgb(0, 255, 12);");
        assert_eq!(rgb_to_style(12.5, 0.0, 0.0), "color: rgb(13, 0, 0);");
    }

    #[test]
    fn gradient_endpoints_and_clamping() {
        let green = [0.0, 255.0, 0.0];
        let red = [255.0, 0.0, 0.0];
        assert_eq!(
            get_gradient_color(0.0, 0.0, 100.0, green, red),
            "color: rgb(0, 255, 0);"
        );
        assert_eq!(
            get_gradient_color(100.0, 0.0, 100.0, green, red),
            "color: rgb(255, 0, 0);"
        );
        assert_eq!(
            get_gradient_color(500.0, 0.0, 100.0, green, red),
            "color: rgb(255, 0, 0);"
        );
        assert_eq!(get_gradient_color(f64::NAN, 0.0, 100.0, green, red), "");
    }

    #[test]
    fn green_yellow_red_passes_through_bright_yellow() {
        let green = [0.0, 255.0, 0.0];
        let red = [255.0, 0.0, 0.0];
        assert_eq!(
            get_green_yellow_red_gradient_color(50.0, 0.0, 100.0, green, red),
            "color: rgb(255, 255, 0);"
        );
        assert_eq!(
            get_green_yellow_red_gradient_color(0.0, 0.0, 100.0, green, red),
            "color: rgb(0, 255, 0);"
        );
        assert_eq!(
            get_green_yellow_red_gradient_color(100.0, 0.0, 100.0, green, red),
            "color: rgb(255, 0, 0);"
        );
    }

    #[test]
    fn config_ranges_match_the_javascript_source() {
        assert_eq!(ETHERNET_MAX_MBPS, 2000);
        assert_eq!(RAM_MAX_GB, 64);
        assert_eq!(DISK_USAGE_MAX_PERCENT, 100);
        assert_eq!(GPU_MEMORY_MAX_GB, 24);
        assert_eq!(RAM_CONFIG.max_val, 64.0);
        assert_eq!(ETH_CONFIG.max_val, 2000.0);
        assert_eq!(GPU_MEMORY_CONFIG.max_val, 24.0);
    }

    #[test]
    fn marker_selection_follows_javascript_priority() {
        assert_eq!(select_gradient_config("__eth"), ETH_CONFIG);
        assert_eq!(select_gradient_config("__wlan"), WLAN_CONFIG);
        assert_eq!(select_gradient_config("__gpuMem"), GPU_MEMORY_CONFIG);
        assert_eq!(select_gradient_config("__diskSpace"), DISK_SPACE_CONFIG);
        assert_eq!(select_gradient_config("__gpu"), GPU_CONFIG);
        assert_eq!(select_gradient_config("__ram"), RAM_CONFIG);
        assert_eq!(select_gradient_config("__cpu"), CPU_CONFIG);
        assert_eq!(select_gradient_config("#ffffff"), CPU_CONFIG);
        // "__gpuMem" must win over the shorter "__gpu" prefix.
        assert_eq!(select_gradient_config("__gpuMem __gpu"), GPU_MEMORY_CONFIG);
    }

    #[test]
    fn usage_color_takes_the_max_finite_sample() {
        assert_eq!(
            gradient_get_usage_color(&[0.0, 50.0, f64::NAN], &["__cpu"]),
            "color: rgb(255, 255, 0);"
        );
        assert_eq!(gradient_get_usage_color(&[f64::NAN], &["__cpu"]), "");
        assert_eq!(gradient_get_usage_color(&[], &["__cpu"]), "");
    }

    #[test]
    fn disk_free_space_uses_the_filesystem_capacity_as_its_gradient_range() {
        assert_eq!(
            get_disk_free_space_style(0.0, 512.0),
            "color: rgb(255, 0, 0);"
        );
        assert_eq!(
            get_disk_free_space_style(256.0, 512.0),
            "color: rgb(255, 255, 0);"
        );
        assert_eq!(
            get_disk_free_space_style(512.0, 512.0),
            "color: rgb(0, 255, 0);"
        );
        assert_eq!(
            get_disk_free_space_style(19.0, 512.0),
            "color: rgb(255, 19, 0);"
        );
        assert_eq!(get_disk_free_space_style(19.0, 0.0), "");
        assert_eq!(get_disk_free_space_style(f64::NAN, 512.0), "");
    }

    #[test]
    fn ram_uses_its_own_gigabyte_range() {
        // 32 GB is the RAM midpoint, so it renders bright yellow.
        assert_eq!(
            gradient_get_usage_color(&[32.0], &["__ram"]),
            "color: rgb(255, 255, 0);"
        );
    }

    #[test]
    fn disk_usage_percent_style_is_bright_at_the_midpoint() {
        assert_eq!(get_disk_usage_percent_style(0.0), "color: rgb(0, 255, 0);");
        assert_eq!(
            get_disk_usage_percent_style(50.0),
            "color: rgb(255, 255, 0);"
        );
        assert_eq!(
            get_disk_usage_percent_style(100.0),
            "color: rgb(255, 0, 0);"
        );
        assert_eq!(get_disk_usage_percent_style(f64::NAN), "");
    }
}
