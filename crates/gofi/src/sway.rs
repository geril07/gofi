//! Sway IPC: ask for the focused output's width before the window maps.
//!
//! The launcher is half the output wide. Without the query the window opens at
//! `PANEL_W`, and Sway's 50% rule then resizes the first frame — on a scaled
//! output that is a visible width change. The query is best-effort: outside
//! Sway, or without `swaymsg`, the compiled default is used.

use std::process::Command;

/// Half the focused output's logical width, or `None` when it cannot be read.
pub fn panel_width() -> Option<f32> {
    std::env::var_os("SWAYSOCK")?;
    let output = Command::new("swaymsg")
        .args(["-t", "get_outputs", "-r"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    focused_output_width(&output.stdout)
}

fn focused_output_width(json: &[u8]) -> Option<f32> {
    let outputs: serde_json::Value = serde_json::from_slice(json).ok()?;
    let focused = outputs
        .as_array()?
        .iter()
        .find(|output| output["focused"].as_bool() == Some(true))?;
    let width = focused["rect"]["width"].as_f64()? as f32;
    (width > 0.0).then_some(width / 2.0)
}

#[cfg(test)]
mod tests {
    use super::focused_output_width;

    const TWO_OUTPUTS: &[u8] = br#"[
        {"name":"eDP-1","focused":false,"scale":1.5,"rect":{"x":0,"y":0,"width":1706,"height":1066}},
        {"name":"HDMI-A-1","focused":true,"scale":1.0,"rect":{"x":1706,"y":0,"width":1920,"height":1080}}
    ]"#;

    #[test]
    fn focused_output_width_uses_the_focused_output() {
        assert_eq!(focused_output_width(TWO_OUTPUTS), Some(960.0));
    }

    #[test]
    fn focused_output_width_rejects_bad_input() {
        assert_eq!(focused_output_width(b"[]"), None);
        assert_eq!(focused_output_width(b"not json"), None);
        assert_eq!(
            focused_output_width(br#"[{"focused":true,"rect":{"width":0,"height":1080}}]"#),
            None
        );
        assert_eq!(
            focused_output_width(br#"[{"focused":true,"rect":{"width":1280,"height":720}}]"#),
            Some(640.0)
        );
    }
}
