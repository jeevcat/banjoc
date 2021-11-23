pub type Result = std::result::Result<(), LoxError>;
pub enum LoxError {
    CompileError,
    RuntimeError,
}
