pub type Result<T> = std::result::Result<T, LoxError>;
#[derive(Debug)]
pub enum LoxError {
    CompileError(&'static str),
    RuntimeError,
}
