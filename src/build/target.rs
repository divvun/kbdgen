pub struct TargetBuild {
    steps: Vec<Box<dyn BuildStep>>,
}

pub trait BuildStep {
    fn build(&self);
}
