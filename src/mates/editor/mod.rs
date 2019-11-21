use std::fs;
use std::io::{Read,Write};
use std::process;
use std::path::Path;

use vobject;

use atomicwrites;

use cursive::Cursive;
use cursive::theme;

mod widgets;

use self::widgets::VcardEditor;

pub fn cli_main<P: AsRef<Path>>(filename: P) {
    let mut vobj = {
        let mut f = fs::File::open(&filename).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        vobject::parse_component(&s[..]).unwrap()
    };

    if vobj.name != "VCARD" {
        println!("Expected VCARD component, got {}", vobj.name);
        process::exit(1);
    }


    let (editor, editor_view) = VcardEditor::new(vobj);

    let mut siv = Cursive::default();
    siv.add_fullscreen_layer(editor_view);

    siv.set_theme(theme::Theme {
        shadow: false,
        borders: theme::BorderStyle::Simple,
        palette: theme::Palette::default(),
    });
    siv.run();

    vobj = editor.to_vobject(&mut siv);
    drop(siv);  // Necessary to be able to write text immediately afterwards

    let af = atomicwrites::AtomicFile::new(filename, atomicwrites::AllowOverwrite);
    af.write(|f| f.write_all(vobject::write_component(&vobj).as_bytes())).unwrap();
}
