use dojo::meta::{FieldLayout, Layout};
use dojo::utils::layout::find_field_layout;

fn build_field_layouts(nb_of_fields: u32) -> Span<FieldLayout> {
    let mut fields = array![];

    for i in 0..nb_of_fields {
        fields
            .append(FieldLayout { selector: i.into(), layout: Layout::Fixed([1, 2, 3, 4].span()) });
    }

    fields.span()
}

#[test]
fn bench_find_field_layout() {
    let _ = find_field_layout(255, build_field_layouts(256));
}
