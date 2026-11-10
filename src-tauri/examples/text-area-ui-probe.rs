//! Browser-test bridge: one bounded request on stdin, real native preparation on stdout.
use folio_engine::{prepare_text_overlay, FontAsset, TextOverlay};
use serde_json::json;
use std::io::{self, Read};

fn main() {
    let font = FontAsset::parse(
        include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec(),
    )
    .expect("bundled fixture font");
    if std::env::args().nth(1).as_deref() == Some("--info") {
        println!("{}", serde_json::to_string(&font.info).unwrap());
        return;
    }
    let mut input = String::new();
    io::stdin().take(65537).read_to_string(&mut input).unwrap();
    let result = if input.len() > 65536 {
        json!({"error":"Fixture request exceeds 64 KiB"})
    } else {
        match serde_json::from_str::<TextOverlay>(&input) {
            Ok(overlay) => match prepare_text_overlay(&font, &overlay) {
                Ok(result) => json!({"result":result}),
                Err(error) => json!({"error":error.to_string()}),
            },
            Err(error) => json!({"error":error.to_string()}),
        }
    };
    println!("{result}");
}
