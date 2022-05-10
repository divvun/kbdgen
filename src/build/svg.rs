use std::path::{Path, PathBuf};
//use std::rc::Rc;
use std::str::FromStr;

use async_trait::async_trait;
use xmlem::Document;

use crate::bundle::KbdgenBundle;

use super::{BuildStep, BuildSteps};

const SVG_EXT: &str = "svg";
static KEYBOARD_SVG: &str = include_str!("../../resources/template-iso-keyboard.svg");

//pub struct SvgFile {}

pub struct SvgBuild {
    pub bundle: KbdgenBundle,
    pub output_path: PathBuf,
    pub steps: Vec<Box<dyn BuildStep>>,
}

#[async_trait(?Send)]
impl BuildSteps for SvgBuild {
    fn new(bundle: KbdgenBundle, output_path: PathBuf) -> Self {
        let steps = vec![];

        Self {
            bundle,
            output_path,
            steps,
        }
    }

    fn steps(&self) -> &[Box<dyn BuildStep>] {
        &self.steps
    }

    fn bundle(&self) -> &KbdgenBundle {
        &self.bundle
    }

    fn output_path(&self) -> &Path {
        &self.output_path
    }
}

pub struct GenerateSvg {}

#[async_trait(?Send)]
impl BuildStep for GenerateSvg {
    async fn build(&self, bundle: &KbdgenBundle, _output_path: &Path) {
        let _document = Document::from_str(KEYBOARD_SVG).unwrap();

        println!("no explosion?");

        // .svg files need to be generated in cases of windows, chromeOS, and macOS
        // we'll start with Windows first
        for (_language_tag, layout) in &bundle.layouts {
            if let Some(_windows_layout) = &layout.windows {
                //let cloned_template = document.clone();

                //let svg_path = output_path.join(format!("{}.{}", language_tag, SVG_EXT));

                //std::fs::write(svg_path, cloned_template.to_string()).unwrap();
            }
        }
    }
}

/*
if let Some(windows_layout) = &layout.windows {
    let layers = &windows_layout.primary.layers;

    // Next steps: layer processing
    // original impl was returning keys from an iterator linked to modes indexed by said keys
    // i.e., IsoKey ->

    let klc_file = KlcFile {
        keyboard_name: language_tag.to_string(),
    };

    let klc_bytes = klc_file.to_string().encode_utf16_le_bom();
    let klc_path = output_path.join(format!("{}.{}", klc_file.keyboard_name, KLC_EXT));
    std::fs::write(klc_path, klc_bytes).unwrap();
}
*/
