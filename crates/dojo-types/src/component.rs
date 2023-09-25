use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

/// Represents a component member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub key: bool,
}

/// Represents a component of an entity
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityComponent {
    pub component: String,
    pub keys: Vec<FieldElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMetadata {
    pub name: String,
    pub size: u32,
    pub class_hash: FieldElement,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Ty {
    Terminal(String),
    Struct(Struct),
    Enum(Enum),
}

impl Ty {
    pub fn name(&self) -> String {
        match self {
            Ty::Terminal(s) => s.clone(),
            Ty::Struct(s) => s.name.clone(),
            Ty::Enum(e) => e.name.clone(),
        }
    }

    pub fn flatten(&self) -> Vec<Ty> {
        let mut flattened = Ty::flatten_ty(self.clone());
        flattened.reverse();
        flattened
    }

    fn flatten_ty(ty: Ty) -> Vec<Ty> {
        let mut items = vec![];
        match ty {
            Ty::Terminal(_) => {
                items.push(ty.clone());
            }
            Ty::Struct(mut s) => {
                for (i, member) in s.children.clone().iter().enumerate() {
                    match member.ty {
                        Ty::Struct(_) => {
                            items.extend(Ty::flatten_ty(member.ty.clone()));
                        }
                        Ty::Enum(_) => {
                            items.extend(Ty::flatten_ty(member.ty.clone()));
                        }
                        _ => {}
                    }

                    s.children[i].ty = Ty::Terminal(member.ty.name());
                }

                items.push(Ty::Struct(s))
            }
            Ty::Enum(mut e) => {
                for (i, ty) in e.values.clone().iter().enumerate() {
                    match ty {
                        Ty::Struct(_) => {
                            items.extend(Ty::flatten_ty(ty.clone()));
                        }
                        Ty::Enum(_) => {
                            items.extend(Ty::flatten_ty(ty.clone()));
                        }
                        _ => {}
                    }

                    e.values[i] = Ty::Terminal(ty.name());
                }

                items.push(Ty::Enum(e))
            }
        };

        items
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut items = self.flatten();
        items.reverse();
        let str = items
            .iter()
            .map(|ty| match ty {
                Ty::Terminal(s) => s.to_string(),
                Ty::Struct(s) => {
                    let mut struct_str = format!("struct {} {{\n", s.name);
                    for member in &s.children {
                        struct_str.push_str(&format!("{},\n", format_member(member)));
                    }
                    struct_str.push('}');
                    struct_str
                }
                Ty::Enum(e) => {
                    let mut enum_str = format!("enum {} {{\n", e.name);
                    for ty in &e.values {
                        enum_str.push_str(&format!("  {}\n", ty.name()));
                    }
                    enum_str.push('}');
                    enum_str
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        write!(f, "{}", str)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Struct {
    pub name: String,
    pub children: Vec<Member>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<Ty>,
}

fn format_member(m: &Member) -> String {
    if m.key {
        format!("  #[key]\n  {}: {}", m.name, m.ty.name())
    } else {
        format!("  {}: {}", m.name, m.ty.name())
    }
}
