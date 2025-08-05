use std::path::{Path, PathBuf};

use clap::{ArgMatches, Command, arg};
use log::error;
use pkm::{Editor, Result, ZettelBuilder, ZettelPathBuf};

fn cli() -> Command {
    Command::new("pkm")
        .about("A PKM management CLI")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(arg!(REPO: <REPO> "The root directory of the pkm").env("PKM_REPO"))
        .subcommand(
            Command::new("zettel")
                .about("Create a new zettel")
                .arg(arg!(ZETTEL_DIR: --"zettel-dir" [ZETTEL_DIR] "The directory where zettels are stored relative to the repo directory").env("PKM_ZETTEL_DIR").default_value("zettels"))
                .arg(arg!(TEMPLATE_DIR: --"template-dir" [TEMPLATE_DIR] "The directory where templates are stored relative to the repo directory").env("PKM_TEMPLATE_DIR").default_value("tmpl"))
                .arg(arg!(TEMPLATE: -t --template [TEMPLATE] "The template of the zettel"))
                .arg(arg!(EDIT: -e --edit "Open the zettel in your $EDITOR after creation"))
                .arg(arg!(TITLE: <TITLE> "The title of the zettel"))
                .arg(arg!(VARS: ... "variables for the template (title:\"Hello World\")"))
                .arg_required_else_help(true),
        )
        .subcommand(Command::new("search").about("Finds your relavent data"))
}

fn main() {
    env_logger::init();

    let matches = cli().get_matches();

    let res = match matches.subcommand() {
        Some(("zettel", sub_matches)) => run_zettel(
            sub_matches,
            matches.get_one::<String>("REPO").expect("required"),
        ),

        _ => unreachable!(), // If all subcommands are defined above, anything else is unreachable!()
    };

    if let Err(err) = res {
        error!("{}", err)
    }
}

fn run_zettel<P>(sub_matches: &ArgMatches, repo: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut context = tera::Context::new();
    let title = sub_matches.get_one::<String>("TITLE").expect("required");
    // add the title
    context.insert("title", title);

    if let Some(vars) = sub_matches.get_many::<String>("VARS") {
        for value in vars {
            if let Some((key, value)) = value.split_once(":") {
                context.insert(key, value)
            }
        }
    }

    // destination starts with the path to the repo
    let mut destination = PathBuf::new();
    destination.push(repo.as_ref());
    // then add the zettel directory
    destination.push(
        sub_matches
            .get_one::<String>("ZETTEL_DIR")
            .expect("defaulted"),
    );

    // the date directory structure
    destination.push_date_path();
    // the name of the file
    destination.filename_with_hash(title);

    let mut template_dir = PathBuf::new();
    template_dir.push(repo.as_ref());

    template_dir.push(
        sub_matches
            .get_one::<String>("TEMPLATE_DIR")
            .expect("defaulted"),
    );

    ZettelBuilder::new(destination.as_path(), template_dir.as_path())
        .template(sub_matches.get_one::<String>("TEMPLATE"))
        .build(&context)?;

    if let Some(true) = sub_matches.get_one::<bool>("EDIT") {
        Editor::new_from_env("EDITOR").file(destination).exec()?;
    } else {
        println!(
            "created zettel: {}",
            destination.as_path().to_string_lossy()
        )
    }

    Ok(())
}
