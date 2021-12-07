pub type Result<T> = std::result::Result<T, LoxError>;
pub enum LoxError {
    CompileError,
    CompileErrorMsg(&'static str),
    RuntimeError,
}
