pub type Result<T> = std::result::Result<T, LoxError>;
pub enum LoxError {
    CompileError(&'static str),
    RuntimeError,
}
