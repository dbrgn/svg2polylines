#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SVG parse error: {0}")]
    SvgParse(String),
    #[error("Could not simplify SVG with usvg: {0}")]
    Usvg(#[from] usvg::Error),
    #[error("SVG path parse error: {0}")]
    PathParse(String),
    #[error("Polyline error: {0}")]
    Polyline(String),
    #[error("Transform error: {0}")]
    Transform(String),
}
