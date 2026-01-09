use crate::{Anyhow, anyhow};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    Function {
        params: Vec<Self>,
        ret: Box<Self>,
    },
    Unit,
    #[default]
    Unknown,
}

impl Type {
    pub fn is_resolved(&self) -> bool {
        match self {
            Self::Int | Self::Bool | Self::Unit => true,
            Self::Unknown => false,
            Self::Function { params, ret } => {
                ret.is_resolved() && params.iter().all(Self::is_resolved)
            }
        }
    }

    pub fn unify(&self, other: &Self) -> Anyhow<Self> {
        match (self, other) {
            (Self::Int, Self::Int) => Ok(Self::Int),
            (Self::Bool, Self::Bool) => Ok(Self::Bool),
            (Self::Unit, Self::Unit) => Ok(Self::Unit),
            (Self::Unknown, t) | (t, Self::Unknown) => Ok(t.clone()),
            (
                Self::Function {
                    params: p1,
                    ret: r1,
                },
                Self::Function {
                    params: p2,
                    ret: r2,
                },
            ) if p1.len() == p2.len() => {
                let params = p1
                    .iter()
                    .zip(p2.iter())
                    .map(|(a, b)| a.unify(b))
                    .collect::<Anyhow<Vec<_>>>()?;

                let ret = Box::new(r1.unify(r2)?);

                Ok(Self::Function { params, ret })
            }
            _ => Err(anyhow!("type mismatch: expected {self:?}, got {other:?}")),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int => write!(f, "int"),
            Self::Bool => write!(f, "bool"),
            Self::Unit => write!(f, "()"),
            Self::Unknown => write!(f, "?"),
            Self::Function { params, ret } => {
                let params_str = params
                    .iter()
                    .map(Self::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "({params_str}) -> {ret}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unify_same_types() {
        assert_eq!(Type::Int.unify(&Type::Int).unwrap(), Type::Int);
        assert_eq!(Type::Bool.unify(&Type::Bool).unwrap(), Type::Bool);
    }

    #[test]
    fn test_unify_unknown() {
        assert_eq!(Type::Unknown.unify(&Type::Int).unwrap(), Type::Int);
        assert_eq!(Type::Int.unify(&Type::Unknown).unwrap(), Type::Int);
    }

    #[test]
    fn test_unify_mismatch() {
        assert!(Type::Int.unify(&Type::Bool).is_err());
    }
}
