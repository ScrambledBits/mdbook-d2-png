use std::path::PathBuf;

use serde::Deserialize;

/// Default path to the D2 binary
fn default_bin_path() -> PathBuf {
    PathBuf::from("d2")
}

/// Default output directory for generated diagrams
fn default_output_dir() -> PathBuf {
    PathBuf::from("d2")
}

/// Default value for inline mode
const fn default_inline() -> bool {
    false
}

#[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Fonts {
    pub regular: PathBuf,
    pub italic: PathBuf,
    pub bold: PathBuf,
}
#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The path to the d2 binary
    #[serde(default = "default_bin_path")]
    pub path: PathBuf,

    #[serde(default = "default_output_dir")]
    pub output_dir: PathBuf,

    pub layout: Option<String>,

    /// Whether to inline PNG images as base64 data URIs
    ///
    /// When `true`, images are embedded directly in the HTML.
    /// When `false` (default), images are saved as separate `.png` files.
    #[serde(default = "default_inline")]
    pub inline: bool,

    /// Custom font path
    ///
    /// Only ttf fonts are valid
    pub fonts: Option<Fonts>,

    pub theme_id: Option<String>,
    pub dark_theme_id: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: default_bin_path(),
            layout: None,
            output_dir: default_output_dir(),
            inline: default_inline(),
            fonts: None,
            theme_id: None,
            dark_theme_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use test_case::test_case;

    use super::Config;

    #[test_case(""; "empty")]
    #[test_case(
        r#"
path = "d2"
layout = "dagre"
output-dir = "d2"
"#
        ; "defaults"
    )]
    fn compatible(input: &str) {
        let _config: Config = toml::from_str(input).expect("config is not compatible");
    }

    #[test_case("" => Config::default(); "default")]
    #[test_case(
        r#"
path = "/custom/bin/d2"
layout = "elk"
output-dir = "d2-img"
"#
    => Config {
        path: PathBuf::from("/custom/bin/d2"),
        layout: Some(String::from("elk")),
        inline: false,
        output_dir: PathBuf::from("d2-img"),
        fonts: None,
        theme_id: None,
        dark_theme_id:None,
    }
        ; "custom"
    )]
    fn parse(input: &str) -> Config {
        toml::from_str(input).unwrap()
    }
}
