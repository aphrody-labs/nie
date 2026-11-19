#[test]
fn add_wraps() {
    assert_eq!(@ident@::add(2, 3), 5);
    assert_eq!(@ident@::add(i32::MAX, 1), i32::MIN);
}
