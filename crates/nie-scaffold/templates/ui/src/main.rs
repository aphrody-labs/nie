fn main() -> std::io::Result<()> {
    std::fs::create_dir_all("dist")?;
    std::fs::write("dist/index.html", @ident@::document())
}
