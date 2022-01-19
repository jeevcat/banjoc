pub type Result<T> = std::result::Result<T, LoxError>;
#[derive(Debug)]
pub enum LoxError {
    CompileError(&'static str),
    CompileErrors(Vec<&'static str>),
    RuntimeError(String),
}

impl LoxError {
    pub fn append(&mut self, other: Self) {
        match self {
            LoxError::CompileError(this) => match other {
                LoxError::CompileError(other) => *self = Self::CompileErrors(vec![this, other]),
                LoxError::CompileErrors(mut other) => {
                    other.push(this);
                    *self = Self::CompileErrors(other)
                }
                _ => {}
            },
            LoxError::CompileErrors(this) => match other {
                LoxError::CompileError(other) => this.push(other),
                LoxError::CompileErrors(mut other) => this.append(&mut other),
                _ => {}
            },
            _ => {}
        }
    }
}

pub fn append<T>(result: &mut Result<T>, other: LoxError) {
    match result {
        Ok(_) => *result = Err(other),
        Err(e) => e.append(other),
    }
}
