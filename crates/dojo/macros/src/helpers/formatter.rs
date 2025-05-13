use super::Member;

pub struct DojoFormatter {}

/// DojoFormatter provides some functions to format data structure
/// to be used in output token streams.
impl DojoFormatter {
    /// Build serializing statement of a member from its description.
    pub(crate) fn serialize_member_ty(member: &Member, with_self: bool) -> String {
        format!(
            "core::serde::Serde::serialize({}{}, ref serialized);\n",
            if with_self { "self." } else { "@" },
            member.name
        )
    }

    /// Return member declaration statement from member name and type.
    pub(crate) fn get_member_declaration(name: &str, ty: &str) -> String {
        format!("pub {}: {},\n", name, ty)
    }
}
