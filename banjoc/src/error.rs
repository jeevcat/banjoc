pub type Result<T> = std::result::Result<T, BanjoError>;
#[derive(Debug)]
pub enum BanjoError {
    CompileError(&'static str),
    CompileErrors(Vec<&'static str>),
    RuntimeError(String),
}

impl BanjoError {
    pub fn append(&mut self, other: Self) {
        match self {
            BanjoError::CompileError(this) => match other {
                BanjoError::CompileError(other) => *self = Self::CompileErrors(vec![this, other]),
                BanjoError::CompileErrors(mut other) => {
                    other.push(this);
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
