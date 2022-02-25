pub type Result<T> = std::result::Result<T, BanjoError>;
#[derive(Debug)]
pub enum BanjoError {
    CompileError(String),
    CompileErrors(Vec<String>),
    RuntimeError(String),
}

impl BanjoError {
    pub fn compile<S: Into<String>>(msg: S) -> Self {
        Self::CompileError(msg.into())
    }
    pub fn compile_err<T, S: Into<String>>(msg: S) -> Result<T> {
        Err(Self::compile(msg))
    }
    pub fn append(&mut self, other: Self) {
        match self {
            BanjoError::CompileError(this) => match other {
                BanjoError::CompileError(other) => {
                    *self = Self::CompileErrors(vec![this.to_string(), other])
                }
                BanjoError::CompileErrors(mut other) => {
                    other.push(this.to_string());
                    *self = Self::CompileErrors(other)
                }
                _ => {}
            },
            BanjoError::CompileErrors(this) => match other {
                BanjoError::CompileError(other) => this.push(other),
                BanjoError::CompileErrors(mut other) => this.append(&mut other),
                _ => {}
            },
            _ => {}
        }
    }
}

pub fn append<T>(result: &mut Result<T>, other: BanjoError) {
    match result {
        Ok(_) => *result = Err(other),
        Err(e) => e.append(other),
    }
}
