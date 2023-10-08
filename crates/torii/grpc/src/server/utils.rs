use dojo_types::schema::{Member, Struct, Ty};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(Debug, sqlx::FromRow)]
struct ModelMember {
    id: String,
    model_idx: u32,
    member_idx: u32,
    name: String,
    r#type: String,
    type_enum: TypeEnum,
    key: bool,
}

#[derive(
    AsRefStr, Display, EnumIter, EnumString, Clone, Debug, Serialize, Deserialize, PartialEq,
)]
enum TypeEnum {
    Primitive,
    Struct,
    Enum,
    Tuple,
}

// assume that the model members are sorted by model_idx and member_idx
// `id` is the type id of the model member
fn parse_ty(model: String, model_members: Vec<ModelMember>) -> Ty {
    let mut model_members = model_members.into_iter();

    fn parse_struct<'a>(path: String, model_members: &[ModelMember]) -> Ty {
        let model_name = path.split("$").last().unwrap();
        let members = model_members.iter().filter(|m| &m.id == &path).collect::<Vec<_>>();

        let mut children: Vec<Member> = vec![];

        for child in members {
            match child.type_enum {
                TypeEnum::Primitive => {
                    children.push(Member {
                        key: child.key,
                        name: child.name,
                        ty: Ty::Primitive(child.r#type.parse().unwrap()),
                    });
                }

                TypeEnum::Struct => {
                    children.push(Member {
                        key: child.key,
                        name: child.name,
                        ty: parse_struct(child.id, model_members),
                    });
                }
            }
        }

        Ty::Struct(Struct { name: model_name.to_string(), children })
    }
}
