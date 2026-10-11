sed -i 's/let content = (0..100)/use std::fmt::Write;\n        let mut content = String::new();\n        for i in 0..100 {\n            let _ = write!(content, "# Heading {i}\\n\\ntext\\n\\n");\n        }/g' src/outline/render.rs
sed -i 's/.map(|i| format!("# Heading {i}\\n\\ntext\\n\\n"))//g' src/outline/render.rs
sed -i 's/.collect::<String>();//g' src/outline/render.rs
cargo clippy --all-targets --all-features
