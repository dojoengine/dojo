use serde::{Deserialize, Serialize};

/// Represents a component member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub key: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Ty {
    Simple(String),
    Struct(Struct),
    Enum(Enum),
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut items = print_ty(self);
        items.reverse();
        write!(f, "{}", items.join("\n\n"))
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

fn format_name(ty: &Ty) -> String {
    match ty {
        Ty::Simple(s) => s.clone(),
        Ty::Struct(s) => s.name.clone(),
        Ty::Enum(e) => e.name.clone(),
    }
}

fn format_member(m: &Member) -> String {
    if m.key {
        format!("  #[key]\n  {}: {}", m.name, format_name(&m.ty))
    } else {
        format!("  {}: {}", m.name, format_name(&m.ty))
    }
}

fn print_ty(ty: &Ty) -> Vec<String> {
    let mut items = vec![];
    match ty {
        Ty::Simple(s) => println!("{}", s),
        Ty::Struct(s) => {
            let mut struct_str = format!("struct {} {{\n", s.name);
            for member in &s.children {
                match member.ty {
                    Ty::Struct(_) => {
                        items.extend(print_ty(&member.ty));
                    }
                    Ty::Enum(_) => {
                        items.extend(print_ty(&member.ty));
                    }
                    _ => {}
                }

                struct_str.push_str(&format!("{},\n", format_member(member)));
            }
            struct_str.push('}');
            items.push(struct_str);
        }
        Ty::Enum(e) => {
            let mut enum_str = format!("enum {} {{\n", e.name);
            for ty in &e.values {
                match ty {
                    Ty::Struct(_) => {
                        items.extend(print_ty(ty));
                    }
                    Ty::Enum(_) => {
                        items.extend(print_ty(ty));
                    }
                    _ => {}
                }

                enum_str.push_str(&format!("  {}\n", format_name(ty)));
            }
            enum_str.push('}');
            items.push(enum_str);
        }
    };

    items
}
