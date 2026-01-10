use std::collections::HashMap;

use crate::{Anyhow, anyhow};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    Class(String),
    Function {
        params: Vec<Self>,
        ret: Box<Self>,
    },
    Method {
        class: String,
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
            Self::Unknown => false,
            Self::Function { params, ret } | Self::Method { params, ret, .. } => {
                ret.is_resolved() && params.iter().all(Self::is_resolved)
            }
            _ => true,
        }
    }

    pub fn unify(&self, other: &Self) -> Anyhow<Self> {
        match (self, other) {
            (Self::Int, Self::Int) => Ok(Self::Int),
            (Self::Bool, Self::Bool) => Ok(Self::Bool),
            (Self::Unit, Self::Unit) => Ok(Self::Unit),
            (Self::Class(a), Self::Class(b)) if a == b => Ok(Self::Class(a.clone())),
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

    pub const fn is_class(&self) -> bool {
        matches!(self, Self::Class(_))
    }

    pub fn class_name(&self) -> Option<&str> {
        if let Self::Class(name) = self {
            Some(name)
        } else {
            None
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
            Self::Class(name) => write!(f, "{name}"),
            Self::Function { params, ret } => {
                let params_str = params
                    .iter()
                    .map(Self::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "({params_str}) -> {ret}")
            }
            Self::Method { class, params, ret } => {
                let params_str = std::iter::once(class.clone())
                    .chain(params.iter().map(Self::to_string))
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "({params_str}) -> {ret}")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub fields: HashMap<String, Type>,
    pub field_order: Vec<String>,
    pub methods: HashMap<String, MethodInfo>,
    pub has_destructor: bool,
}

/// Information about a method
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub return_type: Type,
    pub is_constructor: bool,
    pub is_destructor: bool,
}

impl ClassInfo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            fields: HashMap::new(),
            field_order: Vec::new(),
            methods: HashMap::new(),
            has_destructor: false,
        }
    }

    pub fn add_field(&mut self, name: String, ty: Type) {
        self.fields.insert(name.clone(), ty);
        self.field_order.push(name);
    }

    pub fn add_method(&mut self, info: MethodInfo) {
        if info.is_destructor {
            self.has_destructor = true;
        }

        self.methods.insert(info.name.clone(), info);
    }

    pub fn get_field(&self, name: &str) -> Option<&Type> {
        self.fields.get(name)
    }

    pub fn get_method(&self, name: &str) -> Option<&MethodInfo> {
        self.methods.get(name)
    }

    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.field_order.iter().position(|n| n == name)
    }

    pub fn size(&self) -> usize {
        self.fields.len()
    }
}

pub type ClassRegistry = HashMap<String, ClassInfo>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unify_same_types() {
        assert_eq!(Type::Int.unify(&Type::Int).unwrap(), Type::Int);
        assert_eq!(Type::Bool.unify(&Type::Bool).unwrap(), Type::Bool);
    }

    #[test]
    fn test_unify_class_types() {
        let point = Type::Class("Point".to_string());
        assert_eq!(point.unify(&point).unwrap(), point);

        let vec = Type::Class("Vec".to_string());
        assert!(point.unify(&vec).is_err());
    }

    #[test]
    fn test_unify_unknown() {
        assert_eq!(Type::Unknown.unify(&Type::Int).unwrap(), Type::Int);
        assert_eq!(Type::Int.unify(&Type::Unknown).unwrap(), Type::Int);
    }

    #[test]
    fn test_class_info() {
        let mut class = ClassInfo::new("Point".to_string());
        class.add_field("x".to_string(), Type::Int);
        class.add_field("y".to_string(), Type::Int);

        assert_eq!(class.size(), 2);
        assert_eq!(class.field_index("x"), Some(0));
        assert_eq!(class.field_index("y"), Some(1));
    }
}
