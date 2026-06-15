use std::path::Path;

fn main() {
    let path = Path::new(r"c:\soft_projects\NAM\assets\models\Diezel CH 2 Crunch 3.nam");
    println!("Checking model at {:?}", path);
    match nam_rs::NamModel::from_file(&path) {
        Ok(model) => {
            println!("Successfully loaded model.");
            println!("Loudness: {:?}", model.loudness());
        }
        Err(e) => {
            println!("Error loading model: {:?}", e);
        }
    }
}
