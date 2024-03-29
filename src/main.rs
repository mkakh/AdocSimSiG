use scraper::{Html, Selector};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

fn generate_html(source_dir_name: &str) -> Result<(), std::io::Error> {
    // remove the trailing slash
    let source_dir_name = source_dir_name.trim_end_matches('/');

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
    if let Err(e) = fs::remove_dir_all(build_dir_prefix) {
        if e.kind() != std::io::ErrorKind::NotFound {
            println!("{}: {e}", build_dir_prefix);
            return Err(e);
        }
    }

    let mut index_adoc = vec!["= Index".to_string(), "".to_string()];

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
                }

                buf
            };

            if relative_path.is_dir() {
                let level = relative_path.components().count() - 1;
                let stars = "*".repeat(level);
                index_adoc.push(format!(
                    "{} {}",
                    stars,
                    relative_path
                        .strip_prefix(source_dir_name)
                        .unwrap()
                        .display()
                ));

                if !target_path.exists() {
                    fs::create_dir(&target_path)?;
                }
            } else if relative_path.extension() == Some("adoc".as_ref()) {
                let html_src = run_asciidoctor(None, relative_path, source_dir_name)?;
                let mut f = fs::File::create(&target_path)?;
                f.write_all(html_src.as_bytes())?;
                let title = extract_html_title(&html_src)?;

                let level = relative_path.components().count() - 1;
                let stars = "*".repeat(level);
                index_adoc.push(format!(
                    "{} xref:{}[{}]",
                    stars,
                    &target_path
                        .strip_prefix(build_dir_prefix)
                        .unwrap()
                        .to_string_lossy(),
                    title
                ));
            } else {
                fs::copy(relative_path, target_path)?;
            }
        }
    }
    // create index.adoc
    let index_adoc_path = Path::new("build/index.adoc");
    let mut f = fs::File::create(index_adoc_path)?;
    writeln!(f, "{}", index_adoc.join("\n"))?;
    generate_index_html(source_dir_name)?;
    Ok(())
}

fn extract_html_title(html_file_content: &str) -> Result<String, std::io::Error> {
    let document = Html::parse_document(html_file_content);

    // Create a selector for the <title> element
    let title_selector = Selector::parse("title").unwrap();

    // Find the first matching <title> element
    if let Some(title_element) = document.select(&title_selector).next() {
        // Get the text content of the <title> element
        let title = title_element.text().collect::<String>();
        Ok(title)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "No title is found in the HTML file.",
        ))
    }
}

fn generate_index_html(source_dir_name: &str) -> Result<(), std::io::Error> {
    let index_adoc_path = Path::new("build/index.adoc");
    // generate index.html
    let html_src = run_asciidoctor(None, index_adoc_path, source_dir_name)?;
    let mut f = fs::File::create("build/index.html")?;
    f.write_all(html_src.as_bytes())?;

    // remove index.adoc
    std::fs::remove_file(index_adoc_path)?;
    Ok(())
}

fn run_asciidoctor(
    command: Option<String>,
    file_path: &Path,
    source_dir_name: &str,
) -> Result<String, std::io::Error> {
    let file_path = std::fs::canonicalize(file_path)?;
    let build_path = PathBuf::from(
        file_path
            .parent()
            .unwrap()
            .to_string_lossy()
            .replace(source_dir_name, "build"),
    );
    let current_dir = env::current_dir()?;
    fs::create_dir_all(&build_path)?;
    env::set_current_dir(&build_path)?;

    if let Some(_command) = command {
        unimplemented!();
    } else {
        assert!(file_path.exists());
        let output = Command::new("asciidoctor")
            .args(["-r", "asciidoctor-diagram"])
            .args(["-o", "-"])
            .arg(file_path)
            .output()
            .expect("failed to execute asciidoctor");
        env::set_current_dir(current_dir)?;
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
