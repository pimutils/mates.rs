use std::ops::Deref;

use cursive::Cursive;
use cursive::traits::*;
use cursive::views;

use vobject;

struct FormattedNameEditor {
    original_prop: Option<vobject::Property>
}

impl FormattedNameEditor {
    pub fn pop_from_vobject(vobj: &mut vobject::Component) -> (Self, views::IdView<views::EditView>) {
        let prop = vobj.pop("FN");
        let content = match prop {
            Some(ref p) => p.value_as_string(),
            None => "".to_owned()
        };

        (FormattedNameEditor { original_prop: prop }, 
         views::EditView::new().content(content).with_id("FN"))
    }

    pub fn push_to_vobject(&self, siv: &mut Cursive, vobj: &mut vobject::Component) {
        let v = siv.find_id::<views::EditView>("FN").unwrap();
        let content = v.get_content();
        let new_prop = match self.original_prop {
            Some(ref x) => {
                let mut nx = x.clone();
                nx.raw_value = vobject::escape_chars(content.as_str());
                nx
            },
            None => vobject::Property::new("FN", content.as_str())
        };
        vobj.push(new_prop);
    }
}


fn mprops_to_view(props: Vec<vobject::Property>) -> views::TextArea {
    let mut edit_text = String::new();
    for prop in props {
        if let Some(prop_type) = prop.params.get("TYPE") {
            edit_text.push_str(prop_type);
            edit_text.push(' ');
        }
        edit_text.push_str(&prop.value_as_string());
        edit_text.push('\n');
    }

    views::TextArea::new().content(edit_text)
}

fn view_to_mprops<V: Deref<Target=views::TextArea>>(v: V, prop_name: &str, vobj: &mut vobject::Component) {
    for line in v.get_content().split('\n') {
        let mut split = line.rsplitn(2, ' ');
        let mut prop = match split.next() {
            Some(x) if !x.trim().is_empty() => vobject::Property::new(prop_name, x),
            _ => continue,
        };
        if let Some(prop_type) = split.next() {
            prop.params.insert("TYPE".to_owned(), prop_type.to_owned());
        };
        vobj.push(prop);
    }
}

struct EmailsEditor;

impl EmailsEditor {
    pub fn pop_from_vobject(vobj: &mut vobject::Component) -> (Self, views::IdView<views::TextArea>) {
        let props = vobj.props.remove("EMAIL").unwrap_or_default();
        (EmailsEditor, mprops_to_view(props).with_id("emails"))
    }

    pub fn push_to_vobject(&self, siv: &mut Cursive, vobj: &mut vobject::Component) {
        let v = siv.find_id::<views::TextArea>("emails").unwrap();
        view_to_mprops(v, "EMAIL", vobj);
    }
}

struct TelEditor;

impl TelEditor {
    pub fn pop_from_vobject(vobj: &mut vobject::Component) -> (Self, views::IdView<views::TextArea>) {
        let props = vobj.props.remove("TEL").unwrap_or_default();
        (TelEditor, mprops_to_view(props).with_id("tels"))
    }

    pub fn push_to_vobject(&self, siv: &mut Cursive, vobj: &mut vobject::Component) {
        let v = siv.find_id::<views::TextArea>("tels").unwrap();
        view_to_mprops(v, "TEL", vobj);
    }
}

pub struct VcardEditor {
    vobj: vobject::Component,
    fn_field: FormattedNameEditor,
    email_field: EmailsEditor,
    tel_field: TelEditor
}

impl VcardEditor {
    pub fn new(mut vobj: vobject::Component) -> (Self, views::BoxView<views::LinearLayout>) {
        let (fn_field, fn_view) = FormattedNameEditor::pop_from_vobject(&mut vobj);
        let (email_field, email_view) = EmailsEditor::pop_from_vobject(&mut vobj);
        let (tel_field, tel_view) = TelEditor::pop_from_vobject(&mut vobj);

        let main_col = views::LinearLayout::vertical()
            .child(views::Panel::new(views::LinearLayout::vertical()
                                     .child(views::TextView::new("Formatted Name:"))
                                     .child(fn_view)))
            .child(views::Panel::new(views::LinearLayout::vertical()
                                     .child(views::TextView::new("Hit ^C to abort, or "))
                                     .child(views::Button::new("Save", |s| s.quit()))));

        let props_list = views::LinearLayout::vertical()
            .child(views::Panel::new(views::LinearLayout::vertical()
                                     .child(views::TextView::new("Email addresses: (type + email)"))
                                     .child(email_view)))
            .child(views::Panel::new(views::LinearLayout::vertical()
                                     .child(views::TextView::new("Telephone numbers: (type + nr)"))
                                     .child(tel_view)));

        let cols = views::LinearLayout::horizontal()
            .child(main_col)
            .child(props_list)
            .full_screen();

        let rv = VcardEditor {
            vobj,
            fn_field,
            email_field,
            tel_field
        };

        (rv, cols)
    }

    pub fn to_vobject(mut self, siv: &mut Cursive) -> vobject::Component {
        self.fn_field.push_to_vobject(siv, &mut self.vobj);
        self.email_field.push_to_vobject(siv, &mut self.vobj);
        self.tel_field.push_to_vobject(siv, &mut self.vobj);
        self.vobj
    }
}
