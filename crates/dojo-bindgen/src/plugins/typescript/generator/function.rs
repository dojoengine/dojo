const TS_FUNCTION_TPL: &str = "
// Type definition for `{path}` struct
export function {name}({params}): {return_type} {{
    {body}
}}
";