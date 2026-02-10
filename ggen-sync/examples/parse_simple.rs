use ggen_sync::parse_code;

fn main() {
    let code = r#"
        pub struct Person {
            pub name: String,
            pub age: u32,
        }
    "#;

    match parse_code(code) {
        Ok(nodes) => {
            println!("Parsed {} types:", nodes.len());
            for (name, node) in &nodes {
                println!("  {} with {} fields:", name, node.fields.len());
                for field in &node.fields {
                    println!("    {}: {}", field.name, field.field_type);
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
