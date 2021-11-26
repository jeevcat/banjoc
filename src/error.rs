pub type Result<T> = std::result::Result<T, LoxError>;
pub enum LoxError {
    CompileError,
    RuntimeError,
}
