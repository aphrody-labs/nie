use @ident@::{View, document};

#[test]
fn escapes_and_nests() {
    let v = View::Column(vec![View::Text("a<b".to_owned())]);
    assert_eq!(v.to_html(), "<div><p>a&lt;b</p></div>");
}

#[test]
fn document_names_the_app() {
    let d = document();
    assert!(d.starts_with("<!doctype html>"));
    assert!(d.contains("<h1>@name@</h1>"));
}
