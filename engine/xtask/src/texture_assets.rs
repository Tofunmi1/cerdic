//! Copy the procedural texture compiler's source definitions into the same-origin web build.
use std::fs;
use std::path::Path;

type Failure = Box<dyn std::error::Error>;

pub(crate) fn copy_assets() -> Result<(), Failure> {
    const SOURCE: &str = "lib/material-maker/addons/material_maker";
    const DEST: &str = "web/dist/texture-library";
    let mut names = Vec::new();
    let nodes = Path::new(SOURCE).join("nodes");
    fs::create_dir_all(format!("{DEST}/nodes"))?;
    for entry in fs::read_dir(nodes)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "mmg") {
            let name = path
                .file_stem()
                .ok_or("texture node has no name")?
                .to_string_lossy()
                .into_owned();
            fs::copy(&path, format!("{DEST}/nodes/{name}.mmg"))?;
            names.push(name);
        }
    }
    names.sort();
    fs::write(format!("{DEST}/index.json"), serde_json::to_vec(&names)?)?;
    fs::copy(
        format!("{SOURCE}/shader_functions.tres"),
        format!("{DEST}/shader_functions.tres"),
    )?;
    Ok(())
}
