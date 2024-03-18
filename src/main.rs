use std::{
    env,
    fs::{create_dir, remove_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

fn generate_html(source_dir_name: &String) -> Result<(), std::io::Error> {
    let source_dir = env::current_dir()?.join(source_dir_name);
    let build_dir_prefix = "build";

    // Check if the source directory exists
    if !source_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory {} is not found", source_dir_name),
        ));
    }

    // Clean up the build directory
    if let Err(e) = remove_dir_all(build_dir_prefix) {
        if e.kind() != std::io::ErrorKind::NotFound {
            println!("{}: {e}", build_dir_prefix);
            return Err(e);
        }
    }

    let mut html_list = vec![];

    for entry in WalkDir::new(source_dir.clone())
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Ok(relative_path) = entry.path().strip_prefix(env::current_dir()?.as_path()) {
            let target_path = {
                let mut buf = PathBuf::from(
                    relative_path
                        .to_string_lossy()
                        .replace(source_dir_name, build_dir_prefix),
                );

                if buf.extension() == Some("adoc".as_ref()) {
                    buf.set_extension("html");
                    html_list.push(
                        buf.strip_prefix(build_dir_prefix)
                            .unwrap()
                            .to_string_lossy()
                            .to_string(),
                    );
                }

                buf
            };

            if relative_path.is_dir() {
                create_dir(&target_path)?;
            } else if relative_path.extension() == Some("adoc".as_ref()) {
                let html_src = run_asciidoctor(None, relative_path)?;
                let mut f = File::create(target_path)?;
                f.write_all(html_src.as_bytes())?;
            }
        }
    }
    generate_index_html(html_list)?;
    Ok(())
}

fn generate_index_html(html_list: Vec<String>) -> Result<(), std::io::Error> {
    // create index.adoc
    let index_adoc = Path::new("build/index.adoc");
    let mut f = File::create(index_adoc)?;
    writeln!(f, "= Index")?;
    writeln!(f)?;
    for path in html_list.iter() {
        writeln!(f, "* xref:{}[{}]", path, path)?;
    }

    // generate index.html
    let html_src = run_asciidoctor(None, index_adoc)?;
    let mut f = File::create("build/index.html")?;
    f.write_all(html_src.as_bytes())?;

    // remove index.adoc
    std::fs::remove_file(index_adoc)?;
    Ok(())
}

fn run_asciidoctor(command: Option<String>, file: &Path) -> Result<String, std::io::Error> {
    if let Some(_command) = command {
        unimplemented!();
    } else {
        let output = Command::new("asciidoctor")
            .args(["-r", "asciidoctor-diagram"])
            .args(["-o", "-"])
            .arg(file)
            .output()
            .expect("failed to execute asciidoctor");
        String::from_utf8(output.stdout)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <target_dir>", args[0]);
    } else {
        generate_html(&args[1]).unwrap();
    }
}
