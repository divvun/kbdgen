use std::sync::Arc;

use crate::bundle::KbdgenBundle;

use super::windows::klc::KlcFile;

pub trait BuildSteps {
    fn populate_steps(&mut self);
    fn count(&self) -> usize;
    fn build_full(&self);
}

pub trait BuildStep {
    fn build(&self, bundle: Arc<KbdgenBundle>);
}



pub struct WindowsBuild {
    pub bundle: Arc<KbdgenBundle>,
    pub steps: Vec<Box<dyn BuildStep>>,
}

impl BuildSteps for WindowsBuild {
    fn populate_steps(&mut self) {
        &self.steps.push(Box::new(GenerateKlc {}));
        &self.steps.push(Box::new(Print {}));
    }

    fn count(&self) -> usize {
        *&self.steps.len()
    }

    fn build_full(&self) {
        &self.steps.iter().for_each(|step| {
            step.build(self.bundle.clone());
        });
    }
}



pub struct GenerateKlc {

}

impl BuildStep for GenerateKlc {
    fn build(&self, bundle: Arc<KbdgenBundle>) {

        let klc_file = KlcFile {

        };
    }
}

pub struct Print {

}

impl BuildStep for Print {
    fn build(&self, _bundle: Arc<KbdgenBundle>) {
        println!("print step");
    } 
}