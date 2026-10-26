fn main() {
    let request = std::env::args().nth(1).expect("json request");
    match ve_modeling::creature::make_from_json(&request) {
        Ok(text) => print!("{text}"),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
