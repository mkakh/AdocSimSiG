use scraper::{Html, Selector};
use std::{
    collections::HashSet,
    env, fs,
    io::{self, Write},
    path::{Component, Path},
    process::Command,
};
use walkdir::WalkDir;

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn output_name(name: &str) -> io::Result<&str> {
    let mut components = Path::new(name).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        Ok(name)
    } else {
        Err(invalid_input("output name must be one directory name"))
    }
}

fn generate_html(source_name: &str, output_name_arg: &str) -> io::Result<()> {
    let cwd = env::current_dir()?;
    let source = fs::canonicalize(source_name)?;
    if !source.is_dir() {
        return Err(invalid_input("input must be a directory"));
    }
    let output = cwd.join(output_name(output_name_arg)?);
    if source == output || source.starts_with(&output) {
        return Err(invalid_input(
            "output directory cannot contain the input directory",
        ));
    }

    if let Ok(metadata) = fs::symlink_metadata(&output) {
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid_input("output path must be a regular directory"));
        }
        // Only the historical default build directory is replaced. An existing
        // custom directory cannot be reliably identified as generated output.
        if output_name_arg != "build" {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to replace existing directory {}",
                    output.display()
                ),
            ));
        }
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir(&output)?;

    // Record only folders with AsciiDoc files beneath them. Asset-only folders
    // are still copied but should not appear in the index.
    let mut document_dirs = HashSet::new();
    for entry in WalkDir::new(&source)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_output_dir(entry, &output))
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "adoc")
        {
            let mut parent = entry.path().parent();
            while let Some(dir) = parent {
                if dir == source {
                    break;
                }
                document_dirs.insert(dir.to_path_buf());
                parent = dir.parent();
            }
        }
    }

    let mut index = vec!["= Index".to_string(), String::new()];
    for entry in WalkDir::new(&source)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_output_dir(entry, &output))
    {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if error.loop_ancestor().is_some() => continue,
            Err(error) => return Err(io::Error::other(error)),
        };
        let relative = entry
            .path()
            .strip_prefix(&source)
            .map_err(io::Error::other)?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = output.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
            if document_dirs.contains(entry.path()) {
                index.push(format!(
                    "{} {}",
                    "*".repeat(relative.components().count()),
                    relative.display()
                ));
            }
        } else if relative.extension().is_some_and(|ext| ext == "adoc") {
            let html = run_asciidoctor(entry.path(), target.parent().unwrap())?;
            let title = extract_html_title(&html)?;
            let mut link = relative.to_path_buf();
            link.set_extension("html");
            fs::write(output.join(&link), html)?;
            index.push(format!(
                "{} xref:{}[{}]",
                "*".repeat(relative.components().count()),
                link.display(),
                title
            ));
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    let index_path = output.join("index.adoc");
    let mut f = fs::File::create(&index_path)?;
    writeln!(f, "{}", index.join("\n"))?;
    let html = run_asciidoctor(&index_path, &output)?;
    fs::write(output.join("index.html"), html)?;
    fs::remove_file(index_path)?;
    Ok(())
}

fn is_output_dir(entry: &walkdir::DirEntry, output: &Path) -> bool {
    entry.file_type().is_dir()
        && (entry.path().starts_with(output)
            || fs::canonicalize(entry.path()).is_ok_and(|path| path.starts_with(output)))
}

fn extract_html_title(html: &str) -> io::Result<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("title").unwrap();
    document
        .select(&selector)
        .next()
        .map(|title| title.text().collect::<String>())
        .ok_or_else(|| io::Error::other("No title is found in the HTML file."))
}

fn run_asciidoctor(file_path: &Path, output_dir: &Path) -> io::Result<String> {
    let file_path = fs::canonicalize(file_path)?;
    let result = Command::new("asciidoctor")
        .current_dir(output_dir)
        .args(["-r", "asciidoctor-diagram", "-o", "-"])
        .arg(&file_path)
        .output()?;
    if !result.status.success() {
        return Err(io::Error::other(format!(
            "asciidoctor failed for {}: {}",
            file_path.display(),
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    String::from_utf8(result.stdout).map_err(io::Error::other)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if !(2..=3).contains(&args.len()) {
        eprintln!("Usage: {} <input_dir> [output_dir_name]", args[0]);
        std::process::exit(2);
    }
    if let Err(error) = generate_html(&args[1], args.get(2).map(String::as_str).unwrap_or("build"))
    {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
