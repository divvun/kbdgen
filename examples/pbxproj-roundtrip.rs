use std::path::Path;

use kbdgen::build::ios::pbxproj::Pbxproj;

fn main() {
    let filename = std::env::args().skip(1).next().expect("Path to .pbxproj");

    let pbxproj = Pbxproj::from_path(Path::new(&filename));
    println!("{}", pbxproj.to_pbxproj_string())

    // let doc = Document::from_file(std::fs::File::open(filename).unwrap()).unwrap();

    // let key_layouts = doc
    //     .root()
    //     .query_selector_all(&doc, &Selector::new("keyMap").unwrap())
    //     .iter()
    //     .map(|x| {
    //         let mut v = x
    //             .children(&doc)
    //             .iter()
    //             .map(|x| x.attribute(&doc, "code").unwrap().parse::<u32>().unwrap())
    //             .collect::<Vec<_>>();
    //         v.sort();
    //         v
    //     })
    //     .collect::<Vec<_>>();
    // println!("{:#?}", key_layouts);
}
