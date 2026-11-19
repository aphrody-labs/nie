//! @name@: UI app on the nie framework. The view tree is plain data; `to_html` renders it as a
//! static document. A GPU renderer consumes the same tree.

#![forbid(unsafe_code)]

@plugin@
/// A node of the view tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    /// A heading.
    Title(String),
    /// A paragraph.
    Text(String),
    /// A vertical stack.
    Column(Vec<View>),
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl View {
    /// The HTML of this node.
    #[must_use]
    pub fn to_html(&self) -> String {
        match self {
            Self::Title(t) => format!("<h1>{}</h1>", escape(t)),
            Self::Text(t) => format!("<p>{}</p>", escape(t)),
            Self::Column(items) => {
                format!("<div>{}</div>", items.iter().map(Self::to_html).collect::<String>())
            }
        }
    }
}

/// The root view of the app.
#[must_use]
pub fn root() -> View {
    View::Column(vec![
        View::Title(env!("CARGO_PKG_NAME").to_owned()),
        View::Text("Built on the nie framework.".to_owned()),
    ])
}

/// A complete HTML document of `root`.
#[must_use]
pub fn document() -> String {
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\"><title>{}</title>\n{}\n",
        env!("CARGO_PKG_NAME"),
        root().to_html()
    )
}
